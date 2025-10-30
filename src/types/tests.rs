#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, U256};
    use std::str::FromStr;

    fn create_test_transaction_request() -> TransactionRequest {
        TransactionRequest {
            user_address: Address::from_str("0x1234567890123456789012345678901234567890").unwrap(),
            target_contract: Address::from_str("0x0987654321098765432109876543210987654321").unwrap(),
            calldata: "0x1234".to_string(),
            value: U256::from(1000000000000000000u64),
            gas_limit: U256::from(21000u64),
            max_fee_per_gas: U256::from(20000000000u64),
            max_priority_fee_per_gas: U256::from(2000000000u64),
            nonce: U256::from(1u64),
            signature: Signature {
                r: U256::from_str("0x1234567890123456789012345678901234567890123456789012345678901234").unwrap(),
                s: U256::from_str("0x0987654321098765432109876543210987654321098765432109876543210987").unwrap(),
                v: 27,
            },
            priority: Priority::Normal,
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
    fn test_transaction_request_creation() {
        let tx = create_test_transaction_request();
        assert_eq!(tx.value, U256::from(1000000000000000000u64));
        assert_eq!(tx.gas_limit, U256::from(21000u64));
        assert_eq!(tx.priority, Priority::Normal);
    }

    #[test]
    fn test_signature_creation() {
        let sig = create_test_signature();
        assert_eq!(sig.v, 27);
        assert!(!sig.r.is_zero());
        assert!(!sig.s.is_zero());
    }

    #[test]
    fn test_signature_serialization() {
        let sig = create_test_signature();
        let serialized = serde_json::to_string(&sig).unwrap();
        let deserialized: Signature = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(sig.r, deserialized.r);
        assert_eq!(sig.s, deserialized.s);
        assert_eq!(sig.v, deserialized.v);
    }

    #[test]
    fn test_priority_enum() {
        assert_eq!(Priority::Urgent as u8, 0);
        assert_eq!(Priority::High as u8, 1);
        assert_eq!(Priority::Normal as u8, 2);
        assert_eq!(Priority::Low as u8, 3);
    }

    #[test]
    fn test_priority_from_str() {
        assert_eq!(Priority::from_str("urgent").unwrap(), Priority::Urgent);
        assert_eq!(Priority::from_str("high").unwrap(), Priority::High);
        assert_eq!(Priority::from_str("normal").unwrap(), Priority::Normal);
        assert_eq!(Priority::from_str("low").unwrap(), Priority::Low);
        assert!(Priority::from_str("invalid").is_err());
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(format!("{}", Priority::Urgent), "urgent");
        assert_eq!(format!("{}", Priority::High), "high");
        assert_eq!(format!("{}", Priority::Normal), "normal");
        assert_eq!(format!("{}", Priority::Low), "low");
    }

    #[test]
    fn test_transaction_status_enum() {
        assert_eq!(TransactionStatus::Pending as u8, 0);
        assert_eq!(TransactionStatus::Submitted as u8, 1);
        assert_eq!(TransactionStatus::Confirmed as u8, 2);
        assert_eq!(TransactionStatus::Failed as u8, 3);
        assert_eq!(TransactionStatus::Cancelled as u8, 4);
    }

    #[test]
    fn test_relayer_error_creation() {
        let error = RelayerError::Config("test error".to_string());
        assert!(matches!(error, RelayerError::Config(_)));
        
        let error = RelayerError::Database("test error".to_string());
        assert!(matches!(error, RelayerError::Database(_)));
        
        let error = RelayerError::SignatureVerification("test error".to_string());
        assert!(matches!(error, RelayerError::SignatureVerification(_)));
    }

    #[test]
    fn test_relayer_error_from_string() {
        let error: RelayerError = "test error".to_string().into();
        assert!(matches!(error, RelayerError::Internal(_)));
    }
}