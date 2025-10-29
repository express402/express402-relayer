use alloy::{
    primitives::{B256, U256},
    providers::RootProvider,
};
use chrono::Utc;
use hex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

use crate::{
    database::DatabaseManager,
    types::{RelayerError, Result, TransactionStatus},
};

#[derive(Debug, Clone)]
pub struct TransactionTracker {
    database: Arc<DatabaseManager>,
    ethereum_provider: Arc<RootProvider>,
    pending_transactions: Arc<RwLock<HashMap<String, PendingTransaction>>>,
    check_interval: Duration,
    confirmation_blocks: u64,
}

#[derive(Debug, Clone)]
struct PendingTransaction {
    transaction_id: Uuid,
    tx_hash: String,
    submitted_at: Instant,
    last_checked: Instant,
    check_count: u32,
}

impl TransactionTracker {
    pub fn new(
        database: Arc<DatabaseManager>,
        ethereum_provider: Arc<RootProvider>,
        check_interval: Duration,
        confirmation_blocks: u64,
    ) -> Self {
        Self {
            database,
            ethereum_provider,
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
            check_interval,
            confirmation_blocks,
        }
    }

    /// Add a transaction to track
    pub async fn add_transaction(&self, transaction_id: Uuid, tx_hash: String) -> Result<()> {
        let pending = PendingTransaction {
            transaction_id,
            tx_hash: tx_hash.clone(),
            submitted_at: Instant::now(),
            last_checked: Instant::now(),
            check_count: 0,
        };

        let mut pending_map = self.pending_transactions.write().await;
        pending_map.insert(tx_hash.clone(), pending);

        tracing::info!("Added transaction {} ({}) to tracker", transaction_id, tx_hash);
        Ok(())
    }

    /// Check transaction status on blockchain
    pub async fn check_transaction(&self, tx_hash: &str) -> Result<TransactionCheckResult> {
        // Parse transaction hash
        let tx_hash_str = tx_hash.strip_prefix("0x").unwrap_or(tx_hash);
        let tx_hash_bytes = hex::decode(tx_hash_str)
            .map_err(|e| RelayerError::Ethereum(format!("Invalid tx hash format: {}", e)))?;
        
        if tx_hash_bytes.len() != 32 {
            return Err(RelayerError::Ethereum(format!("Invalid tx hash length: {}", tx_hash_bytes.len())));
        }

        let tx_hash_array: [u8; 32] = tx_hash_bytes.try_into()
            .map_err(|_| RelayerError::Ethereum("Failed to convert tx hash to array".to_string()))?;
        let tx_hash_bytes = B256::from(tx_hash_array);

        let receipt = match self.ethereum_provider
            .get_transaction_receipt(tx_hash_bytes)
            .await
        {
            Ok(Some(receipt)) => receipt,
            Ok(None) => {
                // Transaction not yet mined
                return Ok(TransactionCheckResult::Pending);
            }
            Err(e) => {
                tracing::error!("Failed to get transaction receipt: {}", e);
                return Err(RelayerError::Ethereum(format!("Failed to get receipt: {}", e)));
            }
        };

        // Get current block number
        let current_block = self.ethereum_provider
            .get_block_number()
            .await
            .map_err(|e| RelayerError::Ethereum(format!("Failed to get block number: {}", e)))?;

        let block_number = receipt.block_number
            .ok_or_else(|| RelayerError::Ethereum("Receipt missing block number".to_string()))?;

        let block_number_u64 = block_number.to::<u64>();
        let current_block_u64 = current_block.to::<u64>();
        let confirmations = current_block_u64.saturating_sub(block_number_u64);

        let gas_used = receipt.gas_used.unwrap_or(U256::ZERO);
        let gas_used_str = gas_used.to_string();

        if receipt.status == Some(1) {
            // Transaction successful
            if confirmations >= self.confirmation_blocks {
                Ok(TransactionCheckResult::Confirmed {
                    block_number: block_number_u64,
                    gas_used: gas_used_str,
                    confirmations,
                })
            } else {
                Ok(TransactionCheckResult::Processing {
                    block_number: block_number_u64,
                    confirmations,
                    required_confirmations: self.confirmation_blocks,
                })
            }
        } else {
            // Transaction failed
            Ok(TransactionCheckResult::Failed {
                block_number: block_number_u64,
                gas_used: gas_used_str,
            })
        }
    }

