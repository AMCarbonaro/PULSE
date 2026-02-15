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
    /// Biometric entropy hash — derived from HRV and sensor variability
    /// Provides non-deterministic randomness for the network
    #[serde(default)]
    pub bio_entropy: String,
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
            "bio_entropy": self.bio_entropy,
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
    /// Current block reward (after halvings)
    pub current_block_reward: f64,
    /// Current halving epoch
    pub halving_epoch: u64,
    /// Cumulative chain weight (for fork resolution)
    pub cumulative_weight: f64,
    /// Inflation rate: tokens_per_block / total_supply
    pub inflation_rate: f64,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_motion() -> Motion {
        Motion { x: 0.3, y: 0.4, z: 0.0 }
    }

    fn sample_heartbeat() -> Heartbeat {
        Heartbeat {
            timestamp: 1700000000000,
            heart_rate: 72,
            motion: sample_motion(),
            temperature: 36.6,
            device_pubkey: "aabbccdd".to_string(),
            signature: String::new(),
        }
    }

    #[test]
    fn test_motion_magnitude() {
        let m = Motion { x: 3.0, y: 4.0, z: 0.0 };
        assert!((m.magnitude() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_motion_magnitude_zero() {
        let m = Motion { x: 0.0, y: 0.0, z: 0.0 };
        assert_eq!(m.magnitude(), 0.0);
    }

    #[test]
    fn test_heartbeat_serialization_roundtrip() {
        let hb = sample_heartbeat();
        let json = serde_json::to_string(&hb).unwrap();
        let hb2: Heartbeat = serde_json::from_str(&json).unwrap();
        assert_eq!(hb2.heart_rate, 72);
        assert_eq!(hb2.timestamp, hb.timestamp);
        assert!((hb2.temperature - 36.6).abs() < 0.01);
    }

    #[test]
    fn test_heartbeat_signable_bytes_deterministic() {
        let hb = sample_heartbeat();
        assert_eq!(hb.signable_bytes(), hb.signable_bytes());
    }

    #[test]
    fn test_heartbeat_signable_bytes_excludes_signature() {
        let mut hb = sample_heartbeat();
        let bytes1 = hb.signable_bytes();
        hb.signature = "deadbeef".to_string();
        let bytes2 = hb.signable_bytes();
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_heartbeat_weight_range() {
        let hb = sample_heartbeat();
        let w = hb.weight();
        assert!(w > 0.0 && w <= 1.0, "weight out of range: {}", w);
    }

    #[test]
    fn test_heartbeat_weight_with_continuity_zero() {
        let hb = sample_heartbeat();
        let w0 = hb.weight_with_continuity(0.0);
        let w1 = hb.weight_with_continuity(1.0);
        assert!(w1 > w0);
    }

    #[test]
    fn test_transaction_serialization_roundtrip() {
        let tx = Transaction {
            tx_id: "tx1".to_string(),
            sender_pubkey: "sender".to_string(),
            recipient_pubkey: "recipient".to_string(),
            amount: 42.5,
            timestamp: 1700000000000,
            heartbeat_signature: "sig".to_string(),
            signature: String::new(),
        };
        let json = serde_json::to_string(&tx).unwrap();
        let tx2: Transaction = serde_json::from_str(&json).unwrap();
        assert_eq!(tx2.tx_id, "tx1");
        assert!((tx2.amount - 42.5).abs() < 1e-10);
    }

    #[test]
    fn test_transaction_signable_bytes_excludes_signature() {
        let mut tx = Transaction {
            tx_id: "tx1".to_string(),
            sender_pubkey: "s".to_string(),
            recipient_pubkey: "r".to_string(),
            amount: 10.0,
            timestamp: 100,
            heartbeat_signature: "hs".to_string(),
            signature: String::new(),
        };
        let b1 = tx.signable_bytes();
        tx.signature = "changed".to_string();
        assert_eq!(b1, tx.signable_bytes());
    }

    #[test]
    fn test_block_compute_hash_deterministic() {
        let block = PulseBlock {
            index: 1,
            timestamp: 12345,
            previous_hash: "prev".to_string(),
            heartbeats: vec![],
            transactions: vec![],
            n_live: 0,
            total_weight: 0.0,
            security: 0.0,
            bio_entropy: "00".to_string(),
            block_hash: String::new(),
        };
        assert_eq!(block.compute_hash(), block.compute_hash());
        assert!(!block.compute_hash().is_empty());
    }

    #[test]
    fn test_block_compute_hash_changes_with_data() {
        let b1 = PulseBlock {
            index: 1,
            timestamp: 100,
            previous_hash: "p".to_string(),
            heartbeats: vec![],
            transactions: vec![],
            n_live: 0,
            total_weight: 0.0,
            security: 0.0,
            bio_entropy: String::new(),
            block_hash: String::new(),
        };
        let mut b2 = b1.clone();
        b2.index = 2;
        assert_ne!(b1.compute_hash(), b2.compute_hash());
    }

    #[test]
    fn test_block_serialization_roundtrip() {
        let block = PulseBlock {
            index: 5,
            timestamp: 9999,
            previous_hash: "abc".to_string(),
            heartbeats: vec![sample_heartbeat()],
            transactions: vec![],
            n_live: 1,
            total_weight: 0.5,
            security: 0.5,
            bio_entropy: "ff".to_string(),
            block_hash: "hash".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        let b2: PulseBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(b2.index, 5);
        assert_eq!(b2.heartbeats.len(), 1);
    }

    #[test]
    fn test_fork_probability() {
        let block = PulseBlock {
            index: 1, timestamp: 0, previous_hash: String::new(),
            heartbeats: vec![], transactions: vec![],
            n_live: 5, total_weight: 3.0, security: 3.0,
            bio_entropy: String::new(), block_hash: String::new(),
        };
        let p = block.fork_probability(0.5);
        // e^(-0.5 * 3.0) ≈ 0.2231
        assert!((p - 0.2231).abs() < 0.001);
        // Higher security → lower fork probability
        let block2 = PulseBlock { security: 10.0, ..block };
        assert!(block2.fork_probability(0.5) < p);
    }

    #[test]
    fn test_network_stats_default_fields() {
        let stats = NetworkStats {
            chain_length: 10,
            total_minted: 1000.0,
            active_accounts: 5,
            current_tps: 2.0,
            avg_block_time: 5.0,
            total_security: 50.0,
            current_block_reward: 100.0,
            halving_epoch: 0,
            cumulative_weight: 50.0,
            inflation_rate: 0.1,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let s2: NetworkStats = serde_json::from_str(&json).unwrap();
        assert_eq!(s2.chain_length, 10);
    }

    #[test]
    fn test_account_default() {
        let acc = Account::default();
        assert_eq!(acc.balance, 0.0);
        assert_eq!(acc.blocks_participated, 0);
    }
}
