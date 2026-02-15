//! Pulse Network Node Library
//! 
//! A Proof-of-Life consensus node for the Pulse Network.
//! 
//! ## Modules
//! 
//! - `types` - Core data structures (Heartbeat, Transaction, Block)
//! - `crypto` - Cryptographic primitives (ECDSA signing/verification)
//! - `consensus` - Proof-of-Life consensus engine
//! - `api` - HTTP API for device communication
//! - `storage` - Persistent chain storage
//! - `network` - P2P networking (channel-based architecture)

pub mod types;
pub mod crypto;
pub mod consensus;
pub mod api;
pub mod storage;
pub mod network;

pub use types::*;
pub use crypto::Keypair;
pub use consensus::{ProofOfLife, ConsensusConfig};
