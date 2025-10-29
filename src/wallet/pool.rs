use alloy::{
    primitives::Address,
    signers::{k256::ecdsa::SigningKey, LocalWallet},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{Duration, Instant};

use crate::types::{RelayerError, Result, WalletInfo, WalletPoolConfig};

#[derive(Debug, Clone)]
pub struct WalletPool {
    wallets: Arc<RwLock<Vec<WalletInfo>>>,
    active_wallets: Arc<RwLock<Vec<Address>>>,
    wallet_usage: Arc<RwLock<HashMap<Address, WalletUsageStats>>>,
    config: WalletPoolConfig,
    rotation_strategy: RotationStrategy,
    semaphore: Arc<Semaphore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletUsageStats {
    pub address: Address,
    pub total_transactions: u64,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub last_used: DateTime<Utc>,
    pub average_gas_used: u64,
    pub total_gas_used: u64,
}

#[derive(Debug, Clone)]
pub enum RotationStrategy {
    RoundRobin,
    LeastUsed,
    BestPerformance,
    Random,
}

impl WalletPool {
    pub fn new(config: WalletPoolConfig) -> Self {
        let max_concurrent = config.max_concurrent_transactions as usize;
        
        Self {
            wallets: Arc::new(RwLock::new(Vec::new())),
            active_wallets: Arc::new(RwLock::new(Vec::new())),
            wallet_usage: Arc::new(RwLock::new(HashMap::new())),
            config,
            rotation_strategy: RotationStrategy::RoundRobin,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub async fn add_wallet(&self, private_key: SigningKey) -> Result<Address> {
        let wallet = LocalWallet::from(private_key);
        let address = wallet.address();
        
        let wallet_info = WalletInfo::new(address, wallet.signer());
        
        {
            let mut wallets = self.wallets.write().await;
            wallets.push(wallet_info);
        }

        {
            let mut active_wallets = self.active_wallets.write().await;
            if !active_wallets.contains(&address) {
                active_wallets.push(address);
            }
        }

        {
            let mut usage = self.wallet_usage.write().await;
            usage.insert(address, WalletUsageStats {
                address,
                total_transactions: 0,
                successful_transactions: 0,
                failed_transactions: 0,
                last_used: Utc::now(),
                average_gas_used: 0,
                total_gas_used: 0,
            });
        }

        tracing::info!("Added wallet: {:?}", address);
        Ok(address)
    }

    pub async fn remove_wallet(&self, address: Address) -> Result<()> {
        {
            let mut wallets = self.wallets.write().await;
            wallets.retain(|w| w.address != address);
        }

        {
            let mut active_wallets = self.active_wallets.write().await;
            active_wallets.retain(|&addr| addr != address);
        }

        {
            let mut usage = self.wallet_usage.write().await;
            usage.remove(&address);
        }

        tracing::info!("Removed wallet: {:?}", address);
        Ok(())
    }

    pub async fn get_next_wallet(&self) -> Result<Option<WalletInfo>> {
        let active_wallets = self.active_wallets.read().await.clone();
        
        if active_wallets.is_empty() {
            return Ok(None);
        }

        let selected_address = match self.rotation_strategy {
            RotationStrategy::RoundRobin => self.select_round_robin(&active_wallets).await?,
            RotationStrategy::LeastUsed => self.select_least_used(&active_wallets).await?,
            RotationStrategy::BestPerformance => self.select_best_performance(&active_wallets).await?,
            RotationStrategy::Random => self.select_random(&active_wallets).await?,
        };

        let wallets = self.wallets.read().await;
        let wallet = wallets.iter().find(|w| w.address == selected_address).cloned();
        
        Ok(wallet)
    }

    pub async fn acquire_wallet(&self) -> Result<Option<WalletInfo>> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| RelayerError::WalletPool(e.to_string()))?;

        let wallet = self.get_next_wallet().await?;
        
        if let Some(ref wallet_info) = wallet {
            self.update_wallet_usage(wallet_info.address, true).await?;
        }

        Ok(wallet)
    }

    pub async fn release_wallet(&self, address: Address, success: bool, gas_used: u64) -> Result<()> {
        {
            let mut wallets = self.wallets.write().await;
            if let Some(wallet) = wallets.iter_mut().find(|w| w.address == address) {
                wallet.record_transaction(success);
            }
        }

        {
            let mut usage = self.wallet_usage.write().await;
            if let Some(stats) = usage.get_mut(&address) {
                stats.total_transactions += 1;
                if success {
                    stats.successful_transactions += 1;
                } else {
                    stats.failed_transactions += 1;
                }
                stats.last_used = Utc::now();
                stats.total_gas_used += gas_used;
                stats.average_gas_used = stats.total_gas_used / stats.total_transactions;
            }
        }

        Ok(())
    }

    pub async fn get_wallet_stats(&self, address: Address) -> Result<Option<WalletUsageStats>> {
        let usage = self.wallet_usage.read().await;
        Ok(usage.get(&address).cloned())
    }

    pub async fn get_all_wallet_stats(&self) -> Result<Vec<WalletUsageStats>> {
        let usage = self.wallet_usage.read().await;
        Ok(usage.values().cloned().collect())
    }

    pub async fn is_wallet_healthy(&self, address: Address) -> Result<bool> {
        let wallets = self.wallets.read().await;
        if let Some(wallet) = wallets.iter().find(|w| w.address == address) {
            Ok(wallet.is_healthy())
        } else {
            Ok(false)
        }
    }

    pub async fn get_healthy_wallets(&self) -> Result<Vec<WalletInfo>> {
        let wallets = self.wallets.read().await;
        let healthy_wallets: Vec<WalletInfo> = wallets
            .iter()
            .filter(|w| w.is_healthy())
            .cloned()
            .collect();
        
        Ok(healthy_wallets)
    }

    pub async fn set_rotation_strategy(&mut self, strategy: RotationStrategy) {
        self.rotation_strategy = strategy;
    }

    async fn select_round_robin(&self, addresses: &[Address]) -> Result<Address> {
        let usage = self.wallet_usage.read().await;
        
        // Find the wallet with the oldest last_used time
        let mut oldest_wallet = addresses[0];
        let mut oldest_time = Utc::now();
        
        for &address in addresses {
            if let Some(stats) = usage.get(&address) {
                if stats.last_used < oldest_time {
                    oldest_time = stats.last_used;
                    oldest_wallet = address;
                }
            }
        }
        
        Ok(oldest_wallet)
    }

    async fn select_least_used(&self, addresses: &[Address]) -> Result<Address> {
        let usage = self.wallet_usage.read().await;
        
        let mut least_used_wallet = addresses[0];
        let mut min_transactions = u64::MAX;
        
        for &address in addresses {
            if let Some(stats) = usage.get(&address) {
                if stats.total_transactions < min_transactions {
                    min_transactions = stats.total_transactions;
                    least_used_wallet = address;
                }
            }
        }
        
        Ok(least_used_wallet)
    }

    async fn select_best_performance(&self, addresses: &[Address]) -> Result<Address> {
        let usage = self.wallet_usage.read().await;
        
        let mut best_wallet = addresses[0];
        let mut best_success_rate = 0.0;
        
        for &address in addresses {
            if let Some(stats) = usage.get(&address) {
                let success_rate = if stats.total_transactions > 0 {
                    stats.successful_transactions as f64 / stats.total_transactions as f64
                } else {
                    1.0
                };
                
                if success_rate > best_success_rate {
                    best_success_rate = success_rate;
                    best_wallet = address;
                }
            }
        }
        
        Ok(best_wallet)
    }

    async fn select_random(&self, addresses: &[Address]) -> Result<Address> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..addresses.len());
        Ok(addresses[index])
    }

    async fn update_wallet_usage(&self, address: Address, _acquired: bool) -> Result<()> {
        {
            let mut usage = self.wallet_usage.write().await;
            if let Some(stats) = usage.get_mut(&address) {
                stats.last_used = Utc::now();
            }
        }
        Ok(())
    }

    pub async fn get_pool_stats(&self) -> Result<WalletPoolStats> {
        let wallets = self.wallets.read().await;
        let active_wallets = self.active_wallets.read().await;
        let usage = self.wallet_usage.read().await;

        let total_wallets = wallets.len();
        let active_count = active_wallets.len();
        let healthy_count = wallets.iter().filter(|w| w.is_healthy()).count();

        let mut total_transactions = 0u64;
        let mut total_successful = 0u64;
        
        for stats in usage.values() {
            total_transactions += stats.total_transactions;
            total_successful += stats.successful_transactions;
        }

        let overall_success_rate = if total_transactions > 0 {
            total_successful as f64 / total_transactions as f64
        } else {
            1.0
        };

        Ok(WalletPoolStats {
            total_wallets,
            active_wallets: active_count,
            healthy_wallets: healthy_count,
            total_transactions,
            overall_success_rate,
            available_permits: self.semaphore.available_permits(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletPoolStats {
    pub total_wallets: usize,
    pub active_wallets: usize,
    pub healthy_wallets: usize,
    pub total_transactions: u64,
    pub overall_success_rate: f64,
    pub available_permits: usize,
}

impl Clone for WalletPool {
    fn clone(&self) -> Self {
        Self {
            wallets: Arc::clone(&self.wallets),
            active_wallets: Arc::clone(&self.active_wallets),
            wallet_usage: Arc::clone(&self.wallet_usage),
            config: self.config.clone(),
            rotation_strategy: self.rotation_strategy.clone(),
            semaphore: Arc::clone(&self.semaphore),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::signers::k256::ecdsa::SigningKey;

    #[tokio::test]
    async fn test_wallet_pool_creation() {
        let config = WalletPoolConfig::default();
        let pool = WalletPool::new(config);
        
        let stats = pool.get_pool_stats().await.unwrap();
        assert_eq!(stats.total_wallets, 0);
        assert_eq!(stats.active_wallets, 0);
    }

    #[tokio::test]
    async fn test_add_wallet() {
        let config = WalletPoolConfig::default();
        let pool = WalletPool::new(config);
        
        let private_key = SigningKey::random(&mut rand::thread_rng());
        let address = pool.add_wallet(private_key).await.unwrap();
        
        let stats = pool.get_pool_stats().await.unwrap();
        assert_eq!(stats.total_wallets, 1);
        assert_eq!(stats.active_wallets, 1);
        
        assert!(pool.is_wallet_healthy(address).await.unwrap());
    }

    #[tokio::test]
    async fn test_wallet_rotation() {
        let config = WalletPoolConfig::default();
        let pool = WalletPool::new(config);
        
        // Add multiple wallets
        for _ in 0..3 {
            let private_key = SigningKey::random(&mut rand::thread_rng());
            pool.add_wallet(private_key).await.unwrap();
        }
        
        // Test round robin selection
        let wallet1 = pool.get_next_wallet().await.unwrap().unwrap();
        let wallet2 = pool.get_next_wallet().await.unwrap().unwrap();
        
        // Should be different wallets (in round robin)
        assert_ne!(wallet1.address, wallet2.address);
    }
}
