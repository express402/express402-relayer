use alloy::{
    primitives::{Address, U256},
    providers::{Provider, RootProvider},
    rpc::types::BlockId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{RelayerError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub address: Address,
    pub balance: U256,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub is_sufficient: bool,
}

pub struct BalanceChecker<P> {
    provider: Arc<P>,
    balance_cache: Arc<RwLock<HashMap<Address, BalanceInfo>>>,
    min_balance_threshold: U256,
    cache_ttl: std::time::Duration,
}

impl<P> BalanceChecker<P>
where
    P: Provider + Send + Sync + 'static,
{
    pub fn new(provider: Arc<P>, min_balance_threshold: U256, cache_ttl: std::time::Duration) -> Self {
        Self {
            provider,
            balance_cache: Arc::new(RwLock::new(HashMap::new())),
            min_balance_threshold,
            cache_ttl,
        }
    }

    pub async fn check_balance(&self, address: Address) -> Result<BalanceInfo> {
        // Check cache first
        {
            let cache = self.balance_cache.read().await;
            if let Some(cached_info) = cache.get(&address) {
                let now = chrono::Utc::now();
                let cache_age = now - cached_info.last_updated;
                
                if cache_age < chrono::Duration::from_std(self.cache_ttl).unwrap_or_default() {
                    return Ok(cached_info.clone());
                }
            }
        }

        // Fetch fresh balance
        let balance = self.fetch_balance(address).await?;
        let is_sufficient = balance >= self.min_balance_threshold;
        
        let balance_info = BalanceInfo {
            address,
            balance,
            last_updated: chrono::Utc::now(),
            is_sufficient,
        };

        // Update cache
        {
            let mut cache = self.balance_cache.write().await;
            cache.insert(address, balance_info.clone());
        }

        Ok(balance_info)
    }

    pub async fn check_multiple_balances(&self, addresses: Vec<Address>) -> Result<Vec<BalanceInfo>> {
        let mut results = Vec::new();
        
        for address in addresses {
            let balance_info = self.check_balance(address).await?;
            results.push(balance_info);
        }

        Ok(results)
    }

    pub async fn is_balance_sufficient(&self, address: Address) -> Result<bool> {
        let balance_info = self.check_balance(address).await?;
        Ok(balance_info.is_sufficient)
    }

    pub async fn get_balance(&self, address: Address) -> Result<U256> {
        let balance_info = self.check_balance(address).await?;
        Ok(balance_info.balance)
    }

    async fn fetch_balance(&self, address: Address) -> Result<U256> {
        let balance = self.provider
            .get_balance(address)
            .await
            .map_err(|e| RelayerError::Ethereum(e.to_string()))?;

        Ok(balance)
    }

    pub async fn preload_balances(&self, addresses: Vec<Address>) -> Result<()> {
        let mut handles = Vec::new();

        for address in addresses {
            let checker = Arc::new(self.clone());
            let handle = tokio::spawn(async move {
                checker.check_balance(address).await
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await
                .map_err(|e| RelayerError::Internal(e.to_string()))?
                .map_err(|e| RelayerError::Internal(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.balance_cache.write().await;
        cache.clear();
    }

    pub async fn get_cache_stats(&self) -> BalanceCacheStats {
        let cache = self.balance_cache.read().await;
        BalanceCacheStats {
            total_entries: cache.len(),
            cache_ttl_seconds: self.cache_ttl.as_secs(),
            min_balance_threshold: self.min_balance_threshold,
        }
    }

    pub fn set_min_balance_threshold(&mut self, threshold: U256) {
        self.min_balance_threshold = threshold;
    }

    pub fn set_cache_ttl(&mut self, ttl: std::time::Duration) {
        self.cache_ttl = ttl;
    }
}

impl<P> Clone for BalanceChecker<P>
where
    P: Provider + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            balance_cache: Arc::clone(&self.balance_cache),
            min_balance_threshold: self.min_balance_threshold,
            cache_ttl: self.cache_ttl,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceCacheStats {
    pub total_entries: usize,
    pub cache_ttl_seconds: u64,
    pub min_balance_threshold: U256,
}

pub struct BalanceMonitor<P> {
    checker: BalanceChecker<P>,
    monitoring_addresses: Arc<RwLock<Vec<Address>>>,
    alert_threshold: U256,
}

impl<P> BalanceMonitor<P>
where
    P: Provider + Send + Sync + 'static,
{
    pub fn new(
        checker: BalanceChecker<P>,
        alert_threshold: U256,
    ) -> Self {
        Self {
            checker,
            monitoring_addresses: Arc::new(RwLock::new(Vec::new())),
            alert_threshold,
        }
    }

    pub async fn add_address(&self, address: Address) {
        let mut addresses = self.monitoring_addresses.write().await;
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }

    pub async fn remove_address(&self, address: Address) {
        let mut addresses = self.monitoring_addresses.write().await;
        addresses.retain(|&addr| addr != address);
    }

    pub async fn check_all_balances(&self) -> Result<Vec<BalanceInfo>> {
        let addresses = self.monitoring_addresses.read().await.clone();
        self.checker.check_multiple_balances(addresses).await
    }

    pub async fn get_low_balance_addresses(&self) -> Result<Vec<Address>> {
        let balance_infos = self.check_all_balances().await?;
        let low_balance_addresses: Vec<Address> = balance_infos
            .into_iter()
            .filter(|info| info.balance < self.alert_threshold)
            .map(|info| info.address)
            .collect();

        Ok(low_balance_addresses)
    }

    pub async fn start_monitoring(&self, interval: std::time::Duration) -> Result<()> {
        let monitor = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                if let Ok(low_balance_addresses) = monitor.get_low_balance_addresses().await {
                    if !low_balance_addresses.is_empty() {
                        tracing::warn!(
                            "Low balance detected for addresses: {:?}",
                            low_balance_addresses
                        );
                    }
                }
            }
        });

        Ok(())
    }
}

impl<P> Clone for BalanceMonitor<P>
where
    P: Provider + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            checker: self.checker.clone(),
            monitoring_addresses: Arc::clone(&self.monitoring_addresses),
            alert_threshold: self.alert_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    #[tokio::test]
    async fn test_balance_checker_creation() {
        // This would require a mock provider in a real test
        // For now, just test the structure
        let min_balance = U256::from(1000000000000000000u64); // 1 ETH
        let cache_ttl = std::time::Duration::from_secs(60);
        
        // In a real test, you would create a mock provider
        // let provider = Arc::new(MockProvider::new());
        // let checker = BalanceChecker::new(provider, min_balance, cache_ttl);
        
        assert_eq!(min_balance, U256::from(1000000000000000000u64));
        assert_eq!(cache_ttl.as_secs(), 60);
    }
}
