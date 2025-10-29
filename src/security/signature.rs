use alloy::{
    primitives::{Address, U256},
    signers::{k256::ecdsa::SigningKey, Signature},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{RelayerError, Result, TransactionRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EIP712Domain {
    pub name: String,
    pub version: String,
    pub chain_id: U256,
    pub verifying_contract: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMessage {
    pub user_address: Address,
    pub target_contract: Address,
    pub calldata: String,
    pub value: U256,
    pub gas_limit: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub nonce: U256,
    pub timestamp: u64,
}

pub struct SignatureVerifier {
    domain: EIP712Domain,
    nonce_cache: HashMap<Address, u64>,
}

impl SignatureVerifier {
    pub fn new(chain_id: U256, verifying_contract: Address) -> Self {
        Self {
            domain: EIP712Domain {
                name: "Express402Relayer".to_string(),
                version: "1".to_string(),
                chain_id,
                verifying_contract,
            },
            nonce_cache: HashMap::new(),
        }
    }

    pub fn verify_transaction_signature(
        &mut self,
        request: &TransactionRequest,
        expected_nonce: u64,
    ) -> Result<bool> {
        // Check if signature is recent enough
        let now = Utc::now().timestamp() as u64;
        let signature_age = now - request.timestamp.timestamp() as u64;
        
        if signature_age > 300 { // 5 minutes
            return Err(RelayerError::SignatureVerification(
                "Signature too old".to_string(),
            ));
        }

        // Verify nonce
        if !self.verify_nonce(request.user_address, expected_nonce) {
            return Err(RelayerError::SignatureVerification(
                "Invalid nonce".to_string(),
            ));
        }

        // Create the message that was signed
        let message = TransactionMessage {
            user_address: request.user_address,
            target_contract: request.target_contract,
            calldata: format!("0x{}", hex::encode(&request.calldata)),
            value: request.value,
            gas_limit: request.gas_limit,
            max_fee_per_gas: request.max_fee_per_gas,
            max_priority_fee_per_gas: request.max_priority_fee_per_gas,
            nonce: request.nonce,
            timestamp: request.timestamp.timestamp() as u64,
        };

        // Verify EIP-712 signature
        self.verify_eip712_signature(&message, &request.signature, request.user_address)
    }

    fn verify_nonce(&mut self, address: Address, nonce: u64) -> bool {
        let current_nonce = self.nonce_cache.get(&address).copied().unwrap_or(0);
        
        if nonce <= current_nonce {
            return false;
        }

        self.nonce_cache.insert(address, nonce);
        true
    }

    fn verify_eip712_signature(
        &self,
        message: &TransactionMessage,
        signature: &crate::types::Signature,
        expected_address: Address,
    ) -> Result<bool> {
        // Basic signature format check
        if signature.v != 27 && signature.v != 28 {
            return Err(RelayerError::SignatureVerification(
                "Invalid signature v value".to_string(),
            ));
        }

        // Create EIP-712 typed data hash
        let typed_data_hash = self.create_eip712_hash(message)?;
        
        // Convert our signature format to alloy signature
        let alloy_signature = Signature::from_rs_and_parity(
            signature.r,
            signature.s,
            signature.v,
        ).map_err(|e| RelayerError::SignatureVerification(e.to_string()))?;

        // Recover the signer address
        let recovered_address = alloy_signature.recover_address_from_prehash(&typed_data_hash)
            .map_err(|e| RelayerError::SignatureVerification(e.to_string()))?;

        // Compare with expected address
        Ok(recovered_address == expected_address)
    }

    fn create_eip712_hash(&self, message: &TransactionMessage) -> Result<[u8; 32]> {
        use sha3::{Digest, Keccak256};

        // Create domain separator
        let domain_separator = self.create_domain_separator()?;
        
        // Create struct hash
        let struct_hash = self.create_struct_hash(message)?;
        
        // Create final hash: keccak256("\x19\x01" + domain_separator + struct_hash)
        let mut hasher = Keccak256::new();
        hasher.update(b"\x19\x01");
        hasher.update(domain_separator);
        hasher.update(struct_hash);
        
        let hash = hasher.finalize();
        let mut result = [0u8; 32];
        result.copy_from_slice(&hash);
        
        Ok(result)
    }

    fn create_domain_separator(&self) -> Result<[u8; 32]> {
        use sha3::{Digest, Keccak256};

        // EIP-712 domain separator: keccak256(keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)") + keccak256(name) + keccak256(version) + chainId + verifyingContract)
        
        let domain_type_hash = Keccak256::digest(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)");
        let name_hash = Keccak256::digest(self.domain.name.as_bytes());
        let version_hash = Keccak256::digest(self.domain.version.as_bytes());
        
        let mut hasher = Keccak256::new();
        hasher.update(domain_type_hash);
        hasher.update(name_hash);
        hasher.update(version_hash);
        hasher.update(self.domain.chain_id.to_be_bytes::<32>());
        hasher.update(self.domain.verifying_contract.as_slice());
        
        let hash = hasher.finalize();
        let mut result = [0u8; 32];
        result.copy_from_slice(&hash);
        
        Ok(result)
    }

    fn create_struct_hash(&self, message: &TransactionMessage) -> Result<[u8; 32]> {
        use sha3::{Digest, Keccak256};

        // TransactionMessage type hash
        let type_hash = Keccak256::digest(b"TransactionMessage(address user_address,address target_contract,string calldata,uint256 value,uint256 gas_limit,uint256 max_fee_per_gas,uint256 max_priority_fee_per_gas,uint256 nonce,uint256 timestamp)");
        
        // Encode struct data
        let mut encoded_data = Vec::new();
        
        // user_address (32 bytes)
        encoded_data.extend_from_slice(&message.user_address.to_word());
        
        // target_contract (32 bytes)
        encoded_data.extend_from_slice(&message.target_contract.to_word());
        
        // calldata (keccak256 hash)
        let calldata_hash = Keccak256::digest(message.calldata.as_bytes());
        encoded_data.extend_from_slice(&calldata_hash);
        
        // value (32 bytes)
        encoded_data.extend_from_slice(&message.value.to_be_bytes::<32>());
        
        // gas_limit (32 bytes)
        encoded_data.extend_from_slice(&message.gas_limit.to_be_bytes::<32>());
        
        // max_fee_per_gas (32 bytes)
        encoded_data.extend_from_slice(&message.max_fee_per_gas.to_be_bytes::<32>());
        
        // max_priority_fee_per_gas (32 bytes)
        encoded_data.extend_from_slice(&message.max_priority_fee_per_gas.to_be_bytes::<32>());
        
        // nonce (32 bytes)
        encoded_data.extend_from_slice(&message.nonce.to_be_bytes::<32>());
        
        // timestamp (32 bytes)
        encoded_data.extend_from_slice(&message.timestamp.to_be_bytes::<32>());
        
        // Create struct hash
        let mut hasher = Keccak256::new();
        hasher.update(type_hash);
        hasher.update(encoded_data);
        
        let hash = hasher.finalize();
        let mut result = [0u8; 32];
        result.copy_from_slice(&hash);
        
        Ok(result)
    }

    pub fn recover_address_from_signature(
        &self,
        message_hash: &[u8; 32],
        signature: &Signature,
    ) -> Result<Address> {
        // Recover the address from the signature
        let recovered = signature.recover_address_from_prehash(message_hash)
            .map_err(|e| RelayerError::SignatureVerification(e.to_string()))?;
        
        Ok(recovered)
    }

    pub fn sign_transaction(
        &self,
        private_key: &SigningKey,
        message_hash: &[u8; 32],
    ) -> Result<Signature> {
        let signature = private_key.sign_prehash(message_hash)
            .map_err(|e| RelayerError::SignatureVerification(e.to_string()))?;
        
        Ok(signature)
    }
}

impl Clone for SignatureVerifier {
    fn clone(&self) -> Self {
        Self {
            domain: self.domain.clone(),
            nonce_cache: self.nonce_cache.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    #[test]
    fn test_signature_verifier_creation() {
        let chain_id = U256::from(1);
        let contract = Address::ZERO;
        let verifier = SignatureVerifier::new(chain_id, contract);
        
        assert_eq!(verifier.domain.chain_id, chain_id);
        assert_eq!(verifier.domain.verifying_contract, contract);
    }

    #[test]
    fn test_nonce_verification() {
        let mut verifier = SignatureVerifier::new(U256::from(1), Address::ZERO);
        let address = Address::ZERO;
        
        // First nonce should be valid
        assert!(verifier.verify_nonce(address, 1));
        
        // Same nonce should be invalid
        assert!(!verifier.verify_nonce(address, 1));
        
        // Lower nonce should be invalid
        assert!(!verifier.verify_nonce(address, 0));
        
        // Higher nonce should be valid
        assert!(verifier.verify_nonce(address, 2));
    }
}
