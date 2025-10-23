# Salesforce Rust SDK

[![Test Suite](https://github.com/connve-labs/salesforce-rs/workflows/test/badge.svg)](https://github.com/connve/salesforce-rs/actions)
[![Security Audit](https://github.com/connve-labs/salesforce-rs/workflows/security-audit/badge.svg)](https://github.com/connve/salesforce-rs/actions)

Unofficial Rust SDK for the Salesforce API with support for OAuth2 authentication and Pub/Sub API.

## Installation

This package is not yet published to crates.io. Install directly from GitHub:

```toml
[dependencies]
salesforce_core = { git = "https://github.com/connve-labs/salesforce-rs" }
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
