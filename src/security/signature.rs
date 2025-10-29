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
        // This is a simplified implementation
        // In a real implementation, you would:
        // 1. Create the EIP-712 typed data hash
        // 2. Recover the signer from the signature
        // 3. Compare with expected_address
        
        // For now, we'll do a basic signature format check
        if signature.v != 27 && signature.v != 28 {
            return Err(RelayerError::SignatureVerification(
                "Invalid signature v value".to_string(),
            ));
        }

        // TODO: Implement proper EIP-712 signature verification
        // This would involve:
        // - Creating the domain separator
        // - Creating the struct hash
        // - Creating the final hash
        // - Recovering the signer
        // - Comparing with expected_address

        Ok(true) // Placeholder - always return true for now
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
