use alloy::primitives::{Address, U256, Bytes};
use crate::types::{RelayerError, Result, Signature};

/// Transaction validator for validating transaction requests
pub struct TransactionValidator;

impl TransactionValidator {
    /// Validate address format
    pub fn validate_address(address: &Address) -> Result<()> {
        // Basic validation - address is already parsed, just check it's not zero
        if address == &Address::ZERO {
            return Err(RelayerError::Internal("Address cannot be zero".to_string()));
        }
        Ok(())
    }

    /// Validate Ethereum address string
    pub fn validate_address_string(address_str: &str) -> Result<Address> {
        if address_str.is_empty() {
            return Err(RelayerError::Api("Address cannot be empty".to_string()));
        }

        if !address_str.starts_with("0x") {
            return Err(RelayerError::Api("Address must start with 0x".to_string()));
        }

        if address_str.len() != 42 {
            return Err(RelayerError::Api(
                format!("Invalid address length: expected 42, got {}", address_str.len())
            ));
        }

        address_str.parse::<Address>()
            .map_err(|_| RelayerError::Api("Invalid address format".to_string()))
    }

    /// Validate calldata format
    pub fn validate_calldata(calldata: &[u8]) -> Result<()> {
        if calldata.is_empty() {
            return Err(RelayerError::Api("Calldata cannot be empty".to_string()));
        }

        // Check calldata size (max contract call data size)
        const MAX_CALLDATA_SIZE: usize = 1024 * 1024; // 1MB
        if calldata.len() > MAX_CALLDATA_SIZE {
            return Err(RelayerError::Api(
                format!("Calldata size exceeds maximum: {} bytes", MAX_CALLDATA_SIZE)
            ));
        }

        Ok(())
    }

    /// Validate calldata hex string
    pub fn validate_calldata_string(calldata_str: &str) -> Result<Vec<u8>> {
        let hex_str = if calldata_str.starts_with("0x") {
            &calldata_str[2..]
        } else {
            calldata_str
        };

        if hex_str.is_empty() {
            return Err(RelayerError::Api("Calldata cannot be empty".to_string()));
        }

        if hex_str.len() % 2 != 0 {
            return Err(RelayerError::Api("Invalid hex string length (must be even)".to_string()));
        }

        hex::decode(hex_str)
            .map_err(|e| RelayerError::Api(format!("Invalid hex string: {}", e)))
            .and_then(|bytes| {
                Self::validate_calldata(&bytes)?;
                Ok(bytes)
            })
    }

    /// Validate gas limit
    pub fn validate_gas_limit(gas_limit: &U256) -> Result<()> {
        if gas_limit == &U256::ZERO {
            return Err(RelayerError::Api("Gas limit cannot be zero".to_string()));
        }

        const MIN_GAS_LIMIT: u64 = 21000; // Minimum for simple transfer
        const MAX_GAS_LIMIT: u64 = 30_000_000; // Block gas limit

        let gas_limit_u64 = gas_limit.to::<u64>();
        if gas_limit_u64 < MIN_GAS_LIMIT {
            return Err(RelayerError::Api(
                format!("Gas limit too low: minimum is {}", MIN_GAS_LIMIT)
            ));
        }

        if gas_limit_u64 > MAX_GAS_LIMIT {
            return Err(RelayerError::Api(
                format!("Gas limit too high: maximum is {}", MAX_GAS_LIMIT)
            ));
        }

        Ok(())
    }

    /// Validate gas prices (EIP-1559)
    pub fn validate_gas_prices(
        max_fee_per_gas: &U256,
        max_priority_fee_per_gas: &U256,
    ) -> Result<()> {
        // Check both are non-zero
        if max_fee_per_gas == &U256::ZERO {
            return Err(RelayerError::Api("max_fee_per_gas cannot be zero".to_string()));
        }

        if max_priority_fee_per_gas == &U256::ZERO {
            return Err(RelayerError::Api("max_priority_fee_per_gas cannot be zero".to_string()));
        }

        // max_fee_per_gas must be >= max_priority_fee_per_gas
        if max_fee_per_gas < max_priority_fee_per_gas {
            return Err(RelayerError::Api(
                "max_fee_per_gas must be >= max_priority_fee_per_gas".to_string()
            ));
        }

        // Reasonable gas price limits (in gwei)
        const MIN_GAS_PRICE: u64 = 1_000_000_000; // 1 gwei
        const MAX_GAS_PRICE: u64 = 1_000_000_000_000; // 1000 gwei

        if max_fee_per_gas.to::<u64>() < MIN_GAS_PRICE {
            return Err(RelayerError::Api(
                format!("Gas price too low: minimum is {} gwei", MIN_GAS_PRICE / 1_000_000_000)
            ));
        }

        if max_fee_per_gas.to::<u64>() > MAX_GAS_PRICE {
            return Err(RelayerError::Api(
                format!("Gas price too high: maximum is {} gwei", MAX_GAS_PRICE / 1_000_000_000)
            ));
        }

        Ok(())
    }

