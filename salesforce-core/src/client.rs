use oauth2::basic::{BasicClient, BasicTokenType};
use oauth2::{AuthUrl, ClientId, ClientSecret, EmptyExtraTokenFields, TokenUrl};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Default OAuth2 authorization endpoint path.
const DEFAULT_AUTHORIZE_PATH: &str = "/services/oauth2/authorize";

/// Default OAuth2 token endpoint path.
const DEFAULT_TOKEN_PATH: &str = "/services/oauth2/token";

/// Errors that can occur during client operations.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Failed to read credentials file from disk.
    #[error("Failed to read credentials file at {path}: {source}")]
    ReadCredentials {
        /// Path to the credentials file that failed to read.
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Failed to parse credentials JSON.
    #[error("Failed to parse credentials JSON: {source}")]
    ParseCredentials {
        #[source]
        source: serde_json::Error,
    },
    /// Invalid URL format in credentials.
    #[error("Invalid URL format: {source}")]
    ParseUrl {
        #[source]
        source: url::ParseError,
    },
    /// OAuth2 token exchange failed during authentication.
    #[error("OAuth2 token exchange failed: {0:?}")]
    TokenExchange(Box<dyn std::error::Error + Send + Sync>),
    /// Required builder parameter was not provided.
    #[error("Missing required attribute: {}", _0)]
    MissingRequiredAttribute(String),
    /// Invalid credentials for the selected auth flow.
    #[error("Invalid credentials for {flow}: {message}")]
    InvalidCredentials {
        /// The authentication flow that failed validation.
        flow: String,
        /// Description of what's missing or invalid.
        message: String,
    },
}

/// OAuth2 authentication flow type.
///
/// Salesforce supports multiple OAuth2 flows for different use cases.
/// Choose the appropriate flow based on your application's requirements.
///
/// # Flow Descriptions
///
/// ## Client Credentials
///
/// The Client Credentials flow is used for server-to-server API integration
/// where the application acts on its own behalf rather than on behalf of a user.
/// This is the default flow if not specified.
///
/// **Use when:** Your application needs to access Salesforce APIs without user interaction.
///
/// **Required credentials:**
/// - `client_id`
/// - `client_secret`
///
/// ## Username-Password
///
/// The Resource Owner Password Credentials flow allows authentication using
/// a username and password. This flow should only be used when there is a high
/// degree of trust between the user and the application.
///
/// **Use when:** You need to authenticate as a specific user programmatically.
///
/// **Required credentials:**
/// - `client_id`
/// - `client_secret`
/// - `username`
/// - `password`
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthFlow {
    /// OAuth2 Client Credentials flow for server-to-server authentication.
    ///
    /// Requires: `client_id`, `client_secret`
    #[default]
    ClientCredentials,
    /// OAuth2 Resource Owner Password Credentials flow for user authentication.
    ///
    /// Requires: `client_id`, `client_secret`, `username`, `password`
    UsernamePassword,
}

/// Salesforce OAuth2 credentials.
///
/// Obtained from a Salesforce Connected App. Different fields are required
/// depending on the [`AuthFlow`] used.
///
/// # Creating a Connected App
///
/// 1. In Salesforce Setup, navigate to App Manager
/// 2. Create a new Connected App
/// 3. Enable OAuth Settings
/// 4. Set the callback URL and select OAuth scopes
/// 5. Copy the Consumer Key (client_id) and Consumer Secret (client_secret)
///
/// # Examples
///
/// ## Client Credentials
///
/// ```
/// use salesforce_core::client::Credentials;
///
/// let creds = Credentials {
///     client_id: "your_client_id".to_string(),
///     client_secret: Some("your_client_secret".to_string()),
///     username: None,
///     password: None,
///     instance_url: "https://your-instance.salesforce.com".to_string(),
///     tenant_id: "your_tenant_id".to_string(),
/// };
/// ```
///
/// ## Username-Password
///
/// ```
/// use salesforce_core::client::Credentials;
///
/// let creds = Credentials {
///     client_id: "your_client_id".to_string(),
///     client_secret: Some("your_client_secret".to_string()),
///     username: Some("user@example.com".to_string()),
///     password: Some("your_password".to_string()),
///     instance_url: "https://your-instance.salesforce.com".to_string(),
///     tenant_id: "your_tenant_id".to_string(),
/// };
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Credentials {
    /// Client ID from the Connected App (Consumer Key).
    pub client_id: String,
    /// Client Secret from the Connected App (Consumer Secret).
    ///
    /// Required for: [`AuthFlow::ClientCredentials`], [`AuthFlow::UsernamePassword`]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// Username for authentication (email address).
    ///
    /// Required for: [`AuthFlow::UsernamePassword`]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Password for authentication.
    ///
    /// Required for: [`AuthFlow::UsernamePassword`]
    ///
    /// **Note:** If your org requires a security token, append it to the password.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Salesforce instance URL (e.g., `https://mydomain.salesforce.com`).
    ///
    /// For production orgs, use `https://login.salesforce.com`.
    /// For sandbox orgs, use `https://test.salesforce.com`.
    pub instance_url: String,
    /// Organization ID (15 or 18 character Salesforce Org ID).
    pub tenant_id: String,
}

