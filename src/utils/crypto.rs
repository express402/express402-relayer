use alloy::{
    primitives::{Address, U256},
    signers::{k256::ecdsa::SigningKey, Signature},
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use rand::Rng;

use crate::types::{RelayerError, Result};

pub struct CryptoUtils;

impl CryptoUtils {
    /// Generate a new random private key
    pub fn generate_private_key() -> SigningKey {
        SigningKey::random(&mut rand::thread_rng())
    }

    /// Generate a new random address
    pub fn generate_address() -> Address {
        let private_key = Self::generate_private_key();
        let public_key = private_key.verifying_key();
        let public_key_bytes = public_key.to_sec1_bytes();
        let hash = Self::keccak256(&public_key_bytes[1..]); // Skip the first byte (0x04)
        Address::from_slice(&hash[12..]) // Take last 20 bytes
    }

    /// Convert a hex string to U256
    pub fn hex_to_u256(hex: &str) -> Result<U256> {
        let hex = if hex.starts_with("0x") {
            &hex[2..]
        } else {
            hex
        };

        U256::from_str(hex)
            .map_err(|e| RelayerError::Internal(format!("Invalid hex string: {}", e)))
    }

    /// Convert U256 to hex string
    pub fn u256_to_hex(value: &U256) -> String {
        format!("0x{:x}", value)
    }

    /// Convert a hex string to bytes
    pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>> {
        let hex = if hex.starts_with("0x") {
            &hex[2..]
        } else {
            hex
        };

        hex::decode(hex)
            .map_err(|e| RelayerError::Internal(format!("Invalid hex string: {}", e)))
    }

    /// Convert bytes to hex string
    pub fn bytes_to_hex(bytes: &[u8]) -> String {
        format!("0x{}", hex::encode(bytes))
    }

    /// Validate Ethereum address format
    pub fn is_valid_address(address: &str) -> bool {
        if address.len() != 42 || !address.starts_with("0x") {
            return false;
        }

        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Validate hex string format
    pub fn is_valid_hex(hex: &str) -> bool {
        let hex = if hex.starts_with("0x") {
            &hex[2..]
        } else {
            hex
        };

        hex.len() % 2 == 0 && hex.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Calculate keccak256 hash
    pub fn keccak256(data: &[u8]) -> [u8; 32] {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    /// Calculate sha256 hash
    pub fn sha256(data: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    /// Sign a message with a private key
    pub fn sign_message(private_key: &SigningKey, message: &[u8]) -> Result<Signature> {
        use alloy::signers::k256::ecdsa::signature::hazmat::PrehashSigner;
        let message_hash = Self::keccak256(message);
        private_key.sign_prehash(&message_hash)
            .map_err(|e| RelayerError::Internal(format!("Signing failed: {}", e)))
    }

    /// Verify a signature
    pub fn verify_signature(signature: &Signature, message: &[u8], address: Address) -> Result<bool> {
        let message_hash = Self::keccak256(message);
        let message_hash_fixed = alloy::primitives::FixedBytes::<32>::from(message_hash);
        let recovered_address = signature.recover_address_from_prehash(&message_hash_fixed)
            .map_err(|e| RelayerError::Internal(format!("Recovery failed: {}", e)))?;

        Ok(recovered_address == address)
    }

    /// Generate a random nonce
    pub fn generate_nonce() -> U256 {
        U256::from(rand::random::<u64>())
    }

    /// Generate a random salt
    pub fn generate_salt() -> [u8; 32] {
        let mut salt = [0u8; 32];
        rand::thread_rng().fill(&mut salt);
        salt
    }

    /// Derive a key from a password using PBKDF2
    pub fn derive_key(password: &str, salt: &[u8], iterations: u32) -> Result<[u8; 32]> {
        use pbkdf2::{pbkdf2_hmac, Sha256};
        
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);

        Ok(key)
    }

    /// Encrypt data using AES-256-GCM
    pub fn encrypt_aes_gcm(data: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        use aes_gcm::aead::{Aead, KeyInit};

        let cipher = Aes256Gcm::new(Key::from_slice(key));
        cipher.encrypt(Nonce::from_slice(nonce), data)
            .map_err(|e| RelayerError::Internal(format!("Encryption failed: {}", e)))
    }

    /// Decrypt data using AES-256-GCM
    pub fn decrypt_aes_gcm(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        use aes_gcm::aead::{Aead, KeyInit};

        let cipher = Aes256Gcm::new(Key::from_slice(key));
        cipher.decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|e| RelayerError::Internal(format!("Decryption failed: {}", e)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub salt: [u8; 32],
}

impl EncryptedData {
    pub fn encrypt(data: &[u8], password: &str) -> Result<Self> {
        let salt = CryptoUtils::generate_salt();
        let key = CryptoUtils::derive_key(password, &salt, 100000)?;
        let nonce = CryptoUtils::generate_salt()[..12].try_into().unwrap();
        let ciphertext = CryptoUtils::encrypt_aes_gcm(data, &key, &nonce)?;

        Ok(Self {
            ciphertext,
            nonce,
            salt,
        })
    }

    pub fn decrypt(&self, password: &str) -> Result<Vec<u8>> {
        let key = CryptoUtils::derive_key(password, &self.salt, 100000)?;
        CryptoUtils::decrypt_aes_gcm(&self.ciphertext, &key, &self.nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_conversion() {
        let hex = "0x1234";
        let u256 = CryptoUtils::hex_to_u256(hex).unwrap();
        assert_eq!(u256, U256::from(0x1234));

        let back_to_hex = CryptoUtils::u256_to_hex(&u256);
        assert_eq!(back_to_hex, "0x1234");
    }

    #[test]
    fn test_address_validation() {
        assert!(CryptoUtils::is_valid_address("0x1234567890123456789012345678901234567890"));
        assert!(!CryptoUtils::is_valid_address("invalid"));
        assert!(!CryptoUtils::is_valid_address("0x123"));
    }

    #[test]
    fn test_hex_validation() {
        assert!(CryptoUtils::is_valid_hex("0x1234"));
        assert!(CryptoUtils::is_valid_hex("1234"));
        assert!(!CryptoUtils::is_valid_hex("invalid"));
        assert!(!CryptoUtils::is_valid_hex("0x123"));
    }

    #[test]
    fn test_encryption_decryption() {
        let data = b"Hello, World!";
        let password = "test_password";
        
        let encrypted = EncryptedData::encrypt(data, password).unwrap();
        let decrypted = encrypted.decrypt(password).unwrap();
        
        assert_eq!(data, decrypted.as_slice());
    }

    #[test]
    fn test_signature_verification() {
        let private_key = CryptoUtils::generate_private_key();
        let message = b"test message";
        
        let signature = CryptoUtils::sign_message(&private_key, message).unwrap();
        let address = Address::from(private_key.verifying_key());
        
        assert!(CryptoUtils::verify_signature(&signature, message, address).unwrap());
    }
}
