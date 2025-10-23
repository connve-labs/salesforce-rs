//! Unofficial Rust SDK for the Salesforce API.
//!
//! This crate provides authentication and Pub/Sub API support for Salesforce.
//!
//! # Examples
//!
//! ```no_run
//! use salesforce_core::client::{self, Credentials};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let client = client::Builder::new()
//!     .credentials(Credentials {
//!         client_id: "...".to_string(),
//!         client_secret: Some("...".to_string()),
//!         username: None,
//!         password: None,
//!         instance_url: "https://your-instance.salesforce.com".to_string(),
//!         tenant_id: "...".to_string(),
//!     })
//!     .build()?
//!     .connect()
//!     .await?;
//! # Ok(())
//! # }
//! ```

/// OAuth2 client authentication and connection management.
pub mod client;

/// Salesforce Pub/Sub API for real-time event streaming.
pub mod pubsub {
    /// Pub/Sub context for managing gRPC connections and operations.
    pub mod context;
}
