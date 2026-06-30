use std::collections::HashMap;

pub type Metadata = HashMap<String, serde_json::Value>;
pub type Timestamp = chrono::DateTime<chrono::Utc>;

mod error;
pub use error::Error;

mod client;
pub use client::{Client, ClientBuilder, PRODUCTION_URL, SANDBOX_URL};

pub use reqwest::Method;

mod pagination;
pub use futures::{Stream, StreamExt, TryStreamExt};
pub use pagination::{paginate, Page};

pub mod webhooks;
pub use webhooks::WebhookError;
