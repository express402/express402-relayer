use alloy::primitives::Address;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub address: Address,
    #[serde(skip)]
    pub private_key: Option<Arc<alloy::signers::k256::ecdsa::SigningKey>>,
    pub balance: alloy::primitives::U256,
    pub nonce: alloy::primitives::U256,
    pub is_active: bool,
    pub last_used: DateTime<Utc>,
    pub success_rate: f64,
    pub total_transactions: u64,
    pub failed_transactions: u64,
}

impl WalletInfo {
    pub fn new(address: Address, private_key: alloy::signers::k256::ecdsa::SigningKey) -> Self {
        Self {
            address,
            private_key: Some(Arc::new(private_key)),
            balance: alloy::primitives::U256::ZERO,
            nonce: alloy::primitives::U256::ZERO,
            is_active: true,
            last_used: Utc::now(),
            success_rate: 1.0,
            total_transactions: 0,
            failed_transactions: 0,
        }
    }

    pub fn update_success_rate(&mut self) {
        if self.total_transactions > 0 {
            self.success_rate = (self.total_transactions - self.failed_transactions) as f64 
                / self.total_transactions as f64;
        }
    }

    pub fn record_transaction(&mut self, success: bool) {
        self.total_transactions += 1;
        if !success {
            self.failed_transactions += 1;
        }
        self.update_success_rate();
        self.last_used = Utc::now();
    }

    pub fn is_healthy(&self) -> bool {
        self.is_active && self.success_rate > 0.8
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletPoolConfig {
    pub min_wallets: usize,
    pub max_wallets: usize,
    pub balance_threshold: alloy::primitives::U256,
    pub rotation_interval: u64, // seconds
    pub health_check_interval: u64, // seconds
    pub max_concurrent_transactions: u32,
    pub min_balance: u64,
    pub transaction_timeout: u64,
    pub retry_attempts: u32,
    pub retry_delay: u64,
}

impl Default for WalletPoolConfig {
    fn default() -> Self {
        Self {
            min_wallets: 3,
            max_wallets: 10,
            balance_threshold: alloy::primitives::U256::from(1000000000000000000u64), // 1 ETH
            rotation_interval: 300, // 5 minutes
            health_check_interval: 60, // 1 minute
            max_concurrent_transactions: 5,
            min_balance: 1000000000000000000, // 1 ETH
            transaction_timeout: 60,
            retry_attempts: 3,
            retry_delay: 5,
        }
    }
}
