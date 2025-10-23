# Salesforce Rust SDK

[![Test Suite](https://github.com/connve/salesforce-rs/workflows/test/badge.svg)](https://github.com/connve/salesforce-rs/actions)
[![Security Audit](https://github.com/connve/salesforce-rs/workflows/security-audit/badge.svg)](https://github.com/connve/salesforce-rs/actions)

Unofficial Rust SDK for the Salesforce API with support for OAuth2 authentication and Pub/Sub API.

## Installation

```toml
[dependencies]
salesforce_core = "0.1.0"
```

## Quick Start

### Client Credentials Flow

```rust
use salesforce_core::client::{self, Credentials, AuthFlow};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = client::Builder::new()
        .credentials(Credentials {
            client_id: "your_client_id".to_string(),
            client_secret: Some("your_client_secret".to_string()),
            username: None,
            password: None,
            instance_url: "https://your-instance.salesforce.com".to_string(),
            tenant_id: "your_tenant_id".to_string(),
        })
        .auth_flow(AuthFlow::ClientCredentials)
        .build()?
        .connect()
        .await?;

    Ok(())
}
```

### Username-Password Flow

```rust
use salesforce_core::client::{self, Credentials, AuthFlow};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = client::Builder::new()
        .credentials(Credentials {
            client_id: "your_client_id".to_string(),
            client_secret: Some("your_client_secret".to_string()),
            username: Some("user@example.com".to_string()),
            password: Some("your_password".to_string()),
            instance_url: "https://your-instance.salesforce.com".to_string(),
            tenant_id: "your_tenant_id".to_string(),
        })
        .auth_flow(AuthFlow::UsernamePassword)
        .build()?
        .connect()
        .await?;

    Ok(())
}
```

### Loading Credentials from File

```rust
use salesforce_core::client;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = client::Builder::new()
        .credentials_path(PathBuf::from("credentials.json"))
        .build()?
        .connect()
        .await?;

    Ok(())
}
```

**credentials.json:**
```json
{
  "client_id": "your_client_id",
  "client_secret": "your_client_secret",
  "instance_url": "https://your-instance.salesforce.com",
  "tenant_id": "your_tenant_id"
}
```

## Examples

See [examples](examples/) directory for complete working code.

## Project Structure

```
salesforce-rs/
├── salesforce-core/           # Core SDK with OAuth2 and Pub/Sub support
├── generated/                 # Generated gRPC code for Pub/Sub API
│   └── salesforce_pubsub/v1/
└── examples/                  # Working examples
    └── salesforce-pubsub/
```

## API Support

### Authentication
- OAuth2 Client Credentials Flow
- OAuth2 Username-Password Flow (Resource Owner Password Credentials)

### Pub/Sub API
- Get Topic
- Get Schema
- Subscribe
- Publish
- Managed Subscribe
- Publish Stream

## License

MPL-2.0

---

This is an unofficial SDK and is not affiliated with or endorsed by Salesforce.
