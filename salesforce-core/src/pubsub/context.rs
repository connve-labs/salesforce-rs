use crate::client;
use oauth2::TokenResponse;
use salesforce_pubsub_v1::eventbus::v1::pub_sub_client::PubSubClient;
use tokio_stream::StreamExt;

/// Errors that can occur during Pub/Sub operations.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Client is missing or not initialized.
    #[error("Client missing")]
    MissingClient(),
    /// OAuth2 token is missing from client.
    #[error("Token response missing")]
    MissingTokenResponse(),
    /// Required client attribute is missing.
    #[error("Missing required attribute: {}", _0)]
    MissingRequiredAttribute(String),
    /// Failed to create valid gRPC metadata from client credentials.
    #[error("Invalid metadata value for gRPC headers: {source}")]
    InvalidMetadataValue {
        #[source]
        source: tonic::metadata::errors::InvalidMetadataValue,
    },
    /// gRPC communication error.
    #[error("gRPC transport error: {0}")]
    Tonic(Box<tonic::Status>),
}

struct ContextInterceptor {
    auth_header: tonic::metadata::AsciiMetadataValue,
    instance_url: tonic::metadata::AsciiMetadataValue,
    tenant_id: tonic::metadata::AsciiMetadataValue,
}

impl tonic::service::Interceptor for ContextInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request
            .metadata_mut()
            .insert("accesstoken", self.auth_header.to_owned());
        request
            .metadata_mut()
            .insert("instanceurl", self.instance_url.to_owned());
        request
            .metadata_mut()
            .insert("tenantid", self.tenant_id.to_owned());
        Ok(request)
    }
}

/// Pub/Sub API context for making gRPC calls.
///
/// Manages authentication and provides methods for interacting with
/// Salesforce Pub/Sub API endpoints.
///
/// # Examples
///
/// ```no_run
/// use salesforce_core::client;
/// use salesforce_core::pubsub::context::Context;
/// use salesforce_pubsub_v1::eventbus;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = client::Builder::new()
///     .credentials_path(PathBuf::from("credentials.json"))
///     .build()?
///     .connect()
///     .await?;
///
/// let channel = tonic::transport::Channel::from_static(eventbus::ENDPOINT)
///     .connect()
///     .await?;
///
/// let mut context = Context::new(channel, client)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Context {
    pubsub: salesforce_pubsub_v1::eventbus::v1::pub_sub_client::PubSubClient<
        tonic::service::interceptor::InterceptedService<
            tonic::transport::Channel,
            ContextInterceptor,
        >,
    >,
}

impl Context {
    /// Creates a new Pub/Sub context.
    ///
    /// # Errors
    ///
    /// Returns an error if the client is missing required authentication data.
    pub fn new(channel: tonic::transport::Channel, client: client::Client) -> Result<Self, Error> {
        let auth_header: tonic::metadata::AsciiMetadataValue = client
            .token_result
            .as_ref()
            .ok_or_else(Error::MissingTokenResponse)?
            .access_token()
            .secret()
            .parse()
            .map_err(|e| Error::InvalidMetadataValue { source: e })?;

        let instance_url: tonic::metadata::AsciiMetadataValue = client
            .instance_url
            .as_ref()
            .ok_or_else(|| Error::MissingRequiredAttribute("instance_url".to_string()))?
            .parse()
            .map_err(|e| Error::InvalidMetadataValue { source: e })?;

        let tenant_id: tonic::metadata::AsciiMetadataValue = client
            .tenant_id
            .as_ref()
            .ok_or_else(|| Error::MissingRequiredAttribute("tenant_id".to_string()))?
            .parse()
            .map_err(|e| Error::InvalidMetadataValue { source: e })?;

        let interceptor = ContextInterceptor {
            auth_header,
            instance_url,
            tenant_id,
        };

        let pubsub = PubSubClient::with_interceptor(channel, interceptor);

        Ok(Context { pubsub })
    }

    /// Retrieves topic metadata.
    ///
    /// Returns information about a topic including schema ID, permissions,
    /// and RPC ID.
    pub async fn get_topic(
        &mut self,
        request: salesforce_pubsub_v1::eventbus::v1::TopicRequest,
    ) -> Result<tonic::Response<salesforce_pubsub_v1::eventbus::v1::TopicInfo>, Error> {
        self.pubsub
            .get_topic(tonic::Request::new(request))
            .await
            .map_err(|e| Error::Tonic(Box::new(e)))
    }

