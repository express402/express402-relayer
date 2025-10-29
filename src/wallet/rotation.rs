use alloy::primitives::Address;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

use crate::types::{RelayerError, Result, WalletInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationPolicy {
    pub strategy: RotationStrategy,
    pub interval: Duration,
    pub min_usage_before_rotation: u64,
    pub max_usage_before_rotation: u64,
    pub performance_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationStrategy {
    RoundRobin,
    LeastUsed,
    BestPerformance,
    LoadBalanced,
    TimeBased,
    Random,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletRotationStats {
    pub address: Address,
    pub total_rotations: u64,
    pub last_rotation: DateTime<Utc>,
    pub rotation_reason: RotationReason,
    pub performance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationReason {
    Scheduled,
    LowPerformance,
    HighLoad,
    ErrorRate,
    BalanceLow,
    Manual,
}

pub struct WalletRotator {
    wallets: Arc<RwLock<Vec<WalletInfo>>>,
    rotation_stats: Arc<RwLock<HashMap<Address, WalletRotationStats>>>,
    policy: RotationPolicy,
    last_rotation: Arc<RwLock<DateTime<Utc>>>,
    rotation_index: Arc<RwLock<usize>>,
}

impl WalletRotator {
    pub fn new(
        wallets: Arc<RwLock<Vec<WalletInfo>>>,
        policy: RotationPolicy,
    ) -> Self {
        Self {
            wallets,
            rotation_stats: Arc::new(RwLock::new(HashMap::new())),
            policy,
            last_rotation: Arc::new(RwLock::new(Utc::now())),
            rotation_index: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn should_rotate(&self) -> Result<bool> {
        let last_rotation = *self.last_rotation.read().await;
        let now = Utc::now();
        let time_since_last_rotation = now - last_rotation;

        // Check if enough time has passed for scheduled rotation
        if time_since_last_rotation > chrono::Duration::from_std(self.policy.interval)
            .map_err(|e| RelayerError::Internal(e.to_string()))? {
            return Ok(true);
        }

        // Check for performance-based rotation
        if self.check_performance_rotation().await? {
            return Ok(true);
        }

        // Check for load-based rotation
        if self.check_load_rotation().await? {
            return Ok(true);
        }

        Ok(false)
    }

    pub async fn rotate_wallets(&self) -> Result<Vec<Address>> {
        let wallets = self.wallets.read().await.clone();
        let mut rotated_addresses = Vec::new();

        match self.policy.strategy {
            RotationStrategy::RoundRobin => {
                rotated_addresses = self.rotate_round_robin(&wallets).await?;
            }
            RotationStrategy::LeastUsed => {
                rotated_addresses = self.rotate_least_used(&wallets).await?;
            }
            RotationStrategy::BestPerformance => {
                rotated_addresses = self.rotate_best_performance(&wallets).await?;
            }
            RotationStrategy::LoadBalanced => {
                rotated_addresses = self.rotate_load_balanced(&wallets).await?;
            }
            RotationStrategy::TimeBased => {
                rotated_addresses = self.rotate_time_based(&wallets).await?;
            }
            RotationStrategy::Random => {
                rotated_addresses = self.rotate_random(&wallets).await?;
            }
        }

        // Update rotation stats
        for address in &rotated_addresses {
            self.update_rotation_stats(*address, RotationReason::Scheduled).await?;
        }

        // Update last rotation time
        {
            let mut last_rotation = self.last_rotation.write().await;
            *last_rotation = Utc::now();
        }

        tracing::info!("Rotated {} wallets using {:?} strategy", 
                      rotated_addresses.len(), self.policy.strategy);

        Ok(rotated_addresses)
    }

    async fn rotate_round_robin(&self, wallets: &[WalletInfo]) -> Result<Vec<Address>> {
        let mut index = self.rotation_index.write().await;
        let mut rotated = Vec::new();

        // Rotate through wallets in order
        for _ in 0..wallets.len() {
            let wallet = &wallets[*index % wallets.len()];
            rotated.push(wallet.address);
            *index += 1;
        }

        Ok(rotated)
    }

    async fn rotate_least_used(&self, wallets: &[WalletInfo]) -> Result<Vec<Address>> {
        let mut wallet_usage: Vec<(Address, u64)> = Vec::new();

        for wallet in wallets {
            let usage_count = wallet.total_transactions;
            wallet_usage.push((wallet.address, usage_count));
        }

        // Sort by usage count (ascending)
        wallet_usage.sort_by_key(|(_, count)| *count);

        let rotated: Vec<Address> = wallet_usage
            .into_iter()
            .map(|(address, _)| address)
            .collect();

        Ok(rotated)
    }

    async fn rotate_best_performance(&self, wallets: &[WalletInfo]) -> Result<Vec<Address>> {
        let mut wallet_performance: Vec<(Address, f64)> = Vec::new();

        for wallet in wallets {
            wallet_performance.push((wallet.address, wallet.success_rate));
        }

        // Sort by performance (descending)
        wallet_performance.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());

        let rotated: Vec<Address> = wallet_performance
            .into_iter()
            .map(|(address, _)| address)
            .collect();

        Ok(rotated)
    }

    async fn rotate_load_balanced(&self, wallets: &[WalletInfo]) -> Result<Vec<Address>> {
        // This would implement load balancing based on current active transactions
        // For now, we'll use round robin as a fallback
        self.rotate_round_robin(wallets).await
    }

    async fn rotate_time_based(&self, wallets: &[WalletInfo]) -> Result<Vec<Address>> {
        let mut wallet_times: Vec<(Address, DateTime<Utc>)> = Vec::new();

        for wallet in wallets {
            wallet_times.push((wallet.address, wallet.last_used));
        }

        // Sort by last used time (ascending - oldest first)
        wallet_times.sort_by_key(|(_, time)| *time);

        let rotated: Vec<Address> = wallet_times
            .into_iter()
            .map(|(address, _)| address)
            .collect();

        Ok(rotated)
    }

    async fn rotate_random(&self, wallets: &[WalletInfo]) -> Result<Vec<Address>> {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        let mut addresses: Vec<Address> = wallets.iter().map(|w| w.address).collect();
        addresses.shuffle(&mut thread_rng());

        Ok(addresses)
    }

    async fn check_performance_rotation(&self) -> Result<bool> {
        let wallets = self.wallets.read().await;
        
        for wallet in wallets.iter() {
            if wallet.success_rate < self.policy.performance_threshold {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn check_load_rotation(&self) -> Result<bool> {
        let wallets = self.wallets.read().await;
        
        for wallet in wallets.iter() {
            if wallet.total_transactions > self.policy.max_usage_before_rotation {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn update_rotation_stats(&self, address: Address, reason: RotationReason) -> Result<()> {
        let mut stats = self.rotation_stats.write().await;
        
        let rotation_stats = stats.entry(address).or_insert(WalletRotationStats {
            address,
            total_rotations: 0,
            last_rotation: Utc::now(),
            rotation_reason: reason.clone(),
            performance_score: 0.0,
        });

        rotation_stats.total_rotations += 1;
        rotation_stats.last_rotation = Utc::now();
        rotation_stats.rotation_reason = reason;

        Ok(())
    }

    pub async fn get_rotation_stats(&self, address: Address) -> Result<Option<WalletRotationStats>> {
        let stats = self.rotation_stats.read().await;
        Ok(stats.get(&address).cloned())
    }

    pub async fn get_all_rotation_stats(&self) -> Result<Vec<WalletRotationStats>> {
        let stats = self.rotation_stats.read().await;
        Ok(stats.values().cloned().collect())
    }

    pub async fn set_rotation_policy(&mut self, policy: RotationPolicy) {
        self.policy = policy;
    }

    pub async fn force_rotation(&self, reason: RotationReason) -> Result<Vec<Address>> {
        let wallets = self.wallets.read().await.clone();
        let rotated_addresses = self.rotate_wallets().await?;

        // Update rotation stats with the forced reason
        for address in &rotated_addresses {
            self.update_rotation_stats(*address, reason.clone()).await?;
        }

        Ok(rotated_addresses)
    }

    pub async fn get_rotation_summary(&self) -> Result<RotationSummary> {
        let stats = self.rotation_stats.read().await;
        let wallets = self.wallets.read().await;

        let total_rotations: u64 = stats.values().map(|s| s.total_rotations).sum();
        let average_rotations = if !stats.is_empty() {
            total_rotations as f64 / stats.len() as f64
        } else {
            0.0
        };

        let last_rotation = *self.last_rotation.read().await;
        let time_since_last_rotation = Utc::now() - last_rotation;

        Ok(RotationSummary {
            total_wallets: wallets.len(),
            total_rotations,
            average_rotations_per_wallet: average_rotations,
            last_rotation_time: last_rotation,
            time_since_last_rotation_seconds: time_since_last_rotation.num_seconds(),
            current_strategy: self.policy.strategy.clone(),
            rotation_interval_seconds: self.policy.interval.as_secs(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RotationSummary {
    pub total_wallets: usize,
    pub total_rotations: u64,
    pub average_rotations_per_wallet: f64,
    pub last_rotation_time: DateTime<Utc>,
    pub time_since_last_rotation_seconds: i64,
    pub current_strategy: RotationStrategy,
    pub rotation_interval_seconds: u64,
}

impl Clone for WalletRotator {
    fn clone(&self) -> Self {
        Self {
            wallets: Arc::clone(&self.wallets),
            rotation_stats: Arc::clone(&self.rotation_stats),
            policy: self.policy.clone(),
            last_rotation: Arc::clone(&self.last_rotation),
            rotation_index: Arc::clone(&self.rotation_index),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_rotation_policy_creation() {
        let policy = RotationPolicy {
            strategy: RotationStrategy::RoundRobin,
            interval: Duration::from_secs(300),
            min_usage_before_rotation: 10,
            max_usage_before_rotation: 100,
            performance_threshold: 0.8,
        };

        assert_eq!(policy.interval.as_secs(), 300);
        assert_eq!(policy.min_usage_before_rotation, 10);
    }

    #[tokio::test]
    async fn test_wallet_rotator_creation() {
        let wallets = Arc::new(RwLock::new(Vec::new()));
        let policy = RotationPolicy {
            strategy: RotationStrategy::RoundRobin,
            interval: Duration::from_secs(300),
            min_usage_before_rotation: 10,
            max_usage_before_rotation: 100,
            performance_threshold: 0.8,
        };

        let rotator = WalletRotator::new(wallets, policy);
        
        let summary = rotator.get_rotation_summary().await.unwrap();
        assert_eq!(summary.total_wallets, 0);
        assert_eq!(summary.total_rotations, 0);
    }

    #[tokio::test]
    async fn test_rotation_strategies() {
        let wallets = Arc::new(RwLock::new(Vec::new()));
        let policy = RotationPolicy {
            strategy: RotationStrategy::Random,
            interval: Duration::from_secs(300),
            min_usage_before_rotation: 10,
            max_usage_before_rotation: 100,
            performance_threshold: 0.8,
        };

        let rotator = WalletRotator::new(wallets, policy);
        
        // Test that rotation strategies can be created
        assert!(matches!(policy.strategy, RotationStrategy::Random));
    }
}
