//! Proof-of-Life consensus engine for the Pulse Network.

pub mod biometrics;

use crate::crypto::{verify_signature, CryptoError};
use crate::storage::Storage;
use crate::types::{Heartbeat, PulseBlock, Transaction, Account};
use biometrics::BiometricValidator;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{info, warn, debug, error};

#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Invalid heartbeat signature")]
    InvalidHeartbeatSignature,
    #[error("Stale heartbeat (too old)")]
    StaleHeartbeat,
    #[error("Invalid heart rate: {0}")]
    InvalidHeartRate(u16),
    #[error("Insufficient live participants: {0}/{1}")]
    InsufficientParticipants(usize, usize),
    #[error("Invalid transaction signature")]
    InvalidTransactionSignature,
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Sender not pulsing")]
    SenderNotPulsing,
    #[error("Biometric validation failed: {0}")]
    BiometricValidationFailed(String),
    #[error("Crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::StorageError),
}

/// Configuration for the consensus engine
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Minimum number of live participants to create a block
    pub n_threshold: usize,
    /// Block interval in milliseconds
    pub block_interval_ms: u64,
    /// Initial base reward per block (before halving)
    pub initial_reward_per_block: f64,
    /// Maximum heartbeat age in milliseconds
    pub max_heartbeat_age_ms: u64,
    /// Fork probability constant (k)
    pub fork_constant: f64,
    /// Halving interval: reward halves every N blocks
    /// Models biological constraint — as network matures, new supply slows
    pub halving_interval: u64,
    /// Minimum reward per block (floor — never goes below this)
    pub min_reward_per_block: f64,
    /// Smoothing window: average inflation over last N blocks to prevent spikes
    pub inflation_smoothing_window: usize,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            n_threshold: 1,
            block_interval_ms: 5000,
            initial_reward_per_block: 100.0,
            max_heartbeat_age_ms: 30000,
            fork_constant: 0.5,
            // Halving every 210,000 blocks (~12 days at 5s intervals)
            // Inspired by Bitcoin's model but on a faster cycle since blocks are faster
            halving_interval: 210_000,
            min_reward_per_block: 0.01,
            inflation_smoothing_window: 100,
        }
    }
}

impl ConsensusConfig {
    /// Calculate the block reward at a given block height, applying halvings.
    /// R(h) = initial_reward / 2^(h / halving_interval)
    /// Clamped to min_reward_per_block.
    pub fn reward_at_height(&self, block_height: u64) -> f64 {
        if self.halving_interval == 0 {
            return self.initial_reward_per_block;
        }
        let halvings = block_height / self.halving_interval;
        // After 64 halvings the reward is effectively 0
        if halvings >= 64 {
            return self.min_reward_per_block;
        }
        let reward = self.initial_reward_per_block / (2u64.pow(halvings as u32) as f64);
        reward.max(self.min_reward_per_block)
    }
}

/// The Proof-of-Life consensus engine
pub struct ProofOfLife {
    config: ConsensusConfig,
    /// Current chain
    chain: Vec<PulseBlock>,
    /// Pool of verified heartbeats awaiting block inclusion
    heartbeat_pool: HashMap<String, Heartbeat>, // pubkey -> heartbeat
    /// Pool of pending transactions
    tx_pool: Vec<Transaction>,
    /// Account balances
    accounts: HashMap<String, Account>,
    /// Total tokens minted
    total_minted: f64,
    /// Persistent storage (optional — None means in-memory only)
    storage: Option<Arc<Storage>>,
    /// Tracks when each device first started pulsing in current session (pubkey -> timestamp_ms)
    /// Used for continuity factor (γ·Δt_i)
    continuity_start: HashMap<String, u64>,
    /// Tracks last seen heartbeat hash per pubkey to prevent duplicate submissions
    last_heartbeat_hash: HashMap<String, String>,
    /// Cumulative chain weight (sum of all block security values)
    /// Used for fork resolution: heaviest chain wins
    cumulative_weight: f64,
    /// Biometric validator for sensor spoofing detection
    biometric_validator: BiometricValidator,
}

