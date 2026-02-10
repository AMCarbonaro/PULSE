//! Proof-of-Life consensus engine for the Pulse Network.

use crate::crypto::{verify_signature, CryptoError};
use crate::types::{Heartbeat, PulseBlock, Transaction, Account};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{info, warn, debug};

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
    #[error("Crypto error: {0}")]
    Crypto(#[from] CryptoError),
}

/// Configuration for the consensus engine
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Minimum number of live participants to create a block
    pub n_threshold: usize,
    /// Block interval in milliseconds
    pub block_interval_ms: u64,
    /// Base reward per block
    pub reward_per_block: f64,
    /// Maximum heartbeat age in milliseconds
    pub max_heartbeat_age_ms: u64,
    /// Fork probability constant (k)
    pub fork_constant: f64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            n_threshold: 1,
            block_interval_ms: 5000,
            reward_per_block: 100.0,
            max_heartbeat_age_ms: 30000,
            fork_constant: 0.5,
        }
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
}

impl ProofOfLife {
    /// Create a new consensus engine with genesis block
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
            block_hash: String::new(),
        };
        block.block_hash = block.compute_hash();
        block
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
        
        // 4. Add to pool (update if already present)
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
        
        // Calculate metrics
        let heartbeats: Vec<Heartbeat> = self.heartbeat_pool.values().cloned().collect();
        let total_weight: f64 = heartbeats.iter().map(|h| h.weight()).sum();
        let security = total_weight;
        let fork_prob = (-self.config.fork_constant * security).exp();
        
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
            block_hash: String::new(),
        };
        block.block_hash = block.compute_hash();
        
        info!("\n💓 PULSE BLOCK #{}", block.index);
        info!("   Hash: {}...", &block.block_hash[..16]);
        info!("   Live participants: {}", n_live);
        info!("   Total weight: {:.4}", total_weight);
        info!("   Security (S): {:.4}", security);
        info!("   Fork probability: {:.6}", fork_prob);
        
        // Distribute rewards
        if total_weight > 0.0 {
            for hb in &heartbeats {
                let reward = (hb.weight() / total_weight) * self.config.reward_per_block;
                
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
                
                info!("   💰 {}... earned {:.4} PULSE", &hb.device_pubkey[..8], reward);
            }
        }
        
        // Process transactions
        for tx in &self.tx_pool {
            if let Some(sender) = self.accounts.get_mut(&tx.sender_pubkey) {
                sender.balance -= tx.amount;
            }
            
            let recipient = self.accounts
                .entry(tx.recipient_pubkey.clone())
                .or_insert_with(|| Account {
                    pubkey: tx.recipient_pubkey.clone(),
                    ..Default::default()
                });
            recipient.balance += tx.amount;
            
            info!("   📤 TX: {}... → {}... ({} PULSE)",
                &tx.sender_pubkey[..8], &tx.recipient_pubkey[..8], tx.amount);
        }
        
        // Commit block
        self.chain.push(block.clone());
        
        // Clear pools
        self.heartbeat_pool.clear();
        self.tx_pool.clear();
        
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
        crate::types::NetworkStats {
            chain_length: self.chain.len() as u64,
            total_minted: self.total_minted,
            active_accounts: self.accounts.len(),
            current_tps: 0.0, // TODO: calculate from recent blocks
            avg_block_time: self.config.block_interval_ms as f64 / 1000.0,
            total_security: self.chain.iter().map(|b| b.security).sum(),
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
}
