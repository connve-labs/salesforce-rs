#![allow(clippy::doc_lazy_continuation)]

pub mod eventbus {
    pub mod v1 {
        include!("eventbus.v1.rs");
    }
    pub const ENDPOINT: &str = "https://api.pubsub.salesforce.com";
    pub const DE_ENDPOINT: &str = "https://api.deu.pubsub.salesforce.com";
}
