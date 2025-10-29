use alloy::{
    primitives::{Address, U256},
    providers::{Provider, RootProvider},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

use crate::types::{RelayerError, Result, WalletInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletHealthStatus {
    pub address: Address,
    pub is_healthy: bool,
    pub balance: U256,
    pub nonce: U256,
    pub last_checked: DateTime<Utc>,
    pub issues: Vec<String>,
}

pub struct WalletMonitor<P> {
    provider: Arc<P>,
    wallets: Arc<RwLock<Vec<WalletInfo>>>,
    health_status: Arc<RwLock<HashMap<Address, WalletHealthStatus>>>,
    check_interval: Duration,
    min_balance_threshold: U256,
    max_nonce_gap: u64,
}

impl<P> WalletMonitor<P>
where
    P: Provider + Send + Sync + 'static,
{
    pub fn new(
        provider: Arc<P>,
        wallets: Arc<RwLock<Vec<WalletInfo>>>,
        check_interval: Duration,
        min_balance_threshold: U256,
        max_nonce_gap: u64,
    ) -> Self {
        Self {
            provider,
            wallets,
            health_status: Arc::new(RwLock::new(HashMap::new())),
            check_interval,
            min_balance_threshold,
            max_nonce_gap,
        }
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        let monitor = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(monitor.check_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = monitor.check_all_wallets().await {
                    tracing::error!("Wallet monitoring error: {}", e);
                }
            }
        });

        Ok(())
    }

    pub async fn check_all_wallets(&self) -> Result<Vec<WalletHealthStatus>> {
        let wallets = self.wallets.read().await.clone();
        let mut health_statuses = Vec::new();

        for wallet in wallets {
            let health_status = self.check_wallet_health(wallet.address).await?;
            health_statuses.push(health_status.clone());

            {
                let mut status_map = self.health_status.write().await;
                status_map.insert(wallet.address, health_status);
            }
        }

        Ok(health_statuses)
    }

    pub async fn check_wallet_health(&self, address: Address) -> Result<WalletHealthStatus> {
        let mut issues = Vec::new();
        let mut is_healthy = true;

        // Check balance
        let balance = self.get_wallet_balance(address).await?;
        if balance < self.min_balance_threshold {
            issues.push(format!("Low balance: {} wei", balance));
            is_healthy = false;
        }

        // Check nonce
        let nonce = self.get_wallet_nonce(address).await?;
        let expected_nonce = self.get_expected_nonce(address).await?;
        
        if nonce > expected_nonce + self.max_nonce_gap {
            issues.push(format!("Nonce gap too large: {} vs {}", nonce, expected_nonce));
            is_healthy = false;
        }

        // Check if wallet is active
        let wallets = self.wallets.read().await;
        let wallet_info = wallets.iter().find(|w| w.address == address);
        
        if let Some(wallet) = wallet_info {
            if !wallet.is_active {
                issues.push("Wallet is marked as inactive".to_string());
                is_healthy = false;
            }

            if wallet.success_rate < 0.8 {
                issues.push(format!("Low success rate: {:.2}%", wallet.success_rate * 100.0));
                is_healthy = false;
            }
        } else {
            issues.push("Wallet not found in pool".to_string());
            is_healthy = false;
        }

        Ok(WalletHealthStatus {
            address,
            is_healthy,
            balance,
            nonce,
            last_checked: Utc::now(),
            issues,
        })
    }

    pub async fn get_wallet_balance(&self, address: Address) -> Result<U256> {
        let balance = self.provider
            .get_balance(address, alloy::rpc::types::BlockId::latest())
            .await
            .map_err(|e| RelayerError::Ethereum(e.to_string()))?;

        Ok(balance)
    }

    pub async fn get_wallet_nonce(&self, address: Address) -> Result<u64> {
        let nonce = self.provider
            .get_transaction_count(address, alloy::rpc::types::BlockId::latest())
            .await
            .map_err(|e| RelayerError::Ethereum(e.to_string()))?;

        Ok(nonce)
    }

    async fn get_expected_nonce(&self, address: Address) -> Result<u64> {
        // This would typically check pending transactions
        // For now, we'll use the current nonce as expected
        self.get_wallet_nonce(address).await
    }

    pub async fn get_health_status(&self, address: Address) -> Result<Option<WalletHealthStatus>> {
        let status_map = self.health_status.read().await;
        Ok(status_map.get(&address).cloned())
    }

    pub async fn get_all_health_statuses(&self) -> Result<Vec<WalletHealthStatus>> {
        let status_map = self.health_status.read().await;
        Ok(status_map.values().cloned().collect())
    }

    pub async fn get_unhealthy_wallets(&self) -> Result<Vec<Address>> {
        let status_map = self.health_status.read().await;
        let unhealthy: Vec<Address> = status_map
            .values()
            .filter(|status| !status.is_healthy)
            .map(|status| status.address)
            .collect();

        Ok(unhealthy)
    }

    pub async fn update_wallet_status(&self, address: Address, is_active: bool) -> Result<()> {
        let mut wallets = self.wallets.write().await;
        if let Some(wallet) = wallets.iter_mut().find(|w| w.address == address) {
            wallet.is_active = is_active;
        }

        Ok(())
    }

    pub async fn get_monitoring_stats(&self) -> Result<WalletMonitoringStats> {
        let status_map = self.health_status.read().await;
        let wallets = self.wallets.read().await;

        let total_wallets = wallets.len();
        let healthy_wallets = status_map.values().filter(|s| s.is_healthy).count();
        let unhealthy_wallets = total_wallets - healthy_wallets;

        let mut total_issues = 0;
        for status in status_map.values() {
            total_issues += status.issues.len();
        }

        Ok(WalletMonitoringStats {
            total_wallets,
            healthy_wallets,
            unhealthy_wallets,
            total_issues,
            check_interval_seconds: self.check_interval.as_secs(),
            min_balance_threshold: self.min_balance_threshold,
        })
    }

    pub fn set_check_interval(&mut self, interval: Duration) {
        self.check_interval = interval;
    }

    pub fn set_min_balance_threshold(&mut self, threshold: U256) {
        self.min_balance_threshold = threshold;
    }

    pub fn set_max_nonce_gap(&mut self, gap: u64) {
        self.max_nonce_gap = gap;
    }
}

