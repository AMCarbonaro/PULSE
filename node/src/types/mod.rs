//! Core data types for the Pulse Network.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Motion vector from device accelerometer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Motion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Motion {
    pub fn magnitude(&self) -> f64 {
        (self.x.powi(2) + self.y.powi(2) + self.z.powi(2)).sqrt()
    }
}

/// A heartbeat packet from a device - the atomic unit of Proof-of-Life
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Unix timestamp in milliseconds
    pub timestamp: u64,
    /// Heart rate in BPM
    pub heart_rate: u16,
    /// Motion vector from accelerometer
    pub motion: Motion,
    /// Body temperature in Celsius
    pub temperature: f32,
    /// Device/user public key (hex-encoded)
    pub device_pubkey: String,
    /// ECDSA signature of the packet (hex-encoded)
    #[serde(default)]
    pub signature: String,
}

impl Heartbeat {
    /// Calculate weighted contribution W_i = α·HR + β·||M|| + γ·continuity
    pub fn weight(&self) -> f64 {
        const ALPHA: f64 = 0.4;  // Heart rate weight
        const BETA: f64 = 0.4;   // Motion weight  
        const GAMMA: f64 = 0.2;  // Continuity weight
        
        // Normalize heart rate around resting (70 BPM)
        let hr_norm = self.heart_rate as f64 / 70.0;
        
        // Normalize motion magnitude
        let motion_norm = (self.motion.magnitude() / 0.5).min(2.0);
        
        // Continuity factor (placeholder - would track gaps)
        let continuity = 1.0;
        
        ALPHA * hr_norm + BETA * motion_norm + GAMMA * continuity
    }
    
    /// Get the signable portion of the heartbeat (excludes signature).
    /// Uses sorted keys for cross-platform compatibility (iOS, Android, Web).
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut map = BTreeMap::new();
        map.insert("device_pubkey", serde_json::to_value(&self.device_pubkey).unwrap());
        map.insert("heart_rate", serde_json::to_value(self.heart_rate).unwrap());
        map.insert("motion", serde_json::to_value(&self.motion).unwrap());
        map.insert("temperature", serde_json::to_value(self.temperature).unwrap());
        map.insert("timestamp", serde_json::to_value(self.timestamp).unwrap());
        serde_json::to_vec(&map).unwrap()
    }
}

/// A pulse-backed transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction ID
    pub tx_id: String,
    /// Sender's public key
    pub sender_pubkey: String,
    /// Recipient's public key
    pub recipient_pubkey: String,
    /// Amount of PULSE tokens
    pub amount: f64,
    /// Unix timestamp in milliseconds
    pub timestamp: u64,
    /// Reference to sender's heartbeat signature (proves life)
    pub heartbeat_signature: String,
    /// Transaction signature
    #[serde(default)]
    pub signature: String,
}

impl Transaction {
    /// Get the signable portion of the transaction (excludes signature).
    /// Uses sorted keys for cross-platform compatibility.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut map = BTreeMap::new();
        map.insert("amount", serde_json::to_value(self.amount).unwrap());
        map.insert("heartbeat_signature", serde_json::to_value(&self.heartbeat_signature).unwrap());
        map.insert("recipient_pubkey", serde_json::to_value(&self.recipient_pubkey).unwrap());
        map.insert("sender_pubkey", serde_json::to_value(&self.sender_pubkey).unwrap());
        map.insert("timestamp", serde_json::to_value(self.timestamp).unwrap());
        map.insert("tx_id", serde_json::to_value(&self.tx_id).unwrap());
        serde_json::to_vec(&map).unwrap()
    }
}

/// A block in the Pulse chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseBlock {
    /// Block index (height)
    pub index: u64,
    /// Unix timestamp in milliseconds
    pub timestamp: u64,
    /// Hash of the previous block
    pub previous_hash: String,
    /// Verified heartbeats in this block
    pub heartbeats: Vec<Heartbeat>,
    /// Transactions in this block
    pub transactions: Vec<Transaction>,
    /// Number of live participants
    pub n_live: usize,
    /// Total weighted contribution
    pub total_weight: f64,
    /// Network security metric (S = Σ W_i)
    pub security: f64,
    /// Block hash
    #[serde(default)]
    pub block_hash: String,
}

impl PulseBlock {
    /// Compute the block hash
    pub fn compute_hash(&self) -> String {
        use sha2::{Sha256, Digest};
        
        let data = serde_json::json!({
            "index": self.index,
            "timestamp": self.timestamp,
            "previous_hash": self.previous_hash,
            "heartbeats": self.heartbeats,
            "transactions": self.transactions,
            "n_live": self.n_live,
            "total_weight": self.total_weight,
        });
        
        let bytes = serde_json::to_vec(&data).unwrap();
        let hash = Sha256::digest(&bytes);
        hex::encode(hash)
    }
    
    /// Calculate fork probability P_fork = e^(-k * S)
    pub fn fork_probability(&self, k: f64) -> f64 {
        (-k * self.security).exp()
    }
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub chain_length: u64,
    pub total_minted: f64,
    pub active_accounts: usize,
    pub current_tps: f64,
    pub avg_block_time: f64,
    pub total_security: f64,
}

/// Account balance and state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    pub pubkey: String,
    pub balance: f64,
    pub last_heartbeat: u64,
    pub total_earned: f64,
    pub blocks_participated: u64,
}