    /// Retrieves schema information for a topic.
    ///
    /// Returns the Avro schema definition for the specified schema ID.
    pub async fn get_schema(
        &mut self,
        request: salesforce_pubsub_v1::eventbus::v1::SchemaRequest,
    ) -> Result<tonic::Response<salesforce_pubsub_v1::eventbus::v1::SchemaInfo>, Error> {
        self.pubsub
            .get_schema(tonic::Request::new(request))
            .await
            .map_err(|e| Error::Tonic(Box::new(e)))
    }

    /// Publishes events to a topic.
    ///
    /// Sends a batch of events to the specified topic. Events must be
    /// serialized according to the topic's Avro schema.
    pub async fn publish(
        &mut self,
        request: salesforce_pubsub_v1::eventbus::v1::PublishRequest,
    ) -> Result<tonic::Response<salesforce_pubsub_v1::eventbus::v1::PublishResponse>, Error> {
        self.pubsub
            .publish(tonic::Request::new(request))
            .await
            .map_err(|e| Error::Tonic(Box::new(e)))
    }

    /// Subscribes to events from a topic.
    ///
    /// Returns a stream of events. The stream will continue until an error
    /// occurs or the connection is closed.
    pub async fn subscribe(
        &mut self,
        request: salesforce_pubsub_v1::eventbus::v1::FetchRequest,
    ) -> Result<
        tonic::Response<tonic::codec::Streaming<salesforce_pubsub_v1::eventbus::v1::FetchResponse>>,
        Error,
    > {
        self.pubsub
            .subscribe(
                tokio_stream::iter(1..usize::MAX)
                    .map(move |_| request.to_owned())
                    .throttle(std::time::Duration::from_millis(10)),
            )
            .await
            .map_err(|e| Error::Tonic(Box::new(e)))
    }

    /// Subscribes to events using a managed subscription.
    ///
    /// Requires a pre-configured managed subscription in Salesforce.
    /// Returns a stream of events with automatic commit handling.
    pub async fn managed_subscribe(
        &mut self,
        request: salesforce_pubsub_v1::eventbus::v1::ManagedFetchRequest,
    ) -> Result<
        tonic::Response<
            tonic::codec::Streaming<salesforce_pubsub_v1::eventbus::v1::ManagedFetchResponse>,
        >,
        Error,
    > {
        self.pubsub
            .managed_subscribe(
                tokio_stream::iter(1..usize::MAX)
                    .map(move |_| request.to_owned())
                    .throttle(std::time::Duration::from_millis(10)),
            )
            .await
            .map_err(|e| Error::Tonic(Box::new(e)))
    }

    /// Publishes events via a bidirectional stream.
    ///
    /// Allows for continuous publishing with server responses for each batch.
    /// Useful for high-throughput scenarios.
    pub async fn publish_stream(
        &mut self,
        request: salesforce_pubsub_v1::eventbus::v1::PublishRequest,
    ) -> Result<
        tonic::Response<
            tonic::codec::Streaming<salesforce_pubsub_v1::eventbus::v1::PublishResponse>,
        >,
        Error,
    > {
        self.pubsub
            .publish_stream(
                tokio_stream::iter(1..usize::MAX)
                    .map(move |_| request.to_owned())
                    .throttle(std::time::Duration::from_millis(10)),
            )
            .await
            .map_err(|e| Error::Tonic(Box::new(e)))
    }
}

#[cfg(test)]
mod tests {

    use std::{fs, path::PathBuf};

    use super::*;
    use tonic::service::Interceptor;

    #[tokio::test]
    async fn test_new_missing_token() {
        let channel = tonic::transport::Channel::from_static("https://api.pubsub.salesforce.com")
            .connect()
            .await
            .unwrap();
        let creds: &str = r#"
            {
                "client_id": "some_client_id",
                "client_secret": "some_client_secret",
                "instance_url": "https://mydomain.salesforce.com",
                "tenant_id": "some_tenant_id"
            }"#;
        let mut path = PathBuf::new();
        path.push("credentials.json");
        let _ = fs::write(path.clone(), creds);
        let client = client::Builder::new()
            .credentials_path(path.clone())
            .build()
            .unwrap();
        let _ = fs::remove_file(path);
        let result = Context::new(channel, client);
        assert!(matches!(result, Err(Error::MissingTokenResponse())));
    }

