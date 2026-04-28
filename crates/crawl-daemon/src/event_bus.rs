//! Central event bus for crawl-daemon.
//! Provides pub/sub event distribution using broadcast channel.

use crawl_ipc::CrawlEvent;
use tokio::sync::broadcast;

/// Central event bus that distributes events to subscribers.
/// Services publish events; IPC server and other services subscribe.
pub struct EventBus {
    tx: broadcast::Sender<CrawlEvent>,
}

impl EventBus {
    /// Create a new event bus with given capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all subscribers.
    pub fn publish(&self, event: CrawlEvent) {
        // Ignore send errors (no active subscribers is fine)
        let _ = self.tx.send(event);
    }

    #[allow(dead_code)]
    /// Subscribe to events.
    pub fn subscribe(&self) -> broadcast::Receiver<CrawlEvent> {
        self.tx.subscribe()
    }

    /// Get the sender for direct use.
    pub fn sender(&self) -> broadcast::Sender<CrawlEvent> {
        self.tx.clone()
    }

    #[allow(dead_code)]
    /// Approximate number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}
