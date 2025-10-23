use salesforce_core::client::{self, AuthFlow, Credentials};
use salesforce_core::pubsub::context::Context;
use salesforce_pubsub_v1::eventbus;
use std::env;
use std::path::PathBuf;
use tokio_stream::StreamExt;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Initialize client from credentials file
    // The auth_flow defaults to ClientCredentials if not specified
    let _client = client::Builder::new()
        .credentials_path(PathBuf::from("credentials.json"))
        .build()?
        .connect()
        .await?;

    // Example 2: Initialize client with Client Credentials flow (environment variables)
    let _client = client::Builder::new()
        .credentials(Credentials {
            client_id: env::var("SALESFORCE_CLIENT_ID")
                .expect("SALESFORCE_CLIENT_ID environment variable not set"),
            client_secret: Some(
                env::var("SALESFORCE_CLIENT_SECRET")
                    .expect("SALESFORCE_CLIENT_SECRET environment variable not set"),
            ),
            username: None,
            password: None,
            instance_url: env::var("SALESFORCE_INSTANCE_URL")
                .unwrap_or_else(|_| "https://mysalesforce.my.salesforce.com".to_string()),
            tenant_id: env::var("SALESFORCE_TENANT_ID")
                .expect("SALESFORCE_TENANT_ID environment variable not set"),
        })
        .auth_flow(AuthFlow::ClientCredentials)
        .build()?
        .connect()
        .await?;

    // Example 3: Initialize client with Username-Password flow
    let client = client::Builder::new()
        .credentials(Credentials {
            client_id: env::var("SALESFORCE_CLIENT_ID")
                .expect("SALESFORCE_CLIENT_ID environment variable not set"),
            client_secret: Some(
                env::var("SALESFORCE_CLIENT_SECRET")
                    .expect("SALESFORCE_CLIENT_SECRET environment variable not set"),
            ),
            username: Some(
                env::var("SALESFORCE_USERNAME")
                    .expect("SALESFORCE_USERNAME environment variable not set"),
            ),
            password: Some(
                env::var("SALESFORCE_PASSWORD")
                    .expect("SALESFORCE_PASSWORD environment variable not set"),
            ),
            instance_url: env::var("SALESFORCE_INSTANCE_URL")
                .unwrap_or_else(|_| "https://mysalesforce.my.salesforce.com".to_string()),
            tenant_id: env::var("SALESFORCE_TENANT_ID")
                .expect("SALESFORCE_TENANT_ID environment variable not set"),
        })
        .auth_flow(AuthFlow::UsernamePassword)
        .build()?
        .connect()
        .await?;

    info!("Client connected successfully");

    // Connect to Pub/Sub API
    let channel = tonic::transport::Channel::from_static(eventbus::ENDPOINT)
        .connect()
        .await?;

    let mut context = Context::new(channel, client)?;

    info!("Pub/Sub context created");

    // Example: Get topic information
    let topic_request = eventbus::v1::TopicRequest {
        topic_name: "/data/AccountChangeEvent".to_string(),
    };

    let schema_id = match context.get_topic(topic_request).await {
        Ok(response) => {
            let topic_info = response.into_inner();
            info!("Topic retrieved: {}", topic_info.topic_name);
            info!("Can publish: {}", topic_info.can_publish);
            info!("Can subscribe: {}", topic_info.can_subscribe);
            topic_info.schema_id
        }
        Err(e) => {
            error!("Failed to get topic: {e}");
            return Err(e.into());
        }
    };

    // Example: Get schema information using schema_id from topic
    let schema_request = eventbus::v1::SchemaRequest {
        schema_id: schema_id.clone(),
    };

    match context.get_schema(schema_request).await {
        Ok(response) => {
            let schema_info = response.into_inner();
            info!("Schema retrieved: {}", schema_info.schema_id);
        }
        Err(e) => {
            error!("Failed to get schema: {e}");
        }
    }

    // Example: Subscribe to events (infinite stream in background task)
    let fetch_request = eventbus::v1::FetchRequest {
        topic_name: "/data/AccountChangeEvent".to_string(),
        replay_preset: eventbus::v1::ReplayPreset::Latest.into(),
        num_requested: 100,
        ..Default::default()
    };

    match context.subscribe(fetch_request).await {
        Ok(response) => {
            info!("Subscribed to topic successfully");
            let stream = response.into_inner();

            tokio::spawn(async move {
                let mut stream = stream;
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(fetch_response) => {
                            info!("Received {} events", fetch_response.events.len());
                            for event in &fetch_response.events {
                                info!("Event replay_id: {:?}", event.replay_id);
                            }
                        }
                        Err(e) => {
                            error!("Stream error: {e}");
                            break;
                        }
                    }
                }
            });
        }
        Err(e) => {
            error!("Failed to subscribe: {e}");
        }
    }

    // Keep the main task alive so background tasks can run
    tokio::signal::ctrl_c().await?;

    Ok(())
}