    #[test]
    fn test_error_display_missing_client() {
        let error = Error::MissingClient();
        assert_eq!(error.to_string(), "Client missing");
    }

    #[test]
    fn test_error_display_missing_token_response() {
        let error = Error::MissingTokenResponse();
        assert_eq!(error.to_string(), "Token response missing");
    }

    #[test]
    fn test_error_display_missing_required_attribute() {
        let error = Error::MissingRequiredAttribute("test_attr".to_string());
        assert_eq!(error.to_string(), "Missing required attribute: test_attr");
    }

    #[test]
    fn test_error_display_invalid_metadata() {
        // Create an invalid metadata value (contains invalid ASCII character)
        let invalid_value = tonic::metadata::AsciiMetadataValue::try_from("\n").unwrap_err();
        let error = Error::InvalidMetadataValue {
            source: invalid_value,
        };
        assert!(error.to_string().contains("Invalid metadata value"));
    }

    #[tokio::test]
    async fn test_new_missing_instance_url() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        let creds: &str = r#"
            {
                "client_id": "some_client_id",
                "client_secret": "some_client_secret",
                "instance_url": "https://mydomain.salesforce.com",
                "tenant_id": "some_tenant_id"
            }"#;
        let mut path = PathBuf::new();
        path.push(format!("test_creds_{}.json", std::process::id()));
        let _ = fs::write(path.clone(), creds);

        let mut client = client::Builder::new()
            .credentials_path(path.clone())
            .build()
            .unwrap();
        let _ = fs::remove_file(path);

