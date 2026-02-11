//! WebSocket support for live block and heartbeat streaming.
//!
//! Clients connect to `/ws` and receive real-time events:
//! - New blocks as they're created
//! - Heartbeat pool updates
//! - Network stats changes

use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, debug, warn};

use crate::types::{PulseBlock, NetworkStats};

/// Events broadcast to WebSocket clients
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum WsEvent {
    #[serde(rename = "new_block")]
    NewBlock {
        block: PulseBlock,
    },
    #[serde(rename = "stats")]
    Stats {
        stats: NetworkStats,
    },
    #[serde(rename = "heartbeat_count")]
    HeartbeatCount {
        count: usize,
    },
}

/// Broadcaster for WebSocket events
#[derive(Clone)]
pub struct WsBroadcaster {
    sender: broadcast::Sender<WsEvent>,
}

impl WsBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcast an event to all connected clients
    pub fn broadcast(&self, event: WsEvent) {
        // Ignore error (no receivers connected)
        let _ = self.sender.send(event);
    }

    /// Get a new receiver
    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.sender.subscribe()
    }

    /// Number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(broadcaster): State<Arc<WsBroadcaster>>,
) -> impl IntoResponse {
    let count = broadcaster.subscriber_count() + 1;
    info!("🔌 WebSocket client connecting (total: {})", count);
    
    ws.on_upgrade(move |socket| handle_ws_connection(socket, broadcaster))
}

/// Handle an individual WebSocket connection
async fn handle_ws_connection(socket: WebSocket, broadcaster: Arc<WsBroadcaster>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut rx = broadcaster.subscribe();

    // Send events to client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    if ws_sender.send(Message::Text(json.into())).await.is_err() {
                        break; // Client disconnected
                    }
                }
                Err(e) => {
                    warn!("Failed to serialize WS event: {}", e);
                }
            }
        }
    });

    // Read from client (handle pings/close, ignore other messages)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(_) => {
                    debug!("WS ping received");
                    // Pong is auto-handled by axum
                }
                _ => {} // Ignore client messages for now
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    info!("🔌 WebSocket client disconnected (remaining: {})", 
        broadcaster.subscriber_count().saturating_sub(1));
}
