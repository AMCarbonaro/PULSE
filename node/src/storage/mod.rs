//! Persistent storage for the Pulse chain using sled embedded database.

use sled::{Db, Tree};
use std::path::Path;
use thiserror::Error;
use tracing::info;

use crate::types::{PulseBlock, Account};

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] sled::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Block not found: {0}")]
    BlockNotFound(u64),
}

/// Persistent storage for the Pulse chain
pub struct Storage {
    db: Db,
    blocks: Tree,
    accounts: Tree,
    metadata: Tree,
}

impl Storage {
    /// Open or create storage at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db = sled::open(path)?;
        let blocks = db.open_tree("blocks")?;
        let accounts = db.open_tree("accounts")?;
        let metadata = db.open_tree("metadata")?;
        
        info!("💾 Storage opened");
        
        Ok(Self { db, blocks, accounts, metadata })
    }
    
    /// Save a block
    pub fn save_block(&self, block: &PulseBlock) -> Result<(), StorageError> {
        let key = block.index.to_be_bytes();
        let value = serde_json::to_vec(block)?;
        self.blocks.insert(key, value)?;
        
        // Update chain height
        self.metadata.insert("chain_height", &block.index.to_be_bytes())?;
        
        Ok(())
    }
    
    /// Load a block by index
    pub fn load_block(&self, index: u64) -> Result<PulseBlock, StorageError> {
        let key = index.to_be_bytes();
        let value = self.blocks.get(key)?
            .ok_or(StorageError::BlockNotFound(index))?;
        let block: PulseBlock = serde_json::from_slice(&value)?;
        Ok(block)
    }
    
    /// Load all blocks (for chain reconstruction)
    pub fn load_all_blocks(&self) -> Result<Vec<PulseBlock>, StorageError> {
        let mut blocks = Vec::new();
        
        for result in self.blocks.iter() {
            let (_, value) = result?;
            let block: PulseBlock = serde_json::from_slice(&value)?;
            blocks.push(block);
        }
        
        // Sort by index
        blocks.sort_by_key(|b| b.index);
        
        Ok(blocks)
    }
    
    /// Get chain height
    pub fn chain_height(&self) -> Result<u64, StorageError> {
        match self.metadata.get("chain_height")? {
            Some(bytes) => {
                let arr: [u8; 8] = bytes.as_ref().try_into().unwrap_or([0; 8]);
                Ok(u64::from_be_bytes(arr))
            }
            None => Ok(0),
        }
    }
    
    /// Save account state
    pub fn save_account(&self, account: &Account) -> Result<(), StorageError> {
        let value = serde_json::to_vec(account)?;
        self.accounts.insert(account.pubkey.as_bytes(), value)?;
        Ok(())
    }
    
    /// Load account state
    pub fn load_account(&self, pubkey: &str) -> Result<Option<Account>, StorageError> {
        match self.accounts.get(pubkey.as_bytes())? {
            Some(value) => {
                let account: Account = serde_json::from_slice(&value)?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }
    
    /// Load all accounts
    pub fn load_all_accounts(&self) -> Result<Vec<Account>, StorageError> {
        let mut accounts = Vec::new();
        
        for result in self.accounts.iter() {
            let (_, value) = result?;
            let account: Account = serde_json::from_slice(&value)?;
            accounts.push(account);
        }
        
        Ok(accounts)
    }
    
    /// Flush to disk
    pub fn flush(&self) -> Result<(), StorageError> {
        self.db.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_storage_roundtrip() {
        let dir = tempdir().unwrap();
        let storage = Storage::open(dir.path()).unwrap();
        
        let block = PulseBlock {
            index: 1,
            timestamp: 12345,
            previous_hash: "abc".to_string(),
            heartbeats: vec![],
            transactions: vec![],
            n_live: 0,
            total_weight: 0.0,
            security: 0.0,
            bio_entropy: "0".repeat(64),
            block_hash: "xyz".to_string(),
        };
        
        storage.save_block(&block).unwrap();
        let loaded = storage.load_block(1).unwrap();
        
        assert_eq!(loaded.index, block.index);
        assert_eq!(loaded.block_hash, block.block_hash);
    }
}
