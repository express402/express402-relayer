use alloy::{
    primitives::{Address, U256},
    providers::RootProvider,
    rpc::types::TransactionRequest as AlloyTransactionRequest,
    signers::{k256::ecdsa::SigningKey, LocalWallet},
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

use crate::{
    database::DatabaseManager,
    queue::scheduler::{ScheduledTask, TaskScheduler},
    queue::tracker::TransactionTracker,
    types::{RelayerError, Result, TransactionRequest, TransactionStatus},
    wallet::pool::WalletPool,
    wallet::WalletInfo,
    utils::gas::GasPriceOracle,
};

#[derive(Debug, Clone)]
pub struct TaskExecutor {
    task_scheduler: Arc<TaskScheduler>,
    wallet_pool: Arc<WalletPool>,
    database: Arc<DatabaseManager>,
    ethereum_provider: Arc<RootProvider>,
    transaction_tracker: Option<Arc<TransactionTracker>>,
    gas_price_oracle: Option<Arc<GasPriceOracle>>,
    max_retries: u32,
    retry_delay: Duration,
}

pub struct ExecutionResult {
    pub task_id: Uuid,
    pub success: bool,
    pub tx_hash: Option<String>,
    pub error_message: Option<String>,
    pub execution_time: Duration,
}

impl TaskExecutor {
    pub fn new(
        task_scheduler: Arc<TaskScheduler>,
        wallet_pool: Arc<WalletPool>,
        database: Arc<DatabaseManager>,
        ethereum_provider: Arc<RootProvider>,
        transaction_tracker: Option<Arc<TransactionTracker>>,
        gas_price_oracle: Option<Arc<GasPriceOracle>>,
        max_retries: u32,
        retry_delay: Duration,
    ) -> Self {
        Self {
            task_scheduler,
            wallet_pool,
            database,
            ethereum_provider,
            transaction_tracker,
            gas_price_oracle,
            max_retries,
            retry_delay,
        }
    }

    /// Execute a scheduled task
    pub async fn execute_task(&self, task: ScheduledTask) -> Result<ExecutionResult> {
        let start_time = Instant::now();
        let task_id = task.id;

        tracing::info!("Executing task {} (priority: {})", task_id, task.priority);

        // Acquire a wallet from the pool
        let wallet_info = match self.wallet_pool.acquire_wallet().await? {
            Some(wallet) => wallet,
            None => {
                let error = "No available wallet in pool".to_string();
                tracing::error!("Task {} failed: {}", task_id, error);
                return Ok(ExecutionResult {
                    task_id,
                    success: false,
                    tx_hash: None,
                    error_message: Some(error),
                    execution_time: start_time.elapsed(),
                });
            }
        };

        // Execute the transaction
        let result = match self.execute_transaction(&task.request, &wallet_info).await {
            Ok(tx_hash) => {
                tracing::info!("Task {} executed successfully, tx_hash: {}", task_id, tx_hash);

                // Update database status
                if let Err(e) = self.database.update_transaction_status(
                    task_id,
                    TransactionStatus::Submitted,
                    Some(tx_hash.clone()),
                    None, // block_number
                    None, // gas_used
                    None, // error_message
                ).await {
                    tracing::error!("Failed to update transaction status: {}", e);
                }

                // Add to transaction tracker for status monitoring
                if let Some(ref tracker) = self.transaction_tracker {
                    if let Err(e) = tracker.add_transaction(task_id, tx_hash.clone()).await {
                        tracing::warn!("Failed to add transaction to tracker: {}", e);
                    }
                }

                // Release wallet with success status
                let _ = self.wallet_pool.release_wallet(
                    wallet_info.address,
                    true,
                    0, // Gas used will be updated later when confirmed
                ).await;

                ExecutionResult {
                    task_id,
                    success: true,
                    tx_hash: Some(tx_hash),
                    error_message: None,
                    execution_time: start_time.elapsed(),
                }
            }
            Err(e) => {
                tracing::error!("Task {} execution failed: {}", task_id, e);

                // Update database status
                let _ = self.database.update_transaction_status(
                    task_id,
                    TransactionStatus::Failed,
                    None, // tx_hash
                    None, // block_number
                    None, // gas_used
                    Some(e.to_string()), // error_message
                ).await;

                // Release wallet with failure status
                let _ = self.wallet_pool.release_wallet(
                    wallet_info.address,
                    false,
                    0,
                ).await;

                ExecutionResult {
                    task_id,
                    success: false,
                    tx_hash: None,
                    error_message: Some(e.to_string()),
                    execution_time: start_time.elapsed(),
                }
            }
        };

        Ok(result)
    }

    /// Execute a transaction using the provided wallet
    async fn execute_transaction(
        &self,
        request: &TransactionRequest,
        wallet_info: &WalletInfo,
    ) -> Result<String> {
        // Get the current nonce for the wallet
        let wallet_nonce = self.ethereum_provider
            .get_transaction_count(wallet_info.address, alloy::rpc::types::BlockId::latest())
            .await
            .map_err(|e| RelayerError::Ethereum(format!("Failed to get wallet nonce: {}", e)))?;

        // Get chain ID
        let chain_id = self.ethereum_provider
            .get_chain_id()
            .await
            .map_err(|e| RelayerError::Ethereum(format!("Failed to get chain ID: {}", e)))?;

        // Determine gas prices - use oracle if available and user's gas price is too low
        let (max_fee_per_gas, max_priority_fee_per_gas) = if let Some(ref oracle) = self.gas_price_oracle {
            let current_gas = oracle.get_current_gas_price().await;
            let priority_str = match request.priority {
                crate::types::Priority::Critical => "critical",
                crate::types::Priority::High => "high",
                crate::types::Priority::Normal => "normal",
                crate::types::Priority::Low => "low",
            };
            
            // Use oracle-recommended gas price if user's gas price is less than 80% of current
            let recommended_gas = oracle.get_recommended_gas_price(priority_str).await
                .unwrap_or(current_gas.clone());
            
            let user_max_fee = request.max_fee_per_gas.to::<u64>() as f64;
            let recommended_max_fee = recommended_gas.max_fee_per_gas.to::<u64>() as f64;
            
            if user_max_fee < recommended_max_fee * 0.8 {
                // User's gas price is too low, use recommended price
                tracing::info!(
                    "Adjusting gas price from {} to {} gwei (recommended: {} gwei)",
                    user_max_fee / 1_000_000_000.0,
                    recommended_gas.max_fee_per_gas.to::<u64>() as f64 / 1_000_000_000.0,
                    recommended_max_fee / 1_000_000_000.0
                );
                (recommended_gas.max_fee_per_gas, recommended_gas.max_priority_fee_per_gas)
            } else {
                // User's gas price is acceptable, use it
                (request.max_fee_per_gas, request.max_priority_fee_per_gas)
            }
        } else {
            // No oracle available, use user's gas price
            (request.max_fee_per_gas, request.max_priority_fee_per_gas)
        };

        // Build and sign transaction
        if let Some(ref private_key) = wallet_info.private_key {
            let wallet = LocalWallet::from(
                SigningKey::from_bytes(private_key.as_bytes())
                    .map_err(|e| RelayerError::Internal(format!("Invalid private key: {}", e)))?
            );

            // Build transaction request - using the wallet's nonce, not the user's nonce
            let tx_request = AlloyTransactionRequest::default()
                .with_from(wallet.address())
                .with_to(request.target_contract)
                .with_value(request.value)
                .with_input(request.calldata.clone())
                .with_gas_limit(request.gas_limit.to::<u64>())
                .with_max_fee_per_gas(max_fee_per_gas)
                .with_max_priority_fee_per_gas(max_priority_fee_per_gas)
                .with_nonce(wallet_nonce.to::<u64>())
                .with_chain_id(chain_id.to::<u64>());

            // Send transaction using the wallet signer
            let pending_tx = self.ethereum_provider
                .send_transaction(tx_request)
                .await
                .map_err(|e| RelayerError::Ethereum(format!("Failed to send transaction: {}", e)))?;

            let tx_hash = format!("{:?}", pending_tx.tx_hash());
            tracing::info!("Transaction sent: {}", tx_hash);

            Ok(tx_hash)
        } else {
            Err(RelayerError::Internal("Wallet private key not available".to_string()))
        }
    }

    /// Start the task execution loop
    pub async fn start_execution_loop(&self) -> Result<()> {
        tracing::info!("Starting task execution loop...");

        loop {
            // Get next task from scheduler
            match self.task_scheduler.get_next_task().await {
                Ok(Some(task)) => {
                    tracing::debug!("Processing task {}", task.id);

                    // Execute the task
                    let result = self.execute_task(task.clone()).await;

                    match result {
                        Ok(execution_result) => {
                            // Complete the task in scheduler
                            let _ = self.task_scheduler.complete_task(
                                execution_result.task_id,
                                execution_result.success,
                                execution_result.tx_hash.clone(),
                                execution_result.error_message.clone(),
                            ).await;

                            // If execution failed and retries are available, retry
                            if !execution_result.success {
                                let current_task = task.clone();
                                if current_task.retry_count < current_task.max_retries {
                                    tracing::info!(
                                        "Retrying task {} (attempt {})",
                                        current_task.id,
                                        current_task.retry_count + 1
                                    );
                                    let _ = self.task_scheduler.retry_task(current_task.id).await;
                                } else {
                                    tracing::warn!(
                                        "Task {} exceeded max retries",
                                        current_task.id
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to execute task {}: {}", task.id, e);
                            // Mark as failed and retry if possible
                            let _ = self.task_scheduler.complete_task(
                                task.id,
                                false,
                                None,
                                Some(e.to_string()),
                            ).await;

                            if task.retry_count < task.max_retries {
                                let _ = self.task_scheduler.retry_task(task.id).await;
                            }
                        }
                    }
                }
                Ok(None) => {
                    // No tasks available, wait a bit before checking again
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    tracing::error!("Error getting next task: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Get execution statistics
    pub async fn get_execution_stats(&self) -> Result<ExecutionStats> {
        let queue_stats = self.task_scheduler.get_queue_stats().await?;
        let wallet_stats = self.wallet_pool.get_pool_stats().await?;

        Ok(ExecutionStats {
            pending_tasks: queue_stats.pending_tasks,
            processing_tasks: queue_stats.processing_tasks,
            completed_tasks: queue_stats.completed_tasks,
            failed_tasks: queue_stats.failed_tasks,
            available_wallets: wallet_stats.healthy_wallets,
            total_wallets: wallet_stats.total_wallets,
        })
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ExecutionStats {
    pub pending_tasks: usize,
    pub processing_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub available_wallets: usize,
    pub total_wallets: usize,
}

