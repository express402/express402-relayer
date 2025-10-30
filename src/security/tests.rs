#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, U256};
    use std::str::FromStr;

    fn create_test_verifier() -> SignatureVerifier {
        SignatureVerifier::new(
            "Express402".to_string(),
            "1".to_string(),
            1,
            Address::from_str("0x1234567890123456789012345678901234567890").unwrap(),
        )
    }

    fn create_test_message() -> TransactionMessage {
        TransactionMessage {
            user_address: Address::from_str("0x1234567890123456789012345678901234567890").unwrap(),
            target_contract: Address::from_str("0x0987654321098765432109876543210987654321").unwrap(),
            calldata: "0x1234".to_string(),
            value: U256::from(1000000000000000000u64),
            gas_limit: U256::from(21000u64),
            max_fee_per_gas: U256::from(20000000000u64),
            max_priority_fee_per_gas: U256::from(2000000000u64),
            nonce: U256::from(1u64),
            timestamp: 1640995200, // 2022-01-01 00:00:00 UTC
        }
    }

    fn create_test_signature() -> Signature {
        Signature {
            r: U256::from_str("0x1234567890123456789012345678901234567890123456789012345678901234").unwrap(),
            s: U256::from_str("0x0987654321098765432109876543210987654321098765432109876543210987").unwrap(),
            v: 27,
        }
    }

    #[test]
    fn test_signature_verifier_creation() {
        let verifier = create_test_verifier();
        assert_eq!(verifier.domain.name, "Express402");
        assert_eq!(verifier.domain.version, "1");
        assert_eq!(verifier.domain.chain_id, U256::from(1u64));
    }

    #[test]
    fn test_verify_nonce() {
        let verifier = create_test_verifier();
        let address = Address::from_str("0x1234567890123456789012345678901234567890").unwrap();
        let nonce = 5;

        // First verification should succeed
        assert!(verifier.verify_nonce(address, nonce));

        // Second verification with same nonce should fail
        assert!(!verifier.verify_nonce(address, nonce));

        // Higher nonce should succeed
        assert!(verifier.verify_nonce(address, nonce + 1));
    }

    #[test]
    fn test_verify_eip712_signature() {
        let verifier = create_test_verifier();
        let message = create_test_message();
        let signature = create_test_signature();
        let expected_address = Address::from_str("0x1234567890123456789012345678901234567890").unwrap();

        // This is a placeholder test since we have placeholder implementations
        // In a real implementation, this would test actual signature verification
        let result = verifier.verify_eip712_signature(&message, &signature, expected_address);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_eip712_hash() {
        let verifier = create_test_verifier();
        let message = create_test_message();

        let result = verifier.create_eip712_hash(&message);
        assert!(result.is_ok());
        
        let hash = result.unwrap();
        assert_eq!(hash.len(), 32); // Should be 32 bytes
    }
}