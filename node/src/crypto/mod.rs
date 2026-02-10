//! Cryptographic operations for the Pulse Network.
//! Uses secp256k1 ECDSA for signing and verification.

use k256::{
    ecdsa::{
        signature::{Signer, Verifier},
        Signature, SigningKey, VerifyingKey,
    },
    SecretKey,
};
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Invalid private key")]
    InvalidPrivateKey,
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Signature verification failed")]
    VerificationFailed,
    #[error("Hex decode error: {0}")]
    HexError(#[from] hex::FromHexError),
}

/// A keypair for device/user identity
#[derive(Clone)]
pub struct Keypair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl Keypair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = *signing_key.verifying_key();
        Self { signing_key, verifying_key }
    }
    
    /// Create keypair from private key hex
    pub fn from_private_key_hex(hex_key: &str) -> Result<Self, CryptoError> {
        let bytes = hex::decode(hex_key)?;
        let secret_key = SecretKey::from_slice(&bytes)
            .map_err(|_| CryptoError::InvalidPrivateKey)?;
        let signing_key = SigningKey::from(secret_key);
        let verifying_key = *signing_key.verifying_key();
        Ok(Self { signing_key, verifying_key })
    }
    
    /// Get private key as hex string
    pub fn private_key_hex(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }
    
    /// Get public key as hex string
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.to_sec1_bytes())
    }
    
    /// Sign data and return hex-encoded signature
    pub fn sign(&self, data: &[u8]) -> String {
        let signature: Signature = self.signing_key.sign(data);
        hex::encode(signature.to_bytes())
    }
}

/// Verify a signature against a public key
pub fn verify_signature(
    public_key_hex: &str,
    data: &[u8],
    signature_hex: &str,
) -> Result<bool, CryptoError> {
    let pubkey_bytes = hex::decode(public_key_hex)?;
    let verifying_key = VerifyingKey::from_sec1_bytes(&pubkey_bytes)
        .map_err(|_| CryptoError::InvalidPublicKey)?;
    
    let sig_bytes = hex::decode(signature_hex)?;
    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|_| CryptoError::InvalidSignature)?;
    
    Ok(verifying_key.verify(data, &signature).is_ok())
}

/// Hash data with SHA-256 and return hex string
pub fn hash_sha256(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keypair_generation() {
        let kp = Keypair::generate();
        assert!(!kp.private_key_hex().is_empty());
        assert!(!kp.public_key_hex().is_empty());
    }
    
    #[test]
    fn test_sign_and_verify() {
        let kp = Keypair::generate();
        let data = b"test heartbeat data";
        let signature = kp.sign(data);
        
        let valid = verify_signature(&kp.public_key_hex(), data, &signature).unwrap();
        assert!(valid);
    }
    
    #[test]
    fn test_invalid_signature() {
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        let data = b"test data";
        let signature = kp1.sign(data);
        
        // Verify with wrong key should fail
        let valid = verify_signature(&kp2.public_key_hex(), data, &signature).unwrap();
        assert!(!valid);
    }
}
