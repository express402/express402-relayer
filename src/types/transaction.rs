use alloy::{
    primitives::{Address, Bytes, U256},
    rpc::types::TransactionRequest as AlloyTransactionRequest,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

impl Priority {
    pub fn weight(&self) -> u8 {
        match self {
            Priority::Low => 1,
            Priority::Normal => 2,
            Priority::High => 3,
            Priority::Critical => 4,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Priority::Low => "low".to_string(),
            Priority::Normal => "normal".to_string(),
            Priority::High => "high".to_string(),
            Priority::Critical => "critical".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub r: U256,
    pub s: U256,
    pub v: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub id: Uuid,
    pub user_address: Address,
    pub target_contract: Address,
    pub calldata: Bytes,
    pub value: U256,
    pub gas_limit: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub nonce: U256,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub priority: Priority,
}

impl TransactionRequest {
    pub fn new(
        user_address: Address,
        target_contract: Address,
        calldata: Bytes,
        value: U256,
        gas_limit: U256,
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        nonce: U256,
        signature: Signature,
        priority: Priority,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_address,
            target_contract,
            calldata,
            value,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            nonce,
            signature,
            timestamp: Utc::now(),
            priority,
        }
    }

    pub fn to_alloy_request(&self, from: Address) -> AlloyTransactionRequest {
        AlloyTransactionRequest {
            from: Some(from),
            to: Some(alloy::primitives::TxKind::Call(self.target_contract)),
            value: Some(self.value),
            gas: Some(self.gas_limit.try_into().unwrap_or(u64::MAX)),
            gas_price: None,
            max_fee_per_gas: Some(self.max_fee_per_gas.try_into().unwrap_or(u128::MAX)),
            max_priority_fee_per_gas: Some(self.max_priority_fee_per_gas.try_into().unwrap_or(u128::MAX)),
            input: Some(self.calldata.clone()),
            nonce: Some(self.nonce.try_into().unwrap_or(u64::MAX)),
            chain_id: None,
            access_list: None,
            blob_versioned_hashes: None,
            max_fee_per_blob_gas: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Processing,
    Submitted,
    Confirmed,
    Failed,
    Cancelled,
}

impl TransactionStatus {
    pub fn to_string(&self) -> String {
        match self {
            TransactionStatus::Pending => "pending".to_string(),
            TransactionStatus::Processing => "processing".to_string(),
            TransactionStatus::Submitted => "submitted".to_string(),
            TransactionStatus::Confirmed => "confirmed".to_string(),
            TransactionStatus::Failed => "failed".to_string(),
            TransactionStatus::Cancelled => "cancelled".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub id: Uuid,
    pub status: TransactionStatus,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub gas_used: Option<U256>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