impl ProofOfLife {
    /// Create a new consensus engine with genesis block (in-memory only)
    pub fn new(config: ConsensusConfig) -> Self {
        let genesis = Self::create_genesis_block();
        info!("🌱 Genesis block created: {}...", &genesis.block_hash[..16]);
        
        Self {
            config,
            chain: vec![genesis],
            heartbeat_pool: HashMap::new(),
            tx_pool: Vec::new(),
            accounts: HashMap::new(),
            total_minted: 0.0,
            storage: None,
            continuity_start: HashMap::new(),
            last_heartbeat_hash: HashMap::new(),
            cumulative_weight: 0.0,
            biometric_validator: BiometricValidator::new(),
        }
    }

    /// Create a new consensus engine with persistent storage.
    /// Loads existing chain from disk if present, otherwise creates genesis.
    pub fn with_storage(config: ConsensusConfig, storage: Arc<Storage>) -> Result<Self, ConsensusError> {
        // Try to load existing chain
        let stored_blocks = storage.load_all_blocks()?;
        let stored_accounts = storage.load_all_accounts()?;
        
        if !stored_blocks.is_empty() {
            // Reconstruct from storage
            let chain_height = stored_blocks.last().map(|b| b.index).unwrap_or(0);
            
            // Rebuild accounts map
            let mut accounts = HashMap::new();
            for account in stored_accounts {
                accounts.insert(account.pubkey.clone(), account);
            }
            
            // Calculate total minted from accounts
            let total_minted: f64 = accounts.values().map(|a| a.total_earned).sum();
            
            info!("💾 Loaded chain from storage:");
            info!("   Chain height: {}", chain_height);
            info!("   Blocks: {}", stored_blocks.len());
            info!("   Accounts: {}", accounts.len());
            // Calculate cumulative chain weight from stored blocks
            let cumulative_weight: f64 = stored_blocks.iter().map(|b| b.security).sum();
            
            info!("   Total minted: {:.4} PULSE", total_minted);
            info!("   Cumulative weight: {:.4}", cumulative_weight);
            
            Ok(Self {
                config,
                chain: stored_blocks,
                heartbeat_pool: HashMap::new(),
                tx_pool: Vec::new(),
                accounts,
                total_minted,
                storage: Some(storage),
                continuity_start: HashMap::new(),
                last_heartbeat_hash: HashMap::new(),
                cumulative_weight,
                biometric_validator: BiometricValidator::new(),
            })
        } else {
            // Fresh start with genesis
            let genesis = Self::create_genesis_block();
            info!("🌱 Genesis block created: {}...", &genesis.block_hash[..16]);
            
            // Persist genesis block
            if let Err(e) = storage.save_block(&genesis) {
                error!("Failed to save genesis block: {}", e);
            }
            if let Err(e) = storage.flush() {
                error!("Failed to flush storage: {}", e);
            }
            
            Ok(Self {
                config,
                chain: vec![genesis],
                heartbeat_pool: HashMap::new(),
                tx_pool: Vec::new(),
                accounts: HashMap::new(),
                total_minted: 0.0,
                storage: Some(storage),
                continuity_start: HashMap::new(),
                last_heartbeat_hash: HashMap::new(),
                cumulative_weight: 0.0,
            biometric_validator: BiometricValidator::new(),
            })
        }
    }
    
    fn create_genesis_block() -> PulseBlock {
        let mut block = PulseBlock {
            index: 0,
            timestamp: current_time_ms(),
            previous_hash: "0".repeat(64),
            heartbeats: vec![],
            transactions: vec![],
            n_live: 0,
            total_weight: 0.0,
            security: 0.0,
            bio_entropy: "0".repeat(64),
            block_hash: String::new(),
        };
        block.block_hash = block.compute_hash();
        block
    }

    /// Persist a block and its affected accounts to storage
    fn persist_block(&self, block: &PulseBlock, affected_pubkeys: &[String]) {
        if let Some(ref storage) = self.storage {
            // Save block
            if let Err(e) = storage.save_block(block) {
                error!("❌ Failed to persist block #{}: {}", block.index, e);
                return;
            }
            
            // Save affected accounts
            for pubkey in affected_pubkeys {
                if let Some(account) = self.accounts.get(pubkey) {
                    if let Err(e) = storage.save_account(account) {
                        error!("❌ Failed to persist account {}...: {}", &pubkey[..8], e);
                    }
                }
            }
            
            // Flush to disk
            if let Err(e) = storage.flush() {
                error!("❌ Failed to flush storage: {}", e);
            } else {
                debug!("💾 Block #{} persisted to disk", block.index);
            }
        }
    }
    
