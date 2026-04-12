use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
};
use std::{convert::Infallible, time::Duration};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tracing::warn;

use crate::state::AppState;

/// GET /events — Server-Sent Events stream.
///
/// Every CrawlEvent broadcast by any domain task is serialised to JSON
/// and pushed to connected clients. Quickshell DataStream and the CLI
/// `--watch` flag both consume this endpoint.
///
/// Example event on the wire:
/// ```
/// data: {"domain":"sysmon","data":{"event":"cpu_update","cpu":{...}}}
///
/// data: {"domain":"bluetooth","data":{"event":"device_connected","device":{...}}}
/// ```
pub async fn handler(State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|result| {
            result.ok().and_then(|event| match serde_json::to_string(&event) {
                Ok(data) => Some(Ok::<Event, Infallible>(Event::default().data(data))),
                Err(err) => {
                    warn!(error = %err, "failed to serialize CrawlEvent");
                    None
                }
            })
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
