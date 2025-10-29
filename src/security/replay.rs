use alloy::primitives::Address;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::time::{Duration, Instant};

use crate::types::{RelayerError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayProtectionEntry {
    pub timestamp: DateTime<Utc>,
    pub nonce: u64,
    pub tx_hash: Option<String>,
}

pub struct ReplayProtection {
    entries: Arc<RwLock<HashMap<Address, Vec<ReplayProtectionEntry>>>>,
    window_duration: Duration,
    cleanup_interval: Duration,
}

impl ReplayProtection {
    pub fn new(window_duration: Duration, cleanup_interval: Duration) -> Self {
        let protection = Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            window_duration,
            cleanup_interval,
        };

        // Start cleanup task
        protection.start_cleanup_task();
        protection
    }

    pub fn check_and_record(
        &self,
        address: Address,
        nonce: u64,
        tx_hash: Option<String>,
    ) -> Result<bool> {
        let now = Utc::now();
        let cutoff_time = now - chrono::Duration::from_std(self.window_duration)
            .map_err(|e| RelayerError::Internal(e.to_string()))?;

        let mut entries = self.entries.write()
            .map_err(|e| RelayerError::Internal(e.to_string()))?;

        let user_entries = entries.entry(address).or_insert_with(Vec::new);

        // Check for replay attacks
        for entry in user_entries.iter() {
            if entry.nonce == nonce && entry.timestamp > cutoff_time {
                return Err(RelayerError::ReplayAttack(format!(
                    "Nonce {} already used recently for address {:?}",
                    nonce, address
                )));
            }
        }

        // Record the new transaction
        user_entries.push(ReplayProtectionEntry {
            timestamp: now,
            nonce,
            tx_hash,
        });

        // Clean up old entries for this user
        user_entries.retain(|entry| entry.timestamp > cutoff_time);

        Ok(true)
    }

    pub fn is_nonce_used(&self, address: Address, nonce: u64) -> Result<bool> {
        let now = Utc::now();
        let cutoff_time = now - chrono::Duration::from_std(self.window_duration)
            .map_err(|e| RelayerError::Internal(e.to_string()))?;

        let entries = self.entries.read()
            .map_err(|e| RelayerError::Internal(e.to_string()))?;

        if let Some(user_entries) = entries.get(&address) {
            for entry in user_entries.iter() {
                if entry.nonce == nonce && entry.timestamp > cutoff_time {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn get_next_nonce(&self, address: Address) -> Result<u64> {
        let now = Utc::now();
        let cutoff_time = now - chrono::Duration::from_std(self.window_duration)
            .map_err(|e| RelayerError::Internal(e.to_string()))?;

        let entries = self.entries.read()
            .map_err(|e| RelayerError::Internal(e.to_string()))?;

        let mut max_nonce = 0u64;

        if let Some(user_entries) = entries.get(&address) {
            for entry in user_entries.iter() {
                if entry.timestamp > cutoff_time && entry.nonce > max_nonce {
                    max_nonce = entry.nonce;
                }
            }
        }

        Ok(max_nonce + 1)
    }

    fn start_cleanup_task(&self) {
        let entries = Arc::clone(&self.entries);
        let window_duration = self.window_duration;
        let cleanup_interval = self.cleanup_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            
            loop {
                interval.tick().await;
                
                let now = Utc::now();
                let cutoff_time = match now.checked_sub_signed(
                    chrono::Duration::from_std(window_duration).unwrap_or_default()
                ) {
                    Some(time) => time,
                    None => continue,
                };

                if let Ok(mut entries_guard) = entries.write() {
                    entries_guard.retain(|_, user_entries| {
                        user_entries.retain(|entry| entry.timestamp > cutoff_time);
                        !user_entries.is_empty()
                    });
                }
            }
        });
    }

    pub fn get_stats(&self) -> Result<ReplayProtectionStats> {
        let entries = self.entries.read()
            .map_err(|e| RelayerError::Internal(e.to_string()))?;

        let mut total_entries = 0;
        let mut unique_addresses = 0;

        for (_, user_entries) in entries.iter() {
            total_entries += user_entries.len();
            unique_addresses += 1;
        }

        Ok(ReplayProtectionStats {
            total_entries,
            unique_addresses,
            window_duration_seconds: self.window_duration.as_secs(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayProtectionStats {
    pub total_entries: usize,
    pub unique_addresses: usize,
    pub window_duration_seconds: u64,
}

impl Default for ReplayProtection {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(3600), // 1 hour window
            Duration::from_secs(300),   // 5 minute cleanup interval
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    #[tokio::test]
    async fn test_replay_protection() {
        let protection = ReplayProtection::new(
            Duration::from_secs(60),
            Duration::from_secs(10),
        );

        let address = Address::ZERO;
        let nonce = 1u64;

        // First transaction should be allowed
        assert!(protection.check_and_record(address, nonce, None).is_ok());

        // Same nonce should be rejected
        assert!(protection.check_and_record(address, nonce, None).is_err());

        // Different nonce should be allowed
        assert!(protection.check_and_record(address, nonce + 1, None).is_ok());
    }

    #[tokio::test]
    async fn test_nonce_checking() {
        let protection = ReplayProtection::new(
            Duration::from_secs(60),
            Duration::from_secs(10),
        );

        let address = Address::ZERO;
        let nonce = 1u64;

        // Initially nonce should not be used
        assert!(!protection.is_nonce_used(address, nonce).unwrap());

        // After recording, nonce should be used
        protection.check_and_record(address, nonce, None).unwrap();
        assert!(protection.is_nonce_used(address, nonce).unwrap());
    }

    #[tokio::test]
    async fn test_next_nonce() {
        let protection = ReplayProtection::new(
            Duration::from_secs(60),
            Duration::from_secs(10),
        );

        let address = Address::ZERO;

        // First nonce should be 1
        assert_eq!(protection.get_next_nonce(address).unwrap(), 1);

        // After recording nonce 1, next should be 2
        protection.check_and_record(address, 1, None).unwrap();
        assert_eq!(protection.get_next_nonce(address).unwrap(), 2);
    }
}