    /// Verify and add a heartbeat to the pool
    pub fn receive_heartbeat(&mut self, hb: Heartbeat) -> Result<(), ConsensusError> {
        // 1. Verify signature
        let valid = verify_signature(
            &hb.device_pubkey,
            &hb.signable_bytes(),
            &hb.signature,
        )?;
        
        if !valid {
            warn!("❌ Invalid signature from {}...", &hb.device_pubkey[..8]);
            return Err(ConsensusError::InvalidHeartbeatSignature);
        }
        
        // 2. Check timestamp freshness
        let now = current_time_ms();
        if now.saturating_sub(hb.timestamp) > self.config.max_heartbeat_age_ms {
            warn!("❌ Stale heartbeat from {}...", &hb.device_pubkey[..8]);
            return Err(ConsensusError::StaleHeartbeat);
        }
        
        // 3. Validate heart rate range
        if hb.heart_rate < 30 || hb.heart_rate > 220 {
            return Err(ConsensusError::InvalidHeartRate(hb.heart_rate));
        }
        
        // 4. Biometric validation — detect synthetic/spoofed heartbeats
        let bio_result = self.biometric_validator.validate(
            &hb.device_pubkey,
            hb.heart_rate,
            hb.motion.magnitude(),
            hb.temperature,
        );
        
        if !bio_result.is_valid {
            let reason = bio_result.reason.unwrap_or_else(|| "Unknown".to_string());
            warn!("🚨 Biometric validation failed for {}...: {}", &hb.device_pubkey[..8], reason);
            return Err(ConsensusError::BiometricValidationFailed(reason));
        }
        
        // 5. Duplicate check — reject identical heartbeat data resubmission
        // (renumbered after adding biometric check above)
        let hb_hash = crate::crypto::hash_sha256(&hb.signable_bytes());
        if let Some(last_hash) = self.last_heartbeat_hash.get(&hb.device_pubkey) {
            if *last_hash == hb_hash {
                warn!("❌ Duplicate heartbeat from {}...", &hb.device_pubkey[..8]);
                return Err(ConsensusError::StaleHeartbeat);
            }
        }
        self.last_heartbeat_hash.insert(hb.device_pubkey.clone(), hb_hash);
        
        // 5. Track continuity — record when this device first started pulsing
        let now = current_time_ms();
        self.continuity_start
            .entry(hb.device_pubkey.clone())
            .or_insert(now);
        
        // 6. Add to pool (update if already present)
        debug!("✅ Heartbeat verified: {}... HR={} W={:.3}", 
            &hb.device_pubkey[..8], hb.heart_rate, hb.weight());
        self.heartbeat_pool.insert(hb.device_pubkey.clone(), hb);
        
        Ok(())
    }
    
    /// Verify and add a transaction to the pool
    pub fn receive_transaction(&mut self, tx: Transaction) -> Result<(), ConsensusError> {
        // 1. Verify signature
        let valid = verify_signature(
            &tx.sender_pubkey,
            &tx.signable_bytes(),
            &tx.signature,
        )?;
        
        if !valid {
            return Err(ConsensusError::InvalidTransactionSignature);
        }
        
        // 2. Check sender balance
        let balance = self.accounts
            .get(&tx.sender_pubkey)
            .map(|a| a.balance)
            .unwrap_or(0.0);
        
        if balance < tx.amount {
            return Err(ConsensusError::InsufficientBalance);
        }
        
        // 3. Check sender is actively pulsing
        if !self.heartbeat_pool.contains_key(&tx.sender_pubkey) {
            return Err(ConsensusError::SenderNotPulsing);
        }
        
        debug!("📨 Transaction queued: {}... → {}... ({} PULSE)",
            &tx.sender_pubkey[..8], &tx.recipient_pubkey[..8], tx.amount);
        self.tx_pool.push(tx);
        
        Ok(())
    }
    
