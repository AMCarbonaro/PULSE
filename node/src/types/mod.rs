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
    /// Calculate weighted contribution W_i = α·HR_norm + β·M_norm + γ·continuity
    /// 
    /// All components are normalized to [0, 1] range to prevent any single
    /// biometric from dominating. This is critical for fair reward distribution
    /// and preventing gaming (e.g., exercising to inflate weight).
    ///
    /// The continuity factor requires external state (how long this device has
    /// been continuously pulsing), so it's passed as a parameter.
    pub fn weight_with_continuity(&self, continuity_factor: f64) -> f64 {
        const ALPHA: f64 = 0.4;  // Heart rate weight
        const BETA: f64 = 0.3;   // Motion weight  
        const GAMMA: f64 = 0.3;  // Continuity weight
        
        // Normalize heart rate to [0, 1] using sigmoid-like mapping:
        // - 30 BPM (minimum valid) → ~0.0
        // - 70 BPM (resting) → ~0.5
        // - 120 BPM (moderate exercise) → ~0.8
        // - 220 BPM (max valid) → ~1.0
        // This prevents extreme HR from giving disproportionate advantage
        let hr_norm = Self::normalize_heart_rate(self.heart_rate);
        
        // Normalize motion magnitude to [0, 1]:
        // - 0.0 g (stationary) → 0.0
        // - 0.5 g (walking) → ~0.5
        // - 2.0+ g (running/vigorous) → 1.0
        // Capped to prevent accelerometer spoofing from being profitable
        let motion_norm = (self.motion.magnitude() / 2.0).min(1.0);
        
        // Continuity: [0, 1] — how long this device has been continuously pulsing
        // 0.0 = just joined, 1.0 = pulsing for full window (e.g., 5+ minutes)
        let cont_norm = continuity_factor.clamp(0.0, 1.0);
        
        ALPHA * hr_norm + BETA * motion_norm + GAMMA * cont_norm
    }
    
    /// Backward-compatible weight (assumes full continuity)
    pub fn weight(&self) -> f64 {
        self.weight_with_continuity(1.0)
    }
    
    /// Sigmoid-like normalization for heart rate to [0, 1].
    /// Uses a logistic curve centered at 100 BPM (midpoint of valid range).
    /// This ensures:
    ///  - Resting HR (~60-70) gives moderate weight
    ///  - Active HR (~100-150) gives higher weight  
    ///  - Extreme HR (>180) plateaus — no incentive to game via overexertion
    fn normalize_heart_rate(hr: u16) -> f64 {
        let hr = hr as f64;
        // Logistic: 1 / (1 + e^(-k*(x - midpoint)))
        // k=0.04 gives a gentle S-curve across 30-220 BPM range
        // midpoint=100 centers the curve
        1.0 / (1.0 + (-0.04 * (hr - 100.0)).exp())
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
            "security": self.security,
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
