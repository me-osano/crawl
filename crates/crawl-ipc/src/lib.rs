/// crawl-ipc: Shared types, event models, and error envelope.
/// No system dependencies — safe to use in any crate including future QML bridges.
pub mod error;
pub mod events;
pub mod types;
pub mod socket;
pub mod protocol;
pub mod client;
pub mod server;
pub mod subscription;

pub use error::{CrawlError, CrawlResult, ErrorEnvelope};
pub use events::CrawlEvent;
pub use protocol::{Request, Response, Error, error_code, now_ms, EventMessage};
pub use socket::{IpcConnection, bind_socket, connect_socket, default_socket_path};
pub use client::CrawlClient;
pub use subscription::EventSubscription;
pub use server::{IpcServer, RequestDispatcher, EventSender, EventReceiver};
pub use subscription::EventSubscription as EventSub;