    /// Attempt to create a new block
    pub fn try_create_block(&mut self) -> Result<Option<PulseBlock>, ConsensusError> {
        let n_live = self.heartbeat_pool.len();
        
        // Check threshold
        if n_live < self.config.n_threshold {
            debug!("⏳ Waiting for heartbeats: {}/{}", n_live, self.config.n_threshold);
            return Ok(None);
        }
        
        // Calculate metrics with proper continuity factors
        let now = current_time_ms();
        let heartbeats: Vec<Heartbeat> = self.heartbeat_pool.values().cloned().collect();
        
        // Calculate continuity-weighted contributions
        // Continuity factor: time pulsing / max_continuity_window (5 minutes)
        const MAX_CONTINUITY_MS: f64 = 300_000.0; // 5 minutes for full continuity credit
        
        // Pre-compute weights with continuity so we use the SAME values
        // for both total_weight and per-participant rewards (mathematical consistency)
        let weighted_heartbeats: Vec<(Heartbeat, f64)> = heartbeats.iter().map(|h| {
            let start = self.continuity_start
                .get(&h.device_pubkey)
                .copied()
                .unwrap_or(now);
            let duration_ms = now.saturating_sub(start) as f64;
            let continuity = (duration_ms / MAX_CONTINUITY_MS).min(1.0);
            let w = h.weight_with_continuity(continuity);
            (h.clone(), w)
        }).collect();
        
        let total_weight: f64 = weighted_heartbeats.iter().map(|(_, w)| w).sum();
        
        let security = total_weight;
        
        // Adaptive fork constant: scales with network size
        // Small network (1-10 participants): k=2.0 (need strong per-participant security)
        // Medium (10-100): k=0.5
        // Large (100+): k=0.1
        // Global (1M+): k=0.000001
        // Formula: k = base_k / ln(1 + n_live), clamped
        let adaptive_k = if n_live <= 1 {
            2.0
        } else {
            (self.config.fork_constant / (1.0 + n_live as f64).ln()).max(0.000001)
        };
        let fork_prob = (-adaptive_k * security).exp();
        
        // Extract biometric entropy from all active devices
        let bio_entropy_bytes = self.biometric_validator.aggregate_entropy();
        let bio_entropy = hex::encode(&bio_entropy_bytes);
        
        // Create block
        let previous = self.chain.last().unwrap();
        let mut block = PulseBlock {
            index: previous.index + 1,
            timestamp: current_time_ms(),
            previous_hash: previous.block_hash.clone(),
            heartbeats: heartbeats.clone(),
            transactions: self.tx_pool.clone(),
            n_live,
            total_weight,
            security,
            bio_entropy,
            block_hash: String::new(),
        };
        block.block_hash = block.compute_hash();
        
        info!("\n💓 PULSE BLOCK #{}", block.index);
        info!("   Hash: {}...", &block.block_hash[..16]);
        info!("   Live participants: {}", n_live);
        info!("   Total weight: {:.4}", total_weight);
        info!("   Security (S): {:.4}", security);
        info!("   Fork probability: {:.6}", fork_prob);
        
        // Track affected accounts for persistence
        let mut affected_pubkeys: Vec<String> = Vec::new();
        
        // Calculate block reward with halving schedule
        let block_reward = self.config.reward_at_height(block.index);
        
        info!("   Block reward: {:.4} PULSE (halving epoch {})", 
            block_reward, block.index / self.config.halving_interval.max(1));
        
        // Distribute rewards using the SAME pre-computed weights
        if total_weight > 0.0 {
            for (hb, w_i) in &weighted_heartbeats {
                let reward = (w_i / total_weight) * block_reward;
                
                let account = self.accounts
                    .entry(hb.device_pubkey.clone())
                    .or_insert_with(|| Account {
                        pubkey: hb.device_pubkey.clone(),
                        ..Default::default()
                    });
                
                account.balance += reward;
                account.total_earned += reward;
                account.last_heartbeat = hb.timestamp;
                account.blocks_participated += 1;
                
                self.total_minted += reward;
                affected_pubkeys.push(hb.device_pubkey.clone());
                
                info!("   💰 {}... earned {:.4} PULSE", &hb.device_pubkey[..8], reward);
            }
        }
        
        // Process transactions
        for tx in &self.tx_pool {
            if let Some(sender) = self.accounts.get_mut(&tx.sender_pubkey) {
                sender.balance -= tx.amount;
                affected_pubkeys.push(tx.sender_pubkey.clone());
            }
            
            let recipient = self.accounts
                .entry(tx.recipient_pubkey.clone())
                .or_insert_with(|| Account {
                    pubkey: tx.recipient_pubkey.clone(),
                    ..Default::default()
                });
            recipient.balance += tx.amount;
            affected_pubkeys.push(tx.recipient_pubkey.clone());
            
            info!("   📤 TX: {}... → {}... ({} PULSE)",
                &tx.sender_pubkey[..8], &tx.recipient_pubkey[..8], tx.amount);
        }
        
        // Commit block to chain
        self.chain.push(block.clone());
        
        // Update cumulative chain weight (for fork resolution)
        self.cumulative_weight += security;
        
        // Persist to storage
        self.persist_block(&block, &affected_pubkeys);
        
        // Clear pools (but keep continuity tracking for devices that keep pulsing)
        self.heartbeat_pool.clear();
        self.tx_pool.clear();
        
        // Note: continuity_start is NOT cleared — devices that keep pulsing
        // accumulate continuity across blocks. Entries are cleaned up when
        // a device stops sending heartbeats (via periodic cleanup, not here).
        
        Ok(Some(block))
    }
    
