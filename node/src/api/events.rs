//! Event log for tracking node activity.
//! Ring buffer of recent events for the activity feed.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

const MAX_EVENTS: usize = 200;

/// Types of events the node can emit
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum NodeEvent {
    #[serde(rename = "heartbeat_received")]
    HeartbeatReceived {
        timestamp: u64,
        device_pubkey: String,
        heart_rate: u16,
        weight: f64,
    },
    #[serde(rename = "block_created")]
    BlockCreated {
        timestamp: u64,
        index: u64,
        block_hash: String,
        n_live: usize,
        total_weight: f64,
        security: f64,
        rewards_distributed: f64,
    },
    #[serde(rename = "transaction_received")]
    TransactionReceived {
        timestamp: u64,
        tx_id: String,
        sender: String,
        recipient: String,
        amount: f64,
    },
    #[serde(rename = "node_started")]
    NodeStarted {
        timestamp: u64,
        version: String,
        chain_height: u64,
    },
}

impl NodeEvent {
    pub fn timestamp(&self) -> u64 {
        match self {
            NodeEvent::HeartbeatReceived { timestamp, .. } => *timestamp,
            NodeEvent::BlockCreated { timestamp, .. } => *timestamp,
            NodeEvent::TransactionReceived { timestamp, .. } => *timestamp,
            NodeEvent::NodeStarted { timestamp, .. } => *timestamp,
        }
    }
}

/// Thread-safe event log with ring buffer
#[derive(Clone)]
pub struct EventLog {
    events: Arc<RwLock<VecDeque<NodeEvent>>>,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_EVENTS))),
        }
    }

    /// Push an event to the log
    pub async fn push(&self, event: NodeEvent) {
        let mut events = self.events.write().await;
        if events.len() >= MAX_EVENTS {
            events.pop_front();
        }
        events.push_back(event);
    }

    /// Get the latest N events (newest first)
    pub async fn latest(&self, limit: usize) -> Vec<NodeEvent> {
        let events = self.events.read().await;
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Get events since a given timestamp
    pub async fn since(&self, timestamp: u64) -> Vec<NodeEvent> {
        let events = self.events.read().await;
        events.iter()
            .filter(|e| e.timestamp() > timestamp)
            .cloned()
            .collect()
    }
}