/// Source for loading credentials.
#[derive(Debug, Clone)]
pub enum CredentialsFrom {
    /// Load credentials from a JSON file.
    Path(PathBuf),
    /// Use credentials provided directly.
    Value(Credentials),
}

/// OAuth2 client for Salesforce API authentication.
///
/// Use [`Builder`] to construct a client instance. The client supports multiple
/// OAuth2 authentication flows via the [`AuthFlow`] enum.
///
/// # Examples
///
/// ## Client Credentials Flow (Default)
///
/// ```no_run
/// use salesforce_core::client::{self, Credentials, AuthFlow};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = client::Builder::new()
///     .credentials(Credentials {
///         client_id: "your_client_id".to_string(),
///         client_secret: Some("your_client_secret".to_string()),
///         username: None,
///         password: None,
///         instance_url: "https://your-instance.salesforce.com".to_string(),
///         tenant_id: "your_tenant_id".to_string(),
///     })
///     // .auth_flow(AuthFlow::ClientCredentials) // Optional, this is the default
///     .build()?
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Username-Password Flow
///
/// ```no_run
/// use salesforce_core::client::{self, Credentials, AuthFlow};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = client::Builder::new()
///     .credentials(Credentials {
///         client_id: "your_client_id".to_string(),
///         client_secret: Some("your_client_secret".to_string()),
///         username: Some("user@example.com".to_string()),
///         password: Some("your_password".to_string()),
///         instance_url: "https://your-instance.salesforce.com".to_string(),
///         tenant_id: "your_tenant_id".to_string(),
///     })
///     .auth_flow(AuthFlow::UsernamePassword)
///     .build()?
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Loading Credentials from File
///
/// ```no_run
/// use salesforce_core::client;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = client::Builder::new()
///     .credentials_path(PathBuf::from("credentials.json"))
///     .build()?
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
#[allow(clippy::type_complexity)]
pub struct Client {
    /// Source of credentials (file path or direct value).
    credentials_from: CredentialsFrom,
    /// OAuth2 authentication flow to use.
    auth_flow: AuthFlow,
    /// OAuth2 token response containing access token and metadata.
    pub token_result: Option<oauth2::StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>>,
    /// Salesforce instance URL.
    pub instance_url: Option<String>,
    /// Organization ID.
    pub tenant_id: Option<String>,
}