    /// Get current chain height
    pub fn chain_height(&self) -> u64 {
        self.chain.last().map(|b| b.index).unwrap_or(0)
    }
    
    /// Get the latest block
    pub fn latest_block(&self) -> Option<&PulseBlock> {
        self.chain.last()
    }

    /// Get the full chain (genesis to tip) for read-only API use
    pub fn get_blocks(&self) -> Vec<PulseBlock> {
        self.chain.clone()
    }

    /// Get a block by index (for "jump to block" etc.)
    pub fn get_block_by_index(&self, index: u64) -> Option<PulseBlock> {
        self.chain.iter().find(|b| b.index == index).cloned()
    }

    /// Get account balance
    pub fn get_balance(&self, pubkey: &str) -> f64 {
        self.accounts.get(pubkey).map(|a| a.balance).unwrap_or(0.0)
    }
    
    /// Get all accounts
    pub fn get_accounts(&self) -> &HashMap<String, Account> {
        &self.accounts
    }
    
    /// Get network stats
    pub fn get_stats(&self) -> crate::types::NetworkStats {
        let height = self.chain_height();
        let current_reward = self.config.reward_at_height(height);
        let halving_epoch = if self.config.halving_interval > 0 {
            height / self.config.halving_interval
        } else {
            0
        };
        let inflation_rate = if self.total_minted > 0.0 {
            current_reward / self.total_minted
        } else {
            0.0
        };
        
        crate::types::NetworkStats {
            chain_length: self.chain.len() as u64,
            total_minted: self.total_minted,
            active_accounts: self.accounts.len(),
            current_tps: 0.0, // TODO: calculate from recent blocks
            avg_block_time: self.config.block_interval_ms as f64 / 1000.0,
            total_security: self.chain.iter().map(|b| b.security).sum(),
            current_block_reward: current_reward,
            halving_epoch,
            cumulative_weight: self.cumulative_weight,
            inflation_rate,
        }
    }
    
    /// Get number of heartbeats in pool
    pub fn heartbeat_pool_size(&self) -> usize {
        self.heartbeat_pool.len()
    }
    
    /// Check if a pubkey is currently pulsing
    pub fn is_pulsing(&self, pubkey: &str) -> bool {
        self.heartbeat_pool.contains_key(pubkey)
    }
    
    /// Get cumulative chain weight (for fork resolution: heaviest chain wins)
    pub fn cumulative_chain_weight(&self) -> f64 {
        self.cumulative_weight
    }
    
    /// Clean up continuity tracking for devices that haven't pulsed recently.
    /// Call this periodically (e.g., every few block intervals).
    pub fn cleanup_stale_continuity(&mut self) {
        let now = current_time_ms();
        let max_age = self.config.max_heartbeat_age_ms * 2; // 2x heartbeat timeout
        
        self.continuity_start.retain(|pubkey, start| {
            let age = now.saturating_sub(*start);
            // Keep if device pulsed recently or started recently
            self.heartbeat_pool.contains_key(pubkey) || age < max_age
        });
        
        // Also clean up stale heartbeat hashes
        self.last_heartbeat_hash.retain(|pubkey, _| {
            self.continuity_start.contains_key(pubkey)
        });
    }
}

