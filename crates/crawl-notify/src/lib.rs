//! crawl-notify: Implements org.freedesktop.Notifications on the session bus.
//!
//! crawl-daemon becomes the notification daemon — no mako or dunst needed.
//! All notifications are stored in-memory and broadcast as CrawlEvents.
//! Quickshell reads from the event stream and renders however it wants.

use crawl_ipc::{
    events::{CrawlEvent, NotifyEvent},
    types::{Notification, NotificationAction, Urgency},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, info};
use zbus::{interface, ConnectionBuilder, SignalContext};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Register as the system notification daemon (org.freedesktop.Notifications)
    pub replace_daemon: bool,
    /// Default notification timeout when app sends -1 (ms)
    pub default_timeout_ms: i32,
    /// Maximum notifications to keep in the in-memory store
    pub max_store: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            replace_daemon: true,
            default_timeout_ms: 5000,
            max_store: 200,
        }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("notification not found: {0}")]
    NotFound(u32),
}

// ── Notification store ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct NotifyStore {
    inner: Arc<Mutex<NotifyStoreInner>>,
}

struct NotifyStoreInner {
    notifications: HashMap<u32, Notification>,
    next_id: u32,
    max_store: usize,
}

impl NotifyStore {
    pub fn new(max_store: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(NotifyStoreInner {
                notifications: HashMap::new(),
                next_id: 1,
                max_store,
            })),
        }
    }

    pub fn insert(&self, mut notif: Notification) -> u32 {
        let mut inner = self.inner.lock().unwrap();
        let id = if notif.id == 0 {
            let id = inner.next_id;
            inner.next_id += 1;
            id
        } else {
            notif.id
        };
        notif.id = id;

        // Evict oldest if over capacity
        if inner.notifications.len() >= inner.max_store
            && let Some(&oldest) = inner.notifications.keys().min()
        {
            inner.notifications.remove(&oldest);
        }

        inner.notifications.insert(id, notif);
        id
    }

    pub fn remove(&self, id: u32) -> Option<Notification> {
        self.inner.lock().unwrap().notifications.remove(&id)
    }

    pub fn list(&self) -> Vec<Notification> {
        let inner = self.inner.lock().unwrap();
        let mut v: Vec<Notification> = inner.notifications.values().cloned().collect();
        v.sort_by_key(|n| n.id);
        v
    }
}

// ── D-Bus interface ───────────────────────────────────────────────────────────

struct NotificationServer {
    store: NotifyStore,
    tx: broadcast::Sender<CrawlEvent>,
    default_timeout_ms: i32,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    /// Called by apps to send a notification. Returns the assigned notification ID.
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let urgency = match hints
            .get("urgency")
            .and_then(|v| v.downcast_ref::<u8>().ok())
        {
            Some(0) => Urgency::Low,
            Some(2) => Urgency::Critical,
            Some(_) => Urgency::Normal,
            None => Urgency::Normal,
        };

        let parsed_actions: Vec<NotificationAction> = actions
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    Some(NotificationAction { key: chunk[0].clone(), label: chunk[1].clone() })
                } else { None }
            })
            .collect();

        let timeout = if expire_timeout == -1 { self.default_timeout_ms } else { expire_timeout };
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let notif = Notification {
            id: replaces_id,
            app_name: app_name.to_string(),
            summary: summary.to_string(),
            body: body.to_string(),
            icon: app_icon.to_string(),
            urgency,
            actions: parsed_actions,
            expire_timeout_ms: timeout,
            timestamp_ms: now_ms,
        };

        let is_replace = replaces_id != 0 && self.store.remove(replaces_id).is_some();
        let id = self.store.insert(notif.clone());

        debug!(id, app = app_name, summary, "notification received");

        let evt = if is_replace {
            NotifyEvent::Replaced { notification: Notification { id, ..notif } }
        } else {
            NotifyEvent::New { notification: Notification { id, ..notif } }
        };
        let _ = self.tx.send(CrawlEvent::Notify(evt));

        id
    }

    async fn close_notification(&mut self, id: u32) {
        if self.store.remove(id).is_some() {
            let _ = self.tx.send(CrawlEvent::Notify(NotifyEvent::Closed { id, reason: 3 }));
        }
    }

    async fn get_capabilities(&self) -> Vec<String> {
        vec![
            "body".into(),
            "body-markup".into(),
            "actions".into(),
            "icon-static".into(),
            "persistence".into(),
        ]
    }

    async fn get_server_information(&self) -> (String, String, String, String) {
        (
            "crawl".into(),
            "crawl-notify".into(),
            env!("CARGO_PKG_VERSION").into(),
            "1.2".into(), // spec version
        )
    }

    #[zbus(signal)]
    async fn notification_closed(ctxt: &SignalContext<'_>, id: u32, reason: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(ctxt: &SignalContext<'_>, id: u32, action_key: String) -> zbus::Result<()>;
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-notify starting (replace_daemon={})", cfg.replace_daemon);

    let store = NotifyStore::new(cfg.max_store);

    let server = NotificationServer {
        store: store.clone(),
        tx: tx.clone(),
        default_timeout_ms: cfg.default_timeout_ms,
    };

    let _conn = ConnectionBuilder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", server)?
        .build()
        .await?;

    info!("crawl-notify: registered org.freedesktop.Notifications on session bus");

    let _ = tx.send(CrawlEvent::Notify(NotifyEvent::Closed { id: 0, reason: 0 })); // sentinel

    std::future::pending::<()>().await;
    Ok(())
}

// ── Public query API ──────────────────────────────────────────────────────────

// Note: The store is owned by the D-Bus server task.
// The HTTP router accesses notifications through a shared Arc<NotifyStore>
// passed via AppState. Wire this in crawl-daemon once implementing handlers.
