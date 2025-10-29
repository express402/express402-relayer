use alloy::{
    primitives::U256,
    providers::RootProvider,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

use crate::types::{RelayerError, Result};

#[derive(Debug, Clone)]
pub struct GasPriceOracle {
    provider: Arc<RootProvider>,
    current_gas_price: Arc<RwLock<GasPriceInfo>>,
    gas_price_history: Arc<RwLock<Vec<GasPriceSnapshot>>>,
    update_interval: Duration,
    multiplier: f64,
    min_gas_price: U256,
    max_gas_price: U256,
    max_history_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasPriceInfo {
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub base_fee: U256,
    pub timestamp: DateTime<Utc>,
    pub block_number: u64,
}

#[derive(Debug, Clone)]
struct GasPriceSnapshot {
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
    timestamp: Instant,
    block_number: u64,
}

impl GasPriceOracle {
    pub fn new(
        provider: Arc<RootProvider>,
        multiplier: f64,
        min_gas_price: U256,
        max_gas_price: U256,
        update_interval: Duration,
    ) -> Self {
        Self {
            provider,
            current_gas_price: Arc::new(RwLock::new(GasPriceInfo {
                max_fee_per_gas: U256::from(20000000000u64), // 20 gwei default
                max_priority_fee_per_gas: U256::from(2000000000u64), // 2 gwei default
                base_fee: U256::from(20000000000u64),
                timestamp: Utc::now(),
                block_number: 0,
            })),
            gas_price_history: Arc::new(RwLock::new(Vec::new())),
            update_interval,
            multiplier,
            min_gas_price,
            max_gas_price,
            max_history_size: 1000,
        }
    }

    /// Fetch current gas price from the network
    pub async fn fetch_gas_price(&self) -> Result<GasPriceInfo> {
        // Get latest block to get base fee
        let latest_block = self.provider
            .get_block_by_number(alloy::rpc::types::BlockId::latest(), false)
            .await
            .map_err(|e| RelayerError::Ethereum(format!("Failed to get latest block: {}", e)))?
            .ok_or_else(|| RelayerError::Ethereum("Latest block not found".to_string()))?;

        let block_number = latest_block.header.number
            .ok_or_else(|| RelayerError::Ethereum("Block number missing".to_string()))?
            .to::<u64>();

        // Get base fee per gas (EIP-1559)
        let base_fee = latest_block.header.base_fee
            .unwrap_or(U256::from(20000000000u64)); // Default 20 gwei if not available

        // Get gas price from the provider
        let gas_price = self.provider
            .get_gas_price()
            .await
            .map_err(|e| RelayerError::Ethereum(format!("Failed to get gas price: {}", e)))?;

        // Calculate max fee per gas (base fee + priority fee)
        // Priority fee is typically 1-2 gwei
        let priority_fee = U256::from(2000000000u64); // 2 gwei
        let max_fee_per_gas = base_fee + priority_fee;

        // Apply multiplier
        let adjusted_max_fee = (max_fee_per_gas.to::<u64>() as f64 * self.multiplier) as u64;
        let adjusted_max_fee = U256::from(adjusted_max_fee);

        // Clamp to min/max bounds
        let max_fee_per_gas = adjusted_max_fee
            .max(self.min_gas_price)
            .min(self.max_gas_price);

        let max_priority_fee_per_gas = priority_fee.max(self.min_gas_price);

        let gas_info = GasPriceInfo {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            base_fee,
            timestamp: Utc::now(),
            block_number,
        };

        // Update current gas price
        {
            let mut current = self.current_gas_price.write().await;
            *current = gas_info.clone();
        }

        // Add to history
        {
            let mut history = self.gas_price_history.write().await;
            history.push(GasPriceSnapshot {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                timestamp: Instant::now(),
                block_number,
            });

            // Keep only recent history
            if history.len() > self.max_history_size {
                history.remove(0);
            }
        }

        tracing::debug!(
            "Gas price updated: max_fee={} gwei, priority_fee={} gwei, base_fee={} gwei",
            max_fee_per_gas.to::<u64>() / 1_000_000_000,
            max_priority_fee_per_gas.to::<u64>() / 1_000_000_000,
            base_fee.to::<u64>() / 1_000_000_000
        );

        Ok(gas_info)
    }

    /// Get current gas price (cached)
    pub async fn get_current_gas_price(&self) -> GasPriceInfo {
        self.current_gas_price.read().await.clone()
    }

    /// Get recommended gas price for a transaction
    pub async fn get_recommended_gas_price(&self, priority: &str) -> Result<GasPriceInfo> {
        let current = self.get_current_gas_price().await;

        // Adjust based on priority
        let (priority_multiplier, priority_fee_multiplier) = match priority {
            "critical" => (1.5, 1.3),
            "high" => (1.2, 1.2),
            "normal" => (1.0, 1.0),
            "low" => (0.8, 0.8),
            _ => (1.0, 1.0),
        };

        let recommended_max_fee = (current.max_fee_per_gas.to::<u64>() as f64 * priority_multiplier) as u64;
        let recommended_max_fee = U256::from(recommended_max_fee)
            .max(self.min_gas_price)
            .min(self.max_gas_price);

        let recommended_priority_fee = (current.max_priority_fee_per_gas.to::<u64>() as f64 * priority_fee_multiplier) as u64;
        let recommended_priority_fee = U256::from(recommended_priority_fee)
            .max(self.min_gas_price);

        Ok(GasPriceInfo {
            max_fee_per_gas: recommended_max_fee,
            max_priority_fee_per_gas: recommended_priority_fee,
            base_fee: current.base_fee,
            timestamp: Utc::now(),
            block_number: current.block_number,
        })
    }

    /// Start the gas price update loop
    pub async fn start_update_loop(&self) -> Result<()> {
        // Initial fetch
        if let Err(e) = self.fetch_gas_price().await {
            tracing::warn!("Failed to fetch initial gas price: {}", e);
        }

        let oracle = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(oracle.update_interval);
            
            loop {
                interval.tick().await;
                
                match oracle.fetch_gas_price().await {
                    Ok(_) => {
                        tracing::debug!("Gas price updated successfully");
                    }
                    Err(e) => {
                        tracing::error!("Failed to update gas price: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Get gas price statistics
    pub async fn get_statistics(&self) -> Result<GasPriceStatistics> {
        let history = self.gas_price_history.read().await;
        let current = self.get_current_gas_price().await;

        if history.is_empty() {
            return Ok(GasPriceStatistics {
                current_max_fee: current.max_fee_per_gas,
                current_priority_fee: current.max_priority_fee_per_gas,
                average_max_fee: current.max_fee_per_gas,
                min_max_fee: current.max_fee_per_gas,
                max_max_fee: current.max_fee_per_gas,
                trend: "stable".to_string(),
                last_updated: current.timestamp,
            });
        }

        let mut total_max_fee = U256::ZERO;
        let mut min_max_fee = U256::MAX;
        let mut max_max_fee = U256::ZERO;

        for snapshot in history.iter() {
            total_max_fee = total_max_fee + snapshot.max_fee_per_gas;
            if snapshot.max_fee_per_gas < min_max_fee {
                min_max_fee = snapshot.max_fee_per_gas;
            }
            if snapshot.max_fee_per_gas > max_max_fee {
                max_max_fee = snapshot.max_fee_per_gas;
            }
        }

        let average_max_fee = total_max_fee / U256::from(history.len());

        // Determine trend
        let trend = if history.len() >= 2 {
            let recent = &history[history.len() - 1];
            let previous = &history[history.len() - 2];
            
            if recent.max_fee_per_gas > previous.max_fee_per_gas {
                "increasing"
            } else if recent.max_fee_per_gas < previous.max_fee_per_gas {
                "decreasing"
            } else {
                "stable"
            }
        } else {
            "stable"
        };

        Ok(GasPriceStatistics {
            current_max_fee: current.max_fee_per_gas,
            current_priority_fee: current.max_priority_fee_per_gas,
            average_max_fee,
            min_max_fee,
            max_max_fee,
            trend: trend.to_string(),
            last_updated: current.timestamp,
        })
    }

    /// Update multiplier
    pub fn set_multiplier(&mut self, multiplier: f64) {
        self.multiplier = multiplier;
    }

    /// Update price bounds
    pub fn set_price_bounds(&mut self, min: U256, max: U256) {
        self.min_gas_price = min;
        self.max_gas_price = max;
    }
}

impl Clone for GasPriceOracle {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            current_gas_price: Arc::clone(&self.current_gas_price),
            gas_price_history: Arc::clone(&self.gas_price_history),
            update_interval: self.update_interval,
            multiplier: self.multiplier,
            min_gas_price: self.min_gas_price,
            max_gas_price: self.max_gas_price,
            max_history_size: self.max_history_size,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GasPriceStatistics {
    pub current_max_fee: U256,
    pub current_priority_fee: U256,
    pub average_max_fee: U256,
    pub min_max_fee: U256,
    pub max_max_fee: U256,
    pub trend: String,
    pub last_updated: DateTime<Utc>,
}