/// Get current time in milliseconds
fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Keypair;
    use crate::types::Motion;
    
    fn create_test_heartbeat(keypair: &Keypair) -> Heartbeat {
        let mut hb = Heartbeat {
            timestamp: current_time_ms(),
            heart_rate: 72,
            motion: Motion { x: 0.1, y: 0.1, z: 0.05 },
            temperature: 36.7,
            device_pubkey: keypair.public_key_hex(),
            signature: String::new(),
        };
        hb.signature = keypair.sign(&hb.signable_bytes());
        hb
    }
    
    #[test]
    fn test_receive_valid_heartbeat() {
        let mut pol = ProofOfLife::new(ConsensusConfig::default());
        let kp = Keypair::generate();
        let hb = create_test_heartbeat(&kp);
        
        assert!(pol.receive_heartbeat(hb).is_ok());
        assert_eq!(pol.heartbeat_pool_size(), 1);
    }
    
    #[test]
    fn test_create_block() {
        let mut pol = ProofOfLife::new(ConsensusConfig::default());
        let kp = Keypair::generate();
        let hb = create_test_heartbeat(&kp);
        
        pol.receive_heartbeat(hb).unwrap();
        let block = pol.try_create_block().unwrap();
        
        assert!(block.is_some());
        assert_eq!(pol.chain_height(), 1);
    }

    #[test]
    fn test_weight_normalization() {
        // Verify that weight function outputs are in reasonable [0, 1] range
        let kp = Keypair::generate();
        
        // Resting person: HR=70, minimal motion
        let mut hb_rest = create_test_heartbeat(&kp);
        hb_rest.heart_rate = 70;
        hb_rest.motion = Motion { x: 0.01, y: 0.01, z: 0.01 };
        let w_rest = hb_rest.weight_with_continuity(1.0);
        
        // Active person: HR=150, walking
        let mut hb_active = create_test_heartbeat(&kp);
        hb_active.heart_rate = 150;
        hb_active.motion = Motion { x: 0.3, y: 0.2, z: 0.1 };
        let w_active = hb_active.weight_with_continuity(1.0);
        
        // Extreme: HR=200, running hard
        let mut hb_extreme = create_test_heartbeat(&kp);
        hb_extreme.heart_rate = 200;
        hb_extreme.motion = Motion { x: 1.5, y: 1.0, z: 0.5 };
        let w_extreme = hb_extreme.weight_with_continuity(1.0);
        
        // All weights should be in [0, 1] range
        assert!(w_rest > 0.0 && w_rest <= 1.0, "Rest weight out of range: {}", w_rest);
        assert!(w_active > 0.0 && w_active <= 1.0, "Active weight out of range: {}", w_active);
        assert!(w_extreme > 0.0 && w_extreme <= 1.0, "Extreme weight out of range: {}", w_extreme);
        
        // Active should be higher than resting
        assert!(w_active > w_rest, "Active ({}) should > rest ({})", w_active, w_rest);
        
        // But extreme shouldn't be MUCH higher than active (sigmoid plateau)
        let extreme_ratio = w_extreme / w_active;
        assert!(extreme_ratio < 1.5, "Extreme/active ratio too high: {}", extreme_ratio);
        
        println!("Weight rest={:.4} active={:.4} extreme={:.4} ratio={:.2}", 
            w_rest, w_active, w_extreme, extreme_ratio);
    }
    
    #[test]
    fn test_continuity_affects_weight() {
        let kp = Keypair::generate();
        let hb = create_test_heartbeat(&kp);
        
        // No continuity vs full continuity
        let w_zero = hb.weight_with_continuity(0.0);
        let w_full = hb.weight_with_continuity(1.0);
        
        assert!(w_full > w_zero, "Full continuity ({}) should > zero ({})", w_full, w_zero);
        
        // The difference should be exactly gamma * 1.0 = 0.3
        let diff = w_full - w_zero;
        assert!((diff - 0.3).abs() < 0.001, "Continuity diff should be ~0.3, got {}", diff);
    }
    
    #[test]
    fn test_reward_distribution_proportional() {
        let mut pol = ProofOfLife::new(ConsensusConfig::default());
        
        // Two devices with different activity levels
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        
        let mut hb1 = create_test_heartbeat(&kp1);
        hb1.heart_rate = 70; // resting
        hb1.motion = Motion { x: 0.01, y: 0.01, z: 0.01 };
        hb1.signature = kp1.sign(&hb1.signable_bytes());
        
        let mut hb2 = create_test_heartbeat(&kp2);
        hb2.heart_rate = 140; // active
        hb2.motion = Motion { x: 0.5, y: 0.3, z: 0.2 };
        hb2.signature = kp2.sign(&hb2.signable_bytes());
        
        pol.receive_heartbeat(hb1).unwrap();
        pol.receive_heartbeat(hb2).unwrap();
        pol.try_create_block().unwrap();
        
        let bal1 = pol.get_balance(&kp1.public_key_hex());
        let bal2 = pol.get_balance(&kp2.public_key_hex());
        
        // Total should be reward_per_block (100.0)
        assert!((bal1 + bal2 - 100.0).abs() < 0.001, 
            "Total rewards should be 100, got {}", bal1 + bal2);
        
        // Active person should earn more than resting
        assert!(bal2 > bal1, "Active ({}) should earn more than rest ({})", bal2, bal1);
        
        println!("Rewards: rest={:.4} active={:.4}", bal1, bal2);
    }
    
    #[test]
    fn test_duplicate_heartbeat_rejected() {
        let mut pol = ProofOfLife::new(ConsensusConfig::default());
        let kp = Keypair::generate();
        let hb = create_test_heartbeat(&kp);
        
        // First submission should succeed
        assert!(pol.receive_heartbeat(hb.clone()).is_ok());
        
        // Exact same heartbeat (same data) should be rejected as duplicate
        assert!(pol.receive_heartbeat(hb).is_err());
    }
    
    #[test]
    fn test_cumulative_chain_weight() {
        let mut pol = ProofOfLife::new(ConsensusConfig::default());
        
        assert_eq!(pol.cumulative_chain_weight(), 0.0);
        
        let kp = Keypair::generate();
        
        // Create first block
        let hb1 = create_test_heartbeat(&kp);
        pol.receive_heartbeat(hb1).unwrap();
        pol.try_create_block().unwrap();
        let weight_after_1 = pol.cumulative_chain_weight();
        assert!(weight_after_1 > 0.0, "Cumulative weight should be > 0 after first block");
        
        // Create second block (need fresh heartbeat with different timestamp)
        std::thread::sleep(std::time::Duration::from_millis(10));
        let hb2 = create_test_heartbeat(&kp);
        pol.receive_heartbeat(hb2).unwrap();
        pol.try_create_block().unwrap();
        let weight_after_2 = pol.cumulative_chain_weight();
        
        // Cumulative should grow
        assert!(weight_after_2 > weight_after_1, 
            "Cumulative weight should grow: {} > {}", weight_after_2, weight_after_1);
    }

    #[test]
    fn test_halving_schedule() {
        let config = ConsensusConfig::default();
        
        // Block 0: full reward
        let r0 = config.reward_at_height(0);
        assert_eq!(r0, 100.0);
        
        // Block at first halving: half reward
        let r1 = config.reward_at_height(config.halving_interval);
        assert!((r1 - 50.0).abs() < 0.001, "First halving should give 50, got {}", r1);
        
        // Block at second halving: quarter reward
        let r2 = config.reward_at_height(config.halving_interval * 2);
        assert!((r2 - 25.0).abs() < 0.001, "Second halving should give 25, got {}", r2);
        
        // Block at third halving
        let r3 = config.reward_at_height(config.halving_interval * 3);
        assert!((r3 - 12.5).abs() < 0.001, "Third halving should give 12.5, got {}", r3);
        
        // Very far in the future: should hit minimum
        let r_far = config.reward_at_height(config.halving_interval * 100);
        assert_eq!(r_far, config.min_reward_per_block);
    }
    
    #[test]
    fn test_inflation_decreases_over_time() {
        let config = ConsensusConfig::default();
        
        // Inflation at height 0 vs height 210_000 — should decrease
        let r_early = config.reward_at_height(1000);
        let r_later = config.reward_at_height(config.halving_interval + 1000);
        
        assert!(r_early > r_later, 
            "Later reward ({}) should be less than early ({})", r_later, r_early);
    }

    #[test]
    fn test_storage_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::open(dir.path()).unwrap());
        
        let config = ConsensusConfig::default();
        let mut pol = ProofOfLife::with_storage(config.clone(), storage.clone()).unwrap();
        
        // Create a block
        let kp = Keypair::generate();
        let hb = create_test_heartbeat(&kp);
        pol.receive_heartbeat(hb).unwrap();
        pol.try_create_block().unwrap();
        
        assert_eq!(pol.chain_height(), 1);
        
        // Reconstruct from storage — chain should be restored
        let pol2 = ProofOfLife::with_storage(config, storage).unwrap();
        assert_eq!(pol2.chain_height(), 1);
    }
}