impl<P> Clone for WalletMonitor<P>
where
    P: Provider + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            wallets: Arc::clone(&self.wallets),
            health_status: Arc::clone(&self.health_status),
            check_interval: self.check_interval,
            min_balance_threshold: self.min_balance_threshold,
            max_nonce_gap: self.max_nonce_gap,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletMonitoringStats {
    pub total_wallets: usize,
    pub healthy_wallets: usize,
    pub unhealthy_wallets: usize,
    pub total_issues: usize,
    pub check_interval_seconds: u64,
    pub min_balance_threshold: U256,
}

pub struct WalletAlertManager {
    alert_thresholds: AlertThresholds,
    alert_history: Arc<RwLock<Vec<WalletAlert>>>,
}

#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub min_balance: U256,
    pub max_nonce_gap: u64,
    pub min_success_rate: f64,
    pub max_response_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAlert {
    pub id: uuid::Uuid,
    pub wallet_address: Address,
    pub alert_type: AlertType,
    pub message: String,
    pub severity: AlertSeverity,
    pub timestamp: DateTime<Utc>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    LowBalance,
    HighNonceGap,
    LowSuccessRate,
    SlowResponse,
    WalletOffline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl WalletAlertManager {
    pub fn new(thresholds: AlertThresholds) -> Self {
        Self {
            alert_thresholds: thresholds,
            alert_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn check_and_create_alerts(&self, health_statuses: Vec<WalletHealthStatus>) -> Result<Vec<WalletAlert>> {
        let mut new_alerts = Vec::new();

        for status in health_statuses {
            if !status.is_healthy {
                for issue in &status.issues {
                    let alert = self.create_alert(status.address, issue.clone()).await?;
                    new_alerts.push(alert);
                }
            }
        }

        Ok(new_alerts)
    }

    async fn create_alert(&self, address: Address, message: String) -> Result<WalletAlert> {
        let alert = WalletAlert {
            id: uuid::Uuid::new_v4(),
            wallet_address: address,
            alert_type: self.determine_alert_type(&message),
            message,
            severity: AlertSeverity::Medium, // Default severity
            timestamp: Utc::now(),
            resolved: false,
        };

        {
            let mut history = self.alert_history.write().await;
            history.push(alert.clone());
        }

        Ok(alert)
    }

    fn determine_alert_type(&self, message: &str) -> AlertType {
        if message.contains("Low balance") {
            AlertType::LowBalance
        } else if message.contains("Nonce gap") {
            AlertType::HighNonceGap
        } else if message.contains("success rate") {
            AlertType::LowSuccessRate
        } else if message.contains("response") {
            AlertType::SlowResponse
        } else {
            AlertType::WalletOffline
        }
    }

    pub async fn get_active_alerts(&self) -> Result<Vec<WalletAlert>> {
        let history = self.alert_history.read().await;
        let active_alerts: Vec<WalletAlert> = history
            .iter()
            .filter(|alert| !alert.resolved)
            .cloned()
            .collect();

        Ok(active_alerts)
    }

    pub async fn resolve_alert(&self, alert_id: uuid::Uuid) -> Result<()> {
        let mut history = self.alert_history.write().await;
        if let Some(alert) = history.iter_mut().find(|a| a.id == alert_id) {
            alert.resolved = true;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    #[tokio::test]
    async fn test_wallet_monitor_creation() {
        // This would require a mock provider in a real test
        let min_balance = U256::from(1000000000000000000u64);
        let check_interval = Duration::from_secs(60);
        
        // In a real test, you would create a mock provider
        // let provider = Arc::new(MockProvider::new());
        // let wallets = Arc::new(RwLock::new(Vec::new()));
        // let monitor = WalletMonitor::new(provider, wallets, check_interval, min_balance, 10);
        
        assert_eq!(min_balance, U256::from(1000000000000000000u64));
        assert_eq!(check_interval.as_secs(), 60);
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let thresholds = AlertThresholds {
            min_balance: U256::from(1000000000000000000u64),
            max_nonce_gap: 10,
            min_success_rate: 0.8,
            max_response_time: Duration::from_secs(30),
        };

        let alert_manager = WalletAlertManager::new(thresholds);
        
        let active_alerts = alert_manager.get_active_alerts().await.unwrap();
        assert_eq!(active_alerts.len(), 0);
    }
}
