/// crawl-ipc: Shared types, event models, and error envelope.
/// No system dependencies — safe to use in any crate including future QML bridges.
pub mod error;
pub mod events;
pub mod types;

pub use error::{CrawlError, CrawlResult, ErrorEnvelope};
pub use events::CrawlEvent;