    /// Process all pending transactions
    pub async fn process_pending_transactions(&self) -> Result<ProcessResult> {
        let mut processed = 0;
        let mut confirmed = 0;
        let mut failed = 0;
        let mut errors = 0;

        // Get pending transactions
        let pending_map = self.pending_transactions.read().await.clone();
        
        for (tx_hash, pending) in pending_map.iter() {
            // Check if enough time has passed since last check
            if pending.last_checked.elapsed() < self.check_interval {
                continue;
            }

            match self.check_transaction(tx_hash).await {
                Ok(result) => {
                    processed += 1;

                    match result {
                        TransactionCheckResult::Confirmed { block_number, gas_used, .. } => {
                            confirmed += 1;
                            
                            // Update database
                            if let Err(e) = self.database.update_transaction_status(
                                pending.transaction_id,
                                TransactionStatus::Confirmed,
                                Some(tx_hash.clone()),
                                Some(block_number),
                                Some(gas_used),
                                None,
                            ).await {
                                tracing::error!("Failed to update transaction {}: {}", pending.transaction_id, e);
                                errors += 1;
                            } else {
                                tracing::info!("Transaction {} confirmed in block {}", pending.transaction_id, block_number);
                                
                                // Remove from pending
                                let mut pending_map = self.pending_transactions.write().await;
                                pending_map.remove(tx_hash);
                            }
                        }
                        TransactionCheckResult::Failed { block_number, gas_used } => {
                            failed += 1;
                            
                            // Update database
                            if let Err(e) = self.database.update_transaction_status(
                                pending.transaction_id,
                                TransactionStatus::Failed,
                                Some(tx_hash.clone()),
                                Some(block_number),
                                Some(gas_used),
                                Some("Transaction reverted".to_string()),
                            ).await {
                                tracing::error!("Failed to update transaction {}: {}", pending.transaction_id, e);
                                errors += 1;
                            } else {
                                tracing::warn!("Transaction {} failed in block {}", pending.transaction_id, block_number);
                                
                                // Remove from pending
                                let mut pending_map = self.pending_transactions.write().await;
                                pending_map.remove(tx_hash);
                            }
                        }
                        TransactionCheckResult::Processing { .. } => {
                            // Update last checked time
                            let mut pending_map = self.pending_transactions.write().await;
                            if let Some(pending) = pending_map.get_mut(tx_hash) {
                                pending.last_checked = Instant::now();
                                pending.check_count += 1;
                                
                                // Update database status to processing
                                let _ = self.database.update_transaction_status(
                                    pending.transaction_id,
                                    TransactionStatus::Processing,
                                    Some(tx_hash.clone()),
                                    None,
                                    None,
                                    None,
                                ).await;
                            }
                        }
                        TransactionCheckResult::Pending => {
                            // Still pending, update last checked time
                            let mut pending_map = self.pending_transactions.write().await;
                            if let Some(pending) = pending_map.get_mut(tx_hash) {
                                pending.last_checked = Instant::now();
                                pending.check_count += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error checking transaction {}: {}", tx_hash, e);
                    errors += 1;
                }
            }
        }

        Ok(ProcessResult {
            processed,
            confirmed,
            failed,
            errors,
            pending_count: self.pending_transactions.read().await.len(),
        })
    }

    /// Start the tracking loop
    pub async fn start_tracking_loop(&self) -> Result<()> {
        tracing::info!("Starting transaction tracking loop...");

        let tracker = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tracker.check_interval);
            
            loop {
                interval.tick().await;
                
                match tracker.process_pending_transactions().await {
                    Ok(result) => {
                        if result.processed > 0 {
                            tracing::debug!(
                                "Processed {} transactions: {} confirmed, {} failed, {} errors, {} pending",
                                result.processed,
                                result.confirmed,
                                result.failed,
                                result.errors,
                                result.pending_count
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!("Transaction tracking error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Get tracking statistics
    pub async fn get_tracking_stats(&self) -> Result<TrackingStats> {
        let pending_map = self.pending_transactions.read().await;
        
        let total_pending = pending_map.len();
        let oldest_pending = pending_map.values()
            .map(|p| p.submitted_at)
            .min()
            .map(|t| t.elapsed().as_secs());

        Ok(TrackingStats {
            total_pending,
            oldest_pending_seconds: oldest_pending,
            check_interval_seconds: self.check_interval.as_secs(),
            confirmation_blocks: self.confirmation_blocks,
        })
    }

    /// Remove a transaction from tracking (e.g., if manually cancelled)
    pub async fn remove_transaction(&self, tx_hash: &str) -> Result<bool> {
        let mut pending_map = self.pending_transactions.write().await;
        Ok(pending_map.remove(tx_hash).is_some())
    }
}

impl Clone for TransactionTracker {
    fn clone(&self) -> Self {
        Self {
            database: Arc::clone(&self.database),
            ethereum_provider: Arc::clone(&self.ethereum_provider),
            pending_transactions: Arc::clone(&self.pending_transactions),
            check_interval: self.check_interval,
            confirmation_blocks: self.confirmation_blocks,
        }
    }
}

#[derive(Debug)]
pub enum TransactionCheckResult {
    Pending,
    Processing {
        block_number: u64,
        confirmations: u64,
        required_confirmations: u64,
    },
    Confirmed {
        block_number: u64,
        gas_used: String,
        confirmations: u64,
    },
    Failed {
        block_number: u64,
        gas_used: String,
    },
}

#[derive(Debug)]
pub struct ProcessResult {
    pub processed: usize,
    pub confirmed: usize,
    pub failed: usize,
    pub errors: usize,
    pub pending_count: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct TrackingStats {
    pub total_pending: usize,
    pub oldest_pending_seconds: Option<u64>,
    pub check_interval_seconds: u64,
    pub confirmation_blocks: u64,
}