impl Client {
    /// Validates that required credential fields are present for the selected auth flow.
    fn validate_credentials(&self, credentials: &Credentials) -> Result<(), Error> {
        let flow_name = format!("{:?}", self.auth_flow);

        match self.auth_flow {
            AuthFlow::ClientCredentials => {
                if credentials.client_secret.is_none() {
                    return Err(Error::InvalidCredentials {
                        flow: flow_name,
                        message: "client_secret is required".to_string(),
                    });
                }
            }
            AuthFlow::UsernamePassword => {
                if credentials.client_secret.is_none() {
                    return Err(Error::InvalidCredentials {
                        flow: flow_name.clone(),
                        message: "client_secret is required".to_string(),
                    });
                }
                if credentials.username.is_none() {
                    return Err(Error::InvalidCredentials {
                        flow: flow_name.clone(),
                        message: "username is required".to_string(),
                    });
                }
                if credentials.password.is_none() {
                    return Err(Error::InvalidCredentials {
                        flow: flow_name,
                        message: "password is required".to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Connects to Salesforce and exchanges credentials for an access token.
    ///
    /// This method performs the configured OAuth2 flow to obtain
    /// an access token for API authentication.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Credentials file cannot be read ([`Error::ReadCredentials`])
    /// - Credentials JSON is invalid ([`Error::ParseCredentials`])
    /// - Required fields are missing for the auth flow ([`Error::InvalidCredentials`])
    /// - Instance URL is malformed ([`Error::ParseUrl`])
    /// - OAuth2 token exchange fails ([`Error::TokenExchange`])
    pub async fn connect(mut self) -> Result<Self, Error> {
        let credentials = match &self.credentials_from {
            CredentialsFrom::Value(creds) => creds.clone(),
            CredentialsFrom::Path(path) => {
                let credentials_string =
                    fs::read_to_string(path).map_err(|e| Error::ReadCredentials {
                        path: path.clone(),
                        source: e,
                    })?;
                serde_json::from_str(&credentials_string)
                    .map_err(|e| Error::ParseCredentials { source: e })?
            }
        };

        // Validate credentials for the selected auth flow
        self.validate_credentials(&credentials)?;

        // Create HTTP client for async requests
        let http_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| Error::TokenExchange(Box::new(e)))?;

        let token_result = match self.auth_flow {
            AuthFlow::ClientCredentials => {
                self.exchange_client_credentials(&credentials, &http_client)
                    .await?
            }
            AuthFlow::UsernamePassword => {
                self.exchange_password(&credentials, &http_client).await?
            }
        };

        self.token_result = Some(token_result);
        self.instance_url = Some(credentials.instance_url);
        self.tenant_id = Some(credentials.tenant_id);

        Ok(self)
    }

    /// Performs OAuth2 Client Credentials flow.
    async fn exchange_client_credentials(
        &self,
        credentials: &Credentials,
        http_client: &reqwest::Client,
    ) -> Result<oauth2::StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>, Error> {
        let client_secret =
            credentials
                .client_secret
                .as_ref()
                .ok_or_else(|| Error::InvalidCredentials {
                    flow: "ClientCredentials".to_string(),
                    message: "client_secret is required".to_string(),
                })?;

        let oauth2_client = BasicClient::new(ClientId::new(credentials.client_id.clone()))
            .set_client_secret(ClientSecret::new(client_secret.clone()))
            .set_auth_uri(
                AuthUrl::new(format!(
                    "{}{}",
                    credentials.instance_url, DEFAULT_AUTHORIZE_PATH
                ))
                .map_err(|e| Error::ParseUrl { source: e })?,
            )
            .set_token_uri(
                TokenUrl::new(format!(
                    "{}{}",
                    credentials.instance_url, DEFAULT_TOKEN_PATH
                ))
                .map_err(|e| Error::ParseUrl { source: e })?,
            );

        oauth2_client
            .exchange_client_credentials()
            .request_async(http_client)
            .await
            .map_err(|e| Error::TokenExchange(Box::new(e)))
    }

    /// Performs OAuth2 Resource Owner Password Credentials flow.
    async fn exchange_password(
        &self,
        credentials: &Credentials,
        http_client: &reqwest::Client,
    ) -> Result<oauth2::StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>, Error> {
        let client_secret =
            credentials
                .client_secret
                .as_ref()
                .ok_or_else(|| Error::InvalidCredentials {
                    flow: "UsernamePassword".to_string(),
                    message: "client_secret is required".to_string(),
                })?;

        let username = credentials
            .username
            .as_ref()
            .ok_or_else(|| Error::InvalidCredentials {
                flow: "UsernamePassword".to_string(),
                message: "username is required".to_string(),
            })?;

        let password = credentials
            .password
            .as_ref()
            .ok_or_else(|| Error::InvalidCredentials {
                flow: "UsernamePassword".to_string(),
                message: "password is required".to_string(),
            })?;

        let oauth2_client = BasicClient::new(ClientId::new(credentials.client_id.clone()))
            .set_client_secret(ClientSecret::new(client_secret.clone()))
            .set_auth_uri(
                AuthUrl::new(format!(
                    "{}{}",
                    credentials.instance_url, DEFAULT_AUTHORIZE_PATH
                ))
                .map_err(|e| Error::ParseUrl { source: e })?,
            )
            .set_token_uri(
                TokenUrl::new(format!(
                    "{}{}",
                    credentials.instance_url, DEFAULT_TOKEN_PATH
                ))
                .map_err(|e| Error::ParseUrl { source: e })?,
            );

        oauth2_client
            .exchange_password(
                &oauth2::ResourceOwnerUsername::new(username.clone()),
                &oauth2::ResourceOwnerPassword::new(password.clone()),
            )
            .request_async(http_client)
            .await
            .map_err(|e| Error::TokenExchange(Box::new(e)))
    }
}

/// Builder for constructing a [`Client`].
///
/// The builder allows you to configure the authentication flow and credentials
/// source before creating a client instance.
///
/// # Examples
///
/// ## Using Client Credentials Flow
///
/// ```no_run
/// use salesforce_core::client::{self, Credentials, AuthFlow};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = client::Builder::new()
///     .credentials(Credentials {
///         client_id: "your_client_id".to_string(),
///         client_secret: Some("your_client_secret".to_string()),
///         username: None,
///         password: None,
///         instance_url: "https://your-instance.salesforce.com".to_string(),
///         tenant_id: "your_tenant_id".to_string(),
///     })
///     .auth_flow(AuthFlow::ClientCredentials)
///     .build()?
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Using Username-Password Flow
///
/// ```no_run
/// use salesforce_core::client::{self, Credentials, AuthFlow};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = client::Builder::new()
///     .credentials(Credentials {
///         client_id: "your_client_id".to_string(),
///         client_secret: Some("your_client_secret".to_string()),
///         username: Some("user@example.com".to_string()),
///         password: Some("your_password".to_string()),
///         instance_url: "https://your-instance.salesforce.com".to_string(),
///         tenant_id: "your_tenant_id".to_string(),
///     })
///     .auth_flow(AuthFlow::UsernamePassword)
///     .build()?
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Loading from File
///
/// ```no_run
/// use salesforce_core::client;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = client::Builder::new()
///     .credentials_path(PathBuf::from("credentials.json"))
///     .build()?
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct Builder {
    credentials_from: Option<CredentialsFrom>,
    auth_flow: Option<AuthFlow>,
}

impl Builder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets credentials to load from a JSON file.
    ///
    /// The file should contain a JSON object with the required fields for
    /// your chosen authentication flow. For example:
    ///
    /// ```json
    /// {
    ///   "client_id": "your_client_id",
    ///   "client_secret": "your_client_secret",
    ///   "instance_url": "https://your-instance.salesforce.com",
    ///   "tenant_id": "your_tenant_id"
    /// }
    /// ```
    ///
    /// For [`AuthFlow::UsernamePassword`], also include `username` and `password`.
    pub fn credentials_path(mut self, path: PathBuf) -> Self {
        self.credentials_from = Some(CredentialsFrom::Path(path));
        self
    }

    /// Sets credentials directly.
    ///
    /// Provide a [`Credentials`] struct with the appropriate fields populated
    /// for your chosen authentication flow.
    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.credentials_from = Some(CredentialsFrom::Value(credentials));
        self
    }

    /// Sets the OAuth2 authentication flow.
    ///
    /// Defaults to [`AuthFlow::ClientCredentials`] if not specified.
    ///
    /// # Available Flows
    ///
    /// - [`AuthFlow::ClientCredentials`] - Server-to-server authentication
    /// - [`AuthFlow::UsernamePassword`] - User authentication with username and password
    pub fn auth_flow(mut self, auth_flow: AuthFlow) -> Self {
        self.auth_flow = Some(auth_flow);
        self
    }

    /// Builds the client.
    ///
    /// # Errors
    ///
    /// Returns an error if credentials were not provided via either
    /// [`credentials_path`](Self::credentials_path) or [`credentials`](Self::credentials).
    pub fn build(self) -> Result<Client, Error> {
        Ok(Client {
            credentials_from: self.credentials_from.ok_or_else(|| {
                Error::MissingRequiredAttribute("credentials or credentials_path".to_string())
            })?,
            auth_flow: self.auth_flow.unwrap_or_default(),
            token_result: None,
            instance_url: None,
            tenant_id: None,
        })
    }
}

#[cfg(test)]
mod tests {

    use std::env;

    use super::*;

    #[test]
    fn test_build_without_credentials() {
        let client = Builder::new().build();
        assert!(matches!(
            client,
            Err(Error::MissingRequiredAttribute(attr)) if attr == "credentials or credentials_path"
        ));
    }

    #[test]
    fn test_build_with_credentials() {
        let mut path = env::temp_dir();
        path.push(format!("credentials_{}.json", std::process::id()));
        let client = Builder::new().credentials_path(path).build();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_connect_with_invalid_credentials() {
        let creds: &str = r#"{"client_id":"client_id"}"#;
        let mut path = env::temp_dir();
        path.push(format!("invalid_credentials_{}.json", std::process::id()));
        let _ = fs::write(path.clone(), creds);
        let client = Builder::new()
            .credentials_path(path.clone())
            .build()
            .unwrap();
        let result = client.connect().await;
        let _ = fs::remove_file(path);
        assert!(matches!(result, Err(Error::ParseCredentials { .. })));
    }

    #[tokio::test]
    async fn test_connect_with_invalid_url() {
        let creds: &str = r#"
            {
                "client_id": "some_client_id",
                "client_secret": "some_client_secret",
                "instance_url": "mydomain.salesforce.com",
                "tenant_id": "some_tenant_id"
            }"#;
        let mut path = env::temp_dir();
        path.push(format!(
            "invalid_url_credentials_{}.json",
            std::process::id()
        ));
        let _ = fs::write(path.clone(), creds);
        let client = Builder::new()
            .credentials_path(path.clone())
            .build()
            .unwrap();
        let result = client.connect().await;
        let _ = fs::remove_file(path);
        assert!(matches!(result, Err(Error::ParseUrl { .. })));
    }

    #[tokio::test]
    async fn test_connect_with_missing_file() {
        let mut path = env::temp_dir();
        path.push(format!("nonexistent_{}.json", std::process::id()));
        let client = Builder::new().credentials_path(path).build().unwrap();
        let result = client.connect().await;
        assert!(matches!(result, Err(Error::ReadCredentials { .. })));
    }

    #[test]
    fn test_builder_default() {
        let builder = Builder::default();
        assert!(builder.credentials_from.is_none());
    }

    #[test]
    fn test_builder_credentials_path() {
        let path = PathBuf::from("/tmp/test.json");
        let builder = Builder::new().credentials_path(path.clone());
        assert!(matches!(
            builder.credentials_from,
            Some(CredentialsFrom::Path(_))
        ));
    }

    #[test]
    fn test_builder_credentials_value() {
        let creds = Credentials {
            client_id: "test_id".to_string(),
            client_secret: Some("test_secret".to_string()),
            username: None,
            password: None,
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant".to_string(),
        };
        let builder = Builder::new().credentials(creds);
        assert!(matches!(
            builder.credentials_from,
            Some(CredentialsFrom::Value(_))
        ));
    }

    #[test]
    fn test_error_display_missing_attribute() {
        let error = Error::MissingRequiredAttribute("test_field".to_string());
        assert_eq!(error.to_string(), "Missing required attribute: test_field");
    }

    #[test]
    fn test_error_display_read_credentials() {
        let path = PathBuf::from("/tmp/test.json");
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = Error::ReadCredentials {
            path: path.clone(),
            source: io_error,
        };
        assert!(error.to_string().contains("/tmp/test.json"));
    }

    #[tokio::test]
    async fn test_connect_with_valid_json_but_invalid_credentials() {
        let creds: &str = r#"
            {
                "client_id": "test_client_id",
                "client_secret": "test_client_secret",
                "instance_url": "https://test.salesforce.com",
                "tenant_id": "test_tenant_id"
            }"#;
        let mut path = env::temp_dir();
        path.push(format!(
            "valid_json_invalid_creds_{}.json",
            std::process::id()
        ));
        let _ = fs::write(path.clone(), creds);
        let client = Builder::new()
            .credentials_path(path.clone())
            .build()
            .unwrap();
        let result = client.connect().await;
        let _ = fs::remove_file(path);
        // Should fail with TokenExchange error (invalid credentials)
        assert!(matches!(result, Err(Error::TokenExchange(_))));
    }

    #[test]
    fn test_client_debug_impl() {
        let path = PathBuf::from("/tmp/test.json");
        let client = Builder::new().credentials_path(path).build().unwrap();
        let debug_str = format!("{client:?}");
        assert!(debug_str.contains("Client"));
    }

    #[test]
    fn test_client_clone() {
        let path = PathBuf::from("/tmp/test.json");
        let client = Builder::new().credentials_path(path).build().unwrap();
        let cloned = client.clone();
        assert!(matches!(
            (&client.credentials_from, &cloned.credentials_from),
            (CredentialsFrom::Path(_), CredentialsFrom::Path(_))
        ));
    }

    #[tokio::test]
    async fn test_connect_with_direct_credentials() {
        let creds = Credentials {
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_client_secret".to_string()),
            username: None,
            password: None,
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant_id".to_string(),
        };
        let client = Builder::new().credentials(creds).build().unwrap();
        let result = client.connect().await;
        // Should fail with TokenExchange error (invalid credentials)
        assert!(matches!(result, Err(Error::TokenExchange(_))));
    }

    #[test]
    fn test_default_authorize_path() {
        assert_eq!(DEFAULT_AUTHORIZE_PATH, "/services/oauth2/authorize");
    }

    #[test]
    fn test_default_token_path() {
        assert_eq!(DEFAULT_TOKEN_PATH, "/services/oauth2/token");
    }

    #[tokio::test]
    async fn test_client_credentials_flow_missing_secret() {
        let creds = Credentials {
            client_id: "test_client_id".to_string(),
            client_secret: None,
            username: None,
            password: None,
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant_id".to_string(),
        };
        let client = Builder::new()
            .credentials(creds)
            .auth_flow(AuthFlow::ClientCredentials)
            .build()
            .unwrap();
        let result = client.connect().await;
        assert!(matches!(result, Err(Error::InvalidCredentials { .. })));
    }

    #[tokio::test]
    async fn test_username_password_flow_missing_username() {
        let creds = Credentials {
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_secret".to_string()),
            username: None,
            password: Some("test_password".to_string()),
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant_id".to_string(),
        };
        let client = Builder::new()
            .credentials(creds)
            .auth_flow(AuthFlow::UsernamePassword)
            .build()
            .unwrap();
        let result = client.connect().await;
        assert!(matches!(result, Err(Error::InvalidCredentials { .. })));
    }

    #[tokio::test]
    async fn test_username_password_flow_missing_password() {
        let creds = Credentials {
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_secret".to_string()),
            username: Some("test_user".to_string()),
            password: None,
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant_id".to_string(),
        };
        let client = Builder::new()
            .credentials(creds)
            .auth_flow(AuthFlow::UsernamePassword)
            .build()
            .unwrap();
        let result = client.connect().await;
        assert!(matches!(result, Err(Error::InvalidCredentials { .. })));
    }

    #[tokio::test]
    async fn test_username_password_flow_with_valid_fields() {
        let creds = Credentials {
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_secret".to_string()),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant_id".to_string(),
        };
        let client = Builder::new()
            .credentials(creds)
            .auth_flow(AuthFlow::UsernamePassword)
            .build()
            .unwrap();
        let result = client.connect().await;
        // Should fail with TokenExchange error (invalid credentials, but validation passed)
        assert!(matches!(result, Err(Error::TokenExchange(_))));
    }

    #[test]
    fn test_auth_flow_default() {
        let default_flow = AuthFlow::default();
        assert_eq!(default_flow, AuthFlow::ClientCredentials);
    }

    #[test]
    fn test_builder_auth_flow() {
        let path = PathBuf::from("/tmp/test.json");
        let client = Builder::new()
            .credentials_path(path)
            .auth_flow(AuthFlow::UsernamePassword)
            .build()
            .unwrap();
        assert_eq!(client.auth_flow, AuthFlow::UsernamePassword);
    }

    #[test]
    fn test_credentials_serde() {
        let creds = Credentials {
            client_id: "test_id".to_string(),
            client_secret: Some("test_secret".to_string()),
            username: Some("test_user".to_string()),
            password: Some("test_pass".to_string()),
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant".to_string(),
        };

        let json = serde_json::to_string(&creds).unwrap();
        let deserialized: Credentials = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.client_id, "test_id");
        assert_eq!(deserialized.client_secret, Some("test_secret".to_string()));
        assert_eq!(deserialized.username, Some("test_user".to_string()));
        assert_eq!(deserialized.password, Some("test_pass".to_string()));
    }

    #[test]
    fn test_credentials_serde_optional_fields() {
        let creds = Credentials {
            client_id: "test_id".to_string(),
            client_secret: Some("test_secret".to_string()),
            username: None,
            password: None,
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant".to_string(),
        };

        let json = serde_json::to_string(&creds).unwrap();
        assert!(!json.contains("username"));
        assert!(!json.contains("password"));
    }

    #[test]
    fn test_auth_flow_serde() {
        let flow = AuthFlow::ClientCredentials;
        let json = serde_json::to_string(&flow).unwrap();
        assert_eq!(json, "\"client_credentials\"");

        let flow = AuthFlow::UsernamePassword;
        let json = serde_json::to_string(&flow).unwrap();
        assert_eq!(json, "\"username_password\"");
    }

    #[test]
    fn test_auth_flow_deserde() {
        let json = "\"client_credentials\"";
        let flow: AuthFlow = serde_json::from_str(json).unwrap();
        assert_eq!(flow, AuthFlow::ClientCredentials);

        let json = "\"username_password\"";
        let flow: AuthFlow = serde_json::from_str(json).unwrap();
        assert_eq!(flow, AuthFlow::UsernamePassword);
    }

    #[test]
    fn test_credentials_debug() {
        let creds = Credentials {
            client_id: "test_id".to_string(),
            client_secret: Some("secret".to_string()),
            username: None,
            password: None,
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "tenant".to_string(),
        };
        let debug_str = format!("{creds:?}");
        assert!(debug_str.contains("test_id"));
        assert!(debug_str.contains("Credentials"));
    }

    #[test]
    fn test_credentials_clone() {
        let creds = Credentials {
            client_id: "test_id".to_string(),
            client_secret: Some("secret".to_string()),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "tenant".to_string(),
        };
        let cloned = creds.clone();
        assert_eq!(creds.client_id, cloned.client_id);
        assert_eq!(creds.username, cloned.username);
    }

    #[test]
    fn test_auth_flow_equality() {
        assert_eq!(AuthFlow::ClientCredentials, AuthFlow::ClientCredentials);
        assert_ne!(AuthFlow::ClientCredentials, AuthFlow::UsernamePassword);
    }

    #[test]
    fn test_auth_flow_clone() {
        let flow = AuthFlow::UsernamePassword;
        let cloned = flow;
        assert_eq!(flow, cloned);
    }

    #[test]
    fn test_error_debug() {
        let error = Error::MissingRequiredAttribute("test".to_string());
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("MissingRequiredAttribute"));
    }

    #[test]
    fn test_credentials_from_debug() {
        let creds_from = CredentialsFrom::Path(PathBuf::from("/tmp/test.json"));
        let debug_str = format!("{creds_from:?}");
        assert!(debug_str.contains("Path"));

        let creds = Credentials {
            client_id: "test".to_string(),
            client_secret: Some("secret".to_string()),
            username: None,
            password: None,
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "tenant".to_string(),
        };
        let creds_from = CredentialsFrom::Value(creds);
        let debug_str = format!("{creds_from:?}");
        assert!(debug_str.contains("Value"));
    }

    #[test]
    fn test_credentials_from_clone() {
        let path = PathBuf::from("/tmp/test.json");
        let creds_from = CredentialsFrom::Path(path.clone());
        let cloned = creds_from.clone();
        assert!(matches!(cloned, CredentialsFrom::Path(_)));
    }

    #[tokio::test]
    async fn test_username_password_flow_missing_client_secret() {
        let creds = Credentials {
            client_id: "test_client_id".to_string(),
            client_secret: None,
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            instance_url: "https://test.salesforce.com".to_string(),
            tenant_id: "test_tenant_id".to_string(),
        };
        let client = Builder::new()
            .credentials(creds)
            .auth_flow(AuthFlow::UsernamePassword)
            .build()
            .unwrap();
        let result = client.connect().await;
        assert!(matches!(result, Err(Error::InvalidCredentials { .. })));
    }

    #[test]
    fn test_error_source() {
        use std::error::Error as StdError;

        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let error = Error::ReadCredentials {
            path: PathBuf::from("/tmp/test.json"),
            source: io_error,
        };
        assert!(error.source().is_some());
    }
}