        // Manually set token but not instance_url
        let token = BasicTokenResponse::new(
            AccessToken::new("test_token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = None; // Missing instance_url
        client.tenant_id = Some("test_tenant".to_string());

        // Create a lazy channel (doesn't actually connect)
        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        let result = Context::new(channel, client);
        assert!(matches!(result, Err(Error::MissingRequiredAttribute(_))));
    }

    #[tokio::test]
    async fn test_new_missing_tenant_id() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        let creds: &str = r#"
            {
                "client_id": "some_client_id",
                "client_secret": "some_client_secret",
                "instance_url": "https://mydomain.salesforce.com",
                "tenant_id": "some_tenant_id"
            }"#;
        let mut path = PathBuf::new();
        path.push(format!("test_creds_tenant_{}.json", std::process::id()));
        let _ = fs::write(path.clone(), creds);

        let mut client = client::Builder::new()
            .credentials_path(path.clone())
            .build()
            .unwrap();
        let _ = fs::remove_file(path);

        // Manually set token and instance_url but not tenant_id
        let token = BasicTokenResponse::new(
            AccessToken::new("test_token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = Some("https://mydomain.salesforce.com".to_string());
        client.tenant_id = None; // Missing tenant_id

        // Create a lazy channel (doesn't actually connect)
        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        let result = Context::new(channel, client);
        assert!(matches!(result, Err(Error::MissingRequiredAttribute(_))));
    }

    #[tokio::test]
    async fn test_context_debug_impl() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        let creds: &str = r#"
            {
                "client_id": "some_client_id",
                "client_secret": "some_client_secret",
                "instance_url": "https://mydomain.salesforce.com",
                "tenant_id": "some_tenant_id"
            }"#;
        let mut path = PathBuf::new();
        path.push(format!("test_creds_debug_{}.json", std::process::id()));
        let _ = fs::write(path.clone(), creds);

        let mut client = client::Builder::new()
            .credentials_path(path.clone())
            .build()
            .unwrap();
        let _ = fs::remove_file(path);

        let token = BasicTokenResponse::new(
            AccessToken::new("test_token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = Some("https://mydomain.salesforce.com".to_string());
        client.tenant_id = Some("test_tenant".to_string());

        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        let context = Context::new(channel, client).unwrap();
        let debug_str = format!("{context:?}");
        assert!(debug_str.contains("Context"));
    }

    #[tokio::test]
    async fn test_new_with_valid_client() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        let mut client = client::Builder::new()
            .credentials(client::Credentials {
                client_id: "test_id".to_string(),
                client_secret: Some("test_secret".to_string()),
                username: None,
                password: None,
                instance_url: "https://test.salesforce.com".to_string(),
                tenant_id: "test_tenant".to_string(),
            })
            .build()
            .unwrap();

        let token = BasicTokenResponse::new(
            AccessToken::new("valid_token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = Some("https://test.salesforce.com".to_string());
        client.tenant_id = Some("tenant123".to_string());

        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        let result = Context::new(channel, client);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_new_with_invalid_token_characters() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        let mut client = client::Builder::new()
            .credentials(client::Credentials {
                client_id: "test_id".to_string(),
                client_secret: Some("test_secret".to_string()),
                username: None,
                password: None,
                instance_url: "https://test.salesforce.com".to_string(),
                tenant_id: "test_tenant".to_string(),
            })
            .build()
            .unwrap();

        // Token with newline character (invalid ASCII for metadata)
        let token = BasicTokenResponse::new(
            AccessToken::new("token\nwith\nnewlines".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = Some("https://test.salesforce.com".to_string());
        client.tenant_id = Some("tenant123".to_string());

        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        let result = Context::new(channel, client);
        assert!(matches!(result, Err(Error::InvalidMetadataValue { .. })));
    }

    #[tokio::test]
    async fn test_new_with_invalid_instance_url_characters() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        let mut client = client::Builder::new()
            .credentials(client::Credentials {
                client_id: "test_id".to_string(),
                client_secret: Some("test_secret".to_string()),
                username: None,
                password: None,
                instance_url: "https://test.salesforce.com".to_string(),
                tenant_id: "test_tenant".to_string(),
            })
            .build()
            .unwrap();

        let token = BasicTokenResponse::new(
            AccessToken::new("valid_token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = Some("url\nwith\nnewlines".to_string());
        client.tenant_id = Some("tenant123".to_string());

        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        let result = Context::new(channel, client);
        assert!(matches!(result, Err(Error::InvalidMetadataValue { .. })));
    }

    #[tokio::test]
    async fn test_new_with_invalid_tenant_id_characters() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        let mut client = client::Builder::new()
            .credentials(client::Credentials {
                client_id: "test_id".to_string(),
                client_secret: Some("test_secret".to_string()),
                username: None,
                password: None,
                instance_url: "https://test.salesforce.com".to_string(),
                tenant_id: "test_tenant".to_string(),
            })
            .build()
            .unwrap();

        let token = BasicTokenResponse::new(
            AccessToken::new("valid_token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = Some("https://test.salesforce.com".to_string());
        client.tenant_id = Some("tenant\nwith\nnewlines".to_string());

        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        let result = Context::new(channel, client);
        assert!(matches!(result, Err(Error::InvalidMetadataValue { .. })));
    }

    #[test]
    fn test_error_tonic_debug() {
        let status = tonic::Status::internal("test error");
        let error = Error::Tonic(Box::new(status));
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("Tonic"));
    }

    #[test]
    fn test_error_invalid_credentials_display() {
        let error = Error::InvalidMetadataValue {
            source: tonic::metadata::AsciiMetadataValue::try_from("\n").unwrap_err(),
        };
        assert!(error.to_string().contains("Invalid metadata value"));
    }

    #[test]
    fn test_error_missing_client_display() {
        let error = Error::MissingClient();
        assert_eq!(format!("{error}"), "Client missing");
    }

    #[test]
    fn test_error_tonic_display() {
        let status = tonic::Status::unavailable("service unavailable");
        let error = Error::Tonic(Box::new(status));
        assert!(format!("{error}").contains("gRPC transport error"));
    }

    #[test]
    fn test_interceptor_adds_headers() {
        let auth_header = tonic::metadata::AsciiMetadataValue::try_from("test_token").unwrap();
        let instance_url =
            tonic::metadata::AsciiMetadataValue::try_from("https://test.salesforce.com").unwrap();
        let tenant_id = tonic::metadata::AsciiMetadataValue::try_from("test_tenant").unwrap();

        let mut interceptor = ContextInterceptor {
            auth_header,
            instance_url,
            tenant_id,
        };

        let request = tonic::Request::new(());
        let result = interceptor.call(request);

        assert!(result.is_ok());
        let request = result.unwrap();
        let metadata = request.metadata();

        assert_eq!(metadata.get("accesstoken").unwrap(), "test_token");
        assert_eq!(
            metadata.get("instanceurl").unwrap(),
            "https://test.salesforce.com"
        );
        assert_eq!(metadata.get("tenantid").unwrap(), "test_tenant");
    }

    #[tokio::test]
    async fn test_context_creation_success_path() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        // Create a client with all valid data
        let mut client = client::Builder::new()
            .credentials(client::Credentials {
                client_id: "client123".to_string(),
                client_secret: Some("secret123".to_string()),
                username: None,
                password: None,
                instance_url: "https://login.salesforce.com".to_string(),
                tenant_id: "00Dxx0000001gPL".to_string(),
            })
            .build()
            .unwrap();

        // Set valid token and metadata
        let token = BasicTokenResponse::new(
            AccessToken::new("valid_access_token_123".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = Some("https://login.salesforce.com".to_string());
        client.tenant_id = Some("00Dxx0000001gPL".to_string());

        // Create lazy channel
        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        // Create context - should succeed
        let context = Context::new(channel, client);
        assert!(context.is_ok());

        // Verify debug output
        let context = context.unwrap();
        let debug_str = format!("{context:?}");
        assert!(debug_str.contains("Context"));
        assert!(debug_str.contains("pubsub"));
    }

    #[test]
    fn test_credentials_from_value_variant() {
        let creds_from = client::CredentialsFrom::Value(client::Credentials {
            client_id: "test".to_string(),
            client_secret: Some("secret".to_string()),
            username: None,
            password: None,
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "tenant".to_string(),
        });

        match creds_from {
            client::CredentialsFrom::Value(creds) => {
                assert_eq!(creds.client_id, "test");
            }
            _ => panic!("Expected Value variant"),
        }
    }

    #[test]
    fn test_credentials_from_path_variant() {
        let path = PathBuf::from("/test/path.json");
        let creds_from = client::CredentialsFrom::Path(path.clone());

        match creds_from {
            client::CredentialsFrom::Path(p) => {
                assert_eq!(p, path);
            }
            _ => panic!("Expected Path variant"),
        }
    }

    #[test]
    fn test_error_display_all_variants() {
        // Test all error display implementations
        let errors = vec![
            Error::MissingClient(),
            Error::MissingTokenResponse(),
            Error::MissingRequiredAttribute("test_field".to_string()),
            Error::InvalidMetadataValue {
                source: tonic::metadata::AsciiMetadataValue::try_from("\n").unwrap_err(),
            },
            Error::Tonic(Box::new(tonic::Status::internal("test"))),
        ];

        for error in errors {
            let display = format!("{error}");
            assert!(!display.is_empty());
        }
    }

    #[tokio::test]
    async fn test_context_with_special_characters_in_token() {
        use oauth2::basic::BasicTokenResponse;
        use oauth2::{AccessToken, EmptyExtraTokenFields};

        let mut client = client::Builder::new()
            .credentials(client::Credentials {
                client_id: "test".to_string(),
                client_secret: Some("secret".to_string()),
                username: None,
                password: None,
                instance_url: "https://test.salesforce.com".to_string(),
                tenant_id: "tenant".to_string(),
            })
            .build()
            .unwrap();

        // Token with special characters that are valid ASCII
        let token = BasicTokenResponse::new(
            AccessToken::new("abc123-xyz_789.token".to_string()),
            oauth2::basic::BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        client.token_result = Some(token);
        client.instance_url = Some("https://test.salesforce.com".to_string());
        client.tenant_id = Some("tenant123".to_string());

        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        let result = Context::new(channel, client);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_all_missing_fields() {
        // Test with completely empty client
        let client = client::Builder::new()
            .credentials(client::Credentials {
                client_id: "test".to_string(),
                client_secret: Some("secret".to_string()),
                username: None,
                password: None,
                instance_url: "https://test.salesforce.com".to_string(),
                tenant_id: "tenant".to_string(),
            })
            .build()
            .unwrap();

        let endpoint = tonic::transport::Endpoint::from_static("http://localhost:50051");
        let channel = endpoint.connect_lazy();

        // Should fail due to missing token
        let result = Context::new(channel, client);
        assert!(matches!(result, Err(Error::MissingTokenResponse())));
    }
}