    /// Validate value (ETH amount)
    pub fn validate_value(value: &U256) -> Result<()> {
        // Value can be zero for contract calls
        // Just check it's reasonable (not too large)
        const MAX_VALUE: u64 = 10_000_000_000_000_000_000_000; // 10,000 ETH
        if value.to::<u128>() > MAX_VALUE as u128 {
            return Err(RelayerError::Api(
                format!("Value too large: maximum is {} ETH", MAX_VALUE / 1_000_000_000_000_000_000)
            ));
        }

        Ok(())
    }

    /// Validate nonce
    pub fn validate_nonce(nonce: &U256) -> Result<()> {
        // Nonce should be a reasonable value (u64 max)
        let nonce_u64 = nonce.to::<u64>();
        if nonce_u64 > u64::MAX / 2 {
            return Err(RelayerError::Api("Nonce value too large".to_string()));
        }

        Ok(())
    }

    /// Validate signature
    pub fn validate_signature(signature: &Signature) -> Result<()> {
        // Signature r and s must be non-zero
        if signature.r == U256::ZERO || signature.s == U256::ZERO {
            return Err(RelayerError::Api("Signature r and s must be non-zero".to_string()));
        }

        // Signature v must be 27 or 28 (or 35+ for EIP-155)
        if signature.v != 27 && signature.v != 28 && signature.v < 35 {
            return Err(RelayerError::Api(
                format!("Invalid signature v value: {}", signature.v)
            ));
        }

        Ok(())
    }

    /// Comprehensive transaction validation
    pub fn validate_transaction_params(
        user_address: &Address,
        target_contract: &Address,
        calldata: &Bytes,
        value: &U256,
        gas_limit: &U256,
        max_fee_per_gas: &U256,
        max_priority_fee_per_gas: &U256,
        nonce: &U256,
        signature: &Signature,
    ) -> Result<()> {
        // Validate addresses
        Self::validate_address(user_address)?;
        Self::validate_address(target_contract)?;

        // Validate calldata
        Self::validate_calldata(calldata.as_ref())?;

        // Validate value
        Self::validate_value(value)?;

        // Validate gas
        Self::validate_gas_limit(gas_limit)?;
        Self::validate_gas_prices(max_fee_per_gas, max_priority_fee_per_gas)?;

        // Validate nonce
        Self::validate_nonce(nonce)?;

        // Validate signature
        Self::validate_signature(signature)?;

        Ok(())
    }

    /// Validate priority string
    pub fn validate_priority(priority_str: &str) -> Result<crate::types::Priority> {
        match priority_str {
            "low" => Ok(crate::types::Priority::Low),
            "normal" => Ok(crate::types::Priority::Normal),
            "high" => Ok(crate::types::Priority::High),
            "critical" => Ok(crate::types::Priority::Critical),
            _ => Err(RelayerError::Api(
                format!("Invalid priority: {}. Must be one of: low, normal, high, critical", priority_str)
            )),
        }
    }

    /// Parse and validate U256 from string
    pub fn parse_u256(value_str: &str) -> Result<U256> {
        if value_str.is_empty() {
            return Err(RelayerError::Api("Value cannot be empty".to_string()));
        }

        // Remove 0x prefix if present
        let clean_str = if value_str.starts_with("0x") {
            &value_str[2..]
        } else {
            value_str
        };

        if clean_str.is_empty() {
            return Err(RelayerError::Api("Value cannot be empty after removing 0x prefix".to_string()));
        }

        U256::from_str_radix(clean_str, 16)
            .or_else(|_| clean_str.parse::<u64>().map(U256::from))
            .map_err(|_| RelayerError::Api(format!("Invalid number format: {}", value_str)))
    }
}

use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_address_string() {
        let valid = "0x1234567890123456789012345678901234567890";
        assert!(TransactionValidator::validate_address_string(valid).is_ok());

        let invalid = "0x123";
        assert!(TransactionValidator::validate_address_string(invalid).is_err());

        let invalid_no_prefix = "1234567890123456789012345678901234567890";
        assert!(TransactionValidator::validate_address_string(invalid_no_prefix).is_err());
    }

    #[test]
    fn test_validate_gas_limit() {
        assert!(TransactionValidator::validate_gas_limit(&U256::from(21000)).is_ok());
        assert!(TransactionValidator::validate_gas_limit(&U256::from(100000)).is_ok());
        assert!(TransactionValidator::validate_gas_limit(&U256::ZERO).is_err());
        assert!(TransactionValidator::validate_gas_limit(&U256::from(1000)).is_err());
    }

    #[test]
    fn test_validate_gas_prices() {
        let max_fee = U256::from(20_000_000_000u64); // 20 gwei
        let priority_fee = U256::from(2_000_000_000u64); // 2 gwei

        assert!(TransactionValidator::validate_gas_prices(&max_fee, &priority_fee).is_ok());

        // max_fee < priority_fee should fail
        assert!(TransactionValidator::validate_gas_prices(&priority_fee, &max_fee).is_err());

        // Zero values should fail
        assert!(TransactionValidator::validate_gas_prices(&U256::ZERO, &priority_fee).is_err());
    }
}

