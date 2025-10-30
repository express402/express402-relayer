use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::types::{RelayerError, Result, TransactionRequest, TransactionStatus};
use crate::database::DatabaseManager;
use crate::cache::CacheManager;
use crate::wallet::pool::WalletPool;
use crate::queue::scheduler::TaskScheduler;
use crate::security::{SignatureVerifier, ReplayProtection};
use crate::config::Config;
use crate::services::EthereumProvider;
use crate::utils::gas::GasPriceOracle;
use crate::utils::validation::TransactionValidator;

#[derive(Debug, Clone)]
pub struct ApiState {
    pub database_manager: Arc<DatabaseManager>,
    pub cache_manager: Arc<CacheManager<serde_json::Value>>,
    pub ethereum_provider: Arc<EthereumProvider>,
    pub wallet_pool: Arc<WalletPool>,
    pub task_scheduler: Arc<TaskScheduler>,
    pub signature_verifier: Arc<SignatureVerifier>,
    pub replay_protection: Arc<ReplayProtection>,
    pub gas_price_oracle: Option<Arc<GasPriceOracle>>,
    pub config: Arc<Config>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitTransactionRequest {
    pub user_address: String,
    pub target_contract: String,
    pub calldata: String,
    pub value: String,
    pub gas_limit: String,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub nonce: String,
    pub signature_r: String,
    pub signature_s: String,
    pub signature_v: u8,
    pub priority: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitTransactionResponse {
    pub transaction_id: Uuid,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionStatusResponse {
    pub transaction_id: Uuid,
    pub status: String,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub gas_used: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserTransactionsResponse {
    pub transactions: Vec<TransactionStatusResponse>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub timestamp: String,
    pub version: String,
    pub services: HashMap<String, ServiceStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub queue_stats: QueueStatsResponse,
    pub wallet_stats: WalletStatsResponse,
    pub cache_stats: CacheStatsResponse,
    pub transaction_stats: TransactionStatsResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueStatsResponse {
    pub pending_tasks: usize,
    pub processing_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub available_permits: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletStatsResponse {
    pub total_wallets: usize,
    pub active_wallets: usize,
    pub healthy_wallets: usize,
    pub total_transactions: u64,
    pub overall_success_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStatsResponse {
    pub memory_entries: usize,
    pub redis_connected: bool,
    pub hit_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionStatsResponse {
    pub total_transactions: u64,
    pub pending_transactions: u64,
    pub confirmed_transactions: u64,
    pub failed_transactions: u64,
    pub average_gas_used: Option<f64>,
}

pub fn create_router(state: ApiState) -> Router {
    use axum::middleware;
    use std::sync::Arc;
    
    // Create auth manager and rate limiter for middleware
    let auth_manager = Arc::new(crate::api::auth::AuthManager::new(
        state.config.security.signature_timeout.to_string(),
    ));
    let rate_limiter = Arc::new(crate::api::auth::RateLimiter::new(
        crate::api::auth::RateLimit {
            requests_per_minute: state.config.server.rate_limit_per_minute,
            requests_per_hour: state.config.server.rate_limit_per_minute * 60,
            requests_per_day: state.config.server.rate_limit_per_minute * 60 * 24,
        },
    ));
    
    Router::new()
        // Public routes (no authentication required)
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/gas-price", get(get_gas_price))
        .route("/metrics", get(get_metrics))
        // Protected routes (require authentication)
        .route("/transactions", post(submit_transaction))
        .route("/transactions/batch", post(submit_batch_transactions))
        .route("/transactions/:id", get(get_transaction_status))
        .route("/users/:address/transactions", get(get_user_transactions))
        .route("/transactions/:id/cancel", post(cancel_transaction))
        // Admin routes
        .route("/admin/queue", get(get_queue_details))
        .route("/admin/wallets", get(get_wallet_details))
        .route("/admin/config", get(get_config_info))
        // Search routes
        .route("/transactions/search", get(search_transactions))
        // Apply rate limiting middleware to all routes
        .layer(middleware::from_fn_with_state(
            (auth_manager.clone(), rate_limiter),
            rate_limit_middleware,
        ))
        .with_state(state)
}

// Rate limiting middleware (simplified version)
async fn rate_limit_middleware(
    axum::extract::State((_auth_manager, rate_limiter)): axum::extract::State<(Arc<crate::api::auth::AuthManager>, Arc<crate::api::auth::RateLimiter>)>,
    headers: axum::http::HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    // Extract identifier (IP or API key)
    let identifier = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    
    // Check rate limit
    match rate_limiter.check_rate_limit(&identifier).await {
        Ok(true) => next.run(request).await,
        Ok(false) => {
            axum::response::Response::builder()
                .status(axum::http::StatusCode::TOO_MANY_REQUESTS)
                .header("retry-after", "60")
                .body(axum::body::Body::from("Rate limit exceeded"))
                .unwrap()
        }
        Err(_) => {
            axum::response::Response::builder()
                .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(axum::body::Body::from("Internal server error"))
                .unwrap()
        }
    }
}

async fn health_check(
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let mut services = HashMap::new();
    let mut overall_status = "healthy".to_string();

    // Check database health
    match state.database_manager.check_connection().await {
        Ok(_) => {
            services.insert("database".to_string(), ServiceStatus {
                status: "healthy".to_string(),
                message: Some("Database connection successful".to_string()),
            });
        }
        Err(e) => {
            overall_status = "unhealthy".to_string();
            services.insert("database".to_string(), ServiceStatus {
                status: "unhealthy".to_string(),
                message: Some(format!("Database error: {}", e)),
            });
        }
    }

    // Check wallet pool health
    match state.wallet_pool.get_pool_stats().await {
        Ok(stats) => {
            let status = if stats.healthy_wallets > 0 {
                "healthy"
            } else {
                overall_status = "degraded".to_string();
                "degraded"
            };
            services.insert("wallet_pool".to_string(), ServiceStatus {
                status: status.to_string(),
                message: Some(format!("Healthy wallets: {}/{}", stats.healthy_wallets, stats.total_wallets)),
            });
        }
        Err(e) => {
            overall_status = "degraded".to_string();
            services.insert("wallet_pool".to_string(), ServiceStatus {
                status: "unhealthy".to_string(),
                message: Some(format!("Wallet pool error: {}", e)),
            });
        }
    }

    // Check queue health
    match state.task_scheduler.get_queue_stats().await {
        Ok(stats) => {
            services.insert("queue".to_string(), ServiceStatus {
                status: "healthy".to_string(),
                message: Some(format!("Pending: {}, Processing: {}", stats.pending_tasks, stats.processing_tasks)),
            });
        }
        Err(e) => {
            overall_status = "degraded".to_string();
            services.insert("queue".to_string(), ServiceStatus {
                status: "unhealthy".to_string(),
                message: Some(format!("Queue error: {}", e)),
            });
        }
    }

    // Check cache health
    match state.cache_manager.get_stats().await {
        Ok(_) => {
            services.insert("cache".to_string(), ServiceStatus {
                status: "healthy".to_string(),
                message: Some("Cache system operational".to_string()),
            });
        }
        Err(e) => {
            // Cache failure is not critical, so we mark as degraded
            if overall_status == "healthy" {
                overall_status = "degraded".to_string();
            }
            services.insert("cache".to_string(), ServiceStatus {
                status: "unhealthy".to_string(),
                message: Some(format!("Cache error: {}", e)),
            });
        }
    }

    // Check Ethereum provider health
    match state.ethereum_provider.get_chain_id().await {
        Ok(chain_id) => {
            services.insert("ethereum_provider".to_string(), ServiceStatus {
                status: "healthy".to_string(),
                message: Some(format!("Chain ID: {}", chain_id)),
            });
        }
        Err(e) => {
            overall_status = "degraded".to_string();
            services.insert("ethereum_provider".to_string(), ServiceStatus {
                status: "unhealthy".to_string(),
                message: Some(format!("Provider error: {}", e)),
            });
        }
    }

    let status_code = match overall_status.as_str() {
        "healthy" => StatusCode::OK,
        "degraded" => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(HealthCheckResponse {
        status: overall_status,
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        services,
    }))
}

async fn get_stats(
    State(state): State<ApiState>,
) -> Result<Json<StatsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get queue stats
    let queue_stats = state.task_scheduler.get_queue_stats().await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to get queue stats: {}", e)
            }))
        ))?;

    // Get wallet pool stats
    let wallet_stats = state.wallet_pool.get_pool_stats().await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to get wallet stats: {}", e)
            }))
        ))?;

    // Get cache stats
    let cache_stats = state.cache_manager.get_stats().await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to get cache stats: {}", e)
            }))
        ))?;

    // Get transaction stats from database
    let transaction_stats = match state.database_manager.get_transaction_stats().await {
        Ok(db_stats) => TransactionStatsResponse {
            total_transactions: db_stats.total_transactions,
            pending_transactions: db_stats.pending_transactions,
            confirmed_transactions: db_stats.confirmed_transactions,
            failed_transactions: db_stats.failed_transactions,
            average_gas_used: db_stats.avg_gas_used.and_then(|s| s.parse::<f64>().ok()),
        },
        Err(e) => {
            tracing::warn!("Failed to get transaction stats: {}", e);
            TransactionStatsResponse {
                total_transactions: 0,
                pending_transactions: 0,
                confirmed_transactions: 0,
                failed_transactions: 0,
                average_gas_used: None,
            }
        }
    };

    Ok(Json(StatsResponse {
        queue_stats: QueueStatsResponse {
            pending_tasks: queue_stats.pending_tasks,
            processing_tasks: queue_stats.processing_tasks,
            completed_tasks: queue_stats.completed_tasks,
            failed_tasks: queue_stats.failed_tasks,
            available_permits: queue_stats.available_permits,
        },
        wallet_stats: WalletStatsResponse {
            total_wallets: wallet_stats.total_wallets,
            active_wallets: wallet_stats.active_wallets,
            healthy_wallets: wallet_stats.healthy_wallets,
            total_transactions: wallet_stats.total_transactions,
            overall_success_rate: wallet_stats.overall_success_rate,
        },
        cache_stats: CacheStatsResponse {
            memory_entries: cache_stats.memory_stats.entry_count,
            redis_connected: cache_stats.use_redis,
            hit_rate: cache_stats.memory_stats.hit_rate,
        },
        transaction_stats,
    }))
}

// Detailed metrics endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub system: SystemMetrics,
    pub queue: QueueMetrics,
    pub wallet: WalletMetrics,
    pub gas_price: Option<GasPriceMetrics>,
    pub transaction_tracker: Option<TransactionTrackerMetrics>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub uptime_seconds: u64,
    pub memory_usage_mb: Option<f64>,
    pub cpu_usage_percent: Option<f64>,
    pub active_connections: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueMetrics {
    pub pending_tasks: usize,
    pub processing_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub available_permits: usize,
    pub max_queue_size: usize,
    pub queue_utilization_percent: f64,
    pub average_processing_time_seconds: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletMetrics {
    pub total_wallets: usize,
    pub active_wallets: usize,
    pub healthy_wallets: usize,
    pub unhealthy_wallets: usize,
    pub total_transactions: u64,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub overall_success_rate: f64,
    pub average_balance_wei: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GasPriceMetrics {
    pub current_max_fee_gwei: String,
    pub current_priority_fee_gwei: String,
    pub base_fee_gwei: String,
    pub trend: String,
    pub min_max_fee_gwei: String,
    pub max_max_fee_gwei: String,
    pub average_max_fee_gwei: String,
    pub last_updated: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionTrackerMetrics {
    pub pending_transactions: usize,
    pub oldest_pending_seconds: Option<u64>,
    pub check_interval_seconds: u64,
    pub confirmation_blocks: u64,
}

async fn get_metrics(
    State(state): State<ApiState>,
) -> Result<Json<MetricsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get queue stats
    let queue_stats = state.task_scheduler.get_queue_stats().await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to get queue stats: {}", e)
            }))
        ))?;

    // Get wallet pool stats
    let wallet_stats = state.wallet_pool.get_pool_stats().await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to get wallet stats: {}", e)
            }))
        ))?;

    // Get gas price metrics
    let gas_price_metrics = if let Some(ref oracle) = state.gas_price_oracle {
        match oracle.get_statistics().await {
            Ok(stats) => {
                Some(GasPriceMetrics {
                    current_max_fee_gwei: (stats.current_max_fee.to::<u64>() as f64 / 1_000_000_000.0).to_string(),
                    current_priority_fee_gwei: (stats.current_priority_fee.to::<u64>() as f64 / 1_000_000_000.0).to_string(),
                    base_fee_gwei: (stats.current_max_fee.to::<u64>() as f64 / 1_000_000_000.0).to_string(),
                    trend: stats.trend,
                    min_max_fee_gwei: (stats.min_max_fee.to::<u64>() as f64 / 1_000_000_000.0).to_string(),
                    max_max_fee_gwei: (stats.max_max_fee.to::<u64>() as f64 / 1_000_000_000.0).to_string(),
                    average_max_fee_gwei: (stats.average_max_fee.to::<u64>() as f64 / 1_000_000_000.0).to_string(),
                    last_updated: stats.last_updated.to_rfc3339(),
                })
            }
            Err(e) => {
                tracing::warn!("Failed to get gas price statistics: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Get transaction tracker metrics
    let tracker_metrics = if let Some(ref tracker) = state.transaction_tracker {
        match tracker.get_tracking_stats().await {
            Ok(stats) => {
                Some(TransactionTrackerMetrics {
                    pending_transactions: stats.total_pending,
                    oldest_pending_seconds: stats.oldest_pending_seconds,
                    check_interval_seconds: stats.check_interval_seconds,
                    confirmation_blocks: stats.confirmation_blocks,
                })
            }
            Err(e) => {
                tracing::warn!("Failed to get tracker stats: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Calculate queue utilization
    let queue_utilization = if queue_stats.max_queue_size > 0 {
        (queue_stats.pending_tasks + queue_stats.processing_tasks) as f64 / queue_stats.max_queue_size as f64 * 100.0
    } else {
        0.0
    };

    // Get transaction stats for wallet metrics
    let db_stats = state.database_manager.get_transaction_stats().await.ok();
    let successful_txs = db_stats.as_ref().map(|s| s.confirmed_transactions).unwrap_or(0);
    let failed_txs = db_stats.as_ref().map(|s| s.failed_transactions).unwrap_or(0);

    Ok(Json(MetricsResponse {
        system: SystemMetrics {
            uptime_seconds: 0, // Would need to track startup time
            memory_usage_mb: None, // Would need system monitoring
            cpu_usage_percent: None, // Would need system monitoring
            active_connections: 0, // Would need connection tracking
        },
        queue: QueueMetrics {
            pending_tasks: queue_stats.pending_tasks,
            processing_tasks: queue_stats.processing_tasks,
            completed_tasks: queue_stats.completed_tasks,
            failed_tasks: queue_stats.failed_tasks,
            available_permits: queue_stats.available_permits,
            max_queue_size: queue_stats.max_queue_size,
            queue_utilization_percent: queue_utilization,
            average_processing_time_seconds: None, // Would need to track processing times
        },
        wallet: WalletMetrics {
            total_wallets: wallet_stats.total_wallets,
            active_wallets: wallet_stats.active_wallets,
            healthy_wallets: wallet_stats.healthy_wallets,
            unhealthy_wallets: wallet_stats.total_wallets.saturating_sub(wallet_stats.healthy_wallets),
            total_transactions: wallet_stats.total_transactions,
            successful_transactions: successful_txs,
            failed_transactions: failed_txs,
            overall_success_rate: wallet_stats.overall_success_rate,
            average_balance_wei: None, // Would need to fetch wallet balances
        },
        gas_price: gas_price_metrics,
        transaction_tracker: tracker_metrics,
    }))
}

async fn submit_transaction(
    State(state): State<ApiState>,
    Json(payload): Json<SubmitTransactionRequest>,
) -> Result<Json<SubmitTransactionResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Validate the request
    if payload.user_address.is_empty() || payload.target_contract.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid request: user_address and target_contract are required"
            })),
        ));
    }

    // Use TransactionValidator for comprehensive validation
    let user_address = TransactionValidator::validate_address_string(&payload.user_address)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_ADDRESS"
            })),
        ))?;

    let target_contract = TransactionValidator::validate_address_string(&payload.target_contract)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_ADDRESS"
            })),
        ))?;

    // Validate addresses are not zero
    TransactionValidator::validate_address(&user_address)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_ADDRESS"
            })),
        ))?;

    TransactionValidator::validate_address(&target_contract)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_ADDRESS"
            })),
        ))?;

    // Parse and validate calldata
    let calldata_bytes = TransactionValidator::validate_calldata_string(&payload.calldata)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_CALLDATA"
            })),
        ))?;
    let calldata = alloy::primitives::Bytes::from(calldata_bytes);

    // Parse numeric values using validator
    let value = TransactionValidator::parse_u256(&payload.value)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_VALUE"
            })),
        ))?;
    TransactionValidator::validate_value(&value)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_VALUE"
            })),
        ))?;

    let gas_limit = TransactionValidator::parse_u256(&payload.gas_limit)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_GAS_LIMIT"
            })),
        ))?;
    TransactionValidator::validate_gas_limit(&gas_limit)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_GAS_LIMIT"
            })),
        ))?;

    let max_fee_per_gas = TransactionValidator::parse_u256(&payload.max_fee_per_gas)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_GAS_PRICE"
            })),
        ))?;

    let max_priority_fee_per_gas = TransactionValidator::parse_u256(&payload.max_priority_fee_per_gas)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_GAS_PRICE"
            })),
        ))?;

    // Validate gas prices
    TransactionValidator::validate_gas_prices(&max_fee_per_gas, &max_priority_fee_per_gas)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_GAS_PRICE"
            })),
        ))?;

    let nonce = TransactionValidator::parse_u256(&payload.nonce)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_NONCE"
            })),
        ))?;
    TransactionValidator::validate_nonce(&nonce)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_NONCE"
            })),
        ))?;

    // Parse signature
    let signature_r = TransactionValidator::parse_u256(&payload.signature_r)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_SIGNATURE"
            })),
        ))?;

    let signature_s = TransactionValidator::parse_u256(&payload.signature_s)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_SIGNATURE"
            })),
        ))?;

    // Parse and validate priority
    let priority = TransactionValidator::validate_priority(&payload.priority)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_PRIORITY"
            })),
        ))?;

    // Validate signature structure
    let signature = crate::types::Signature {
        r: signature_r,
        s: signature_s,
        v: payload.signature_v,
    };
    TransactionValidator::validate_signature(&signature)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string(),
                "code": "INVALID_SIGNATURE"
            })),
        ))?;

    // Create transaction request (all validations already done)
    let transaction_request = TransactionRequest::new(
        user_address,
        target_contract,
        calldata,
        value,
        gas_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        nonce,
        signature,
        priority,
    );

    // Final comprehensive validation
    TransactionValidator::validate_transaction_params(
        &transaction_request.user_address,
        &transaction_request.target_contract,
        &transaction_request.calldata,
        &transaction_request.value,
        &transaction_request.gas_limit,
        &transaction_request.max_fee_per_gas,
        &transaction_request.max_priority_fee_per_gas,
        &transaction_request.nonce,
        &transaction_request.signature,
    ).map_err(|e| (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "error": e.to_string(),
            "code": "VALIDATION_FAILED"
        })),
    ))?;

    // Verify transaction signature
    tracing::debug!("Verifying transaction signature for address: {:?}", user_address);
    let nonce_u64 = nonce.to::<u64>();
    
    // Clone signature verifier for mutable access
    let mut verifier = (*state.signature_verifier).clone();
    
    match verifier.verify_transaction_signature(&transaction_request, nonce_u64) {
        Ok(true) => {
            tracing::info!("Signature verified successfully for transaction {}", transaction_request.id);
        }
        Ok(false) => {
            tracing::warn!("Signature verification failed for transaction {}", transaction_request.id);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Signature verification failed",
                    "code": "INVALID_SIGNATURE"
                })),
            ));
        }
        Err(e) => {
            tracing::error!("Signature verification error: {}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Signature verification error: {}", e),
                    "code": "SIGNATURE_ERROR"
                })),
            ));
        }
    }

    // Check replay protection
    if state.config.security.enable_replay_protection {
        tracing::debug!("Checking replay protection for address: {:?}, nonce: {}", user_address, nonce_u64);
        match state.replay_protection.check_and_record(user_address, nonce_u64, None) {
            Ok(_) => {
                tracing::debug!("Replay protection check passed");
            }
            Err(e) => {
                tracing::warn!("Replay attack detected: {}", e);
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("Replay attack detected: {}", e),
                        "code": "REPLAY_ATTACK"
                    })),
                ));
            }
        }
    }

    // Gas prices are already validated by TransactionValidator

    // Check wallet pool availability
    let wallet_stats = state.wallet_pool.get_pool_stats().await
        .map_err(|e| (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": format!("Failed to check wallet pool: {}", e),
                "code": "WALLET_POOL_ERROR"
            })),
        ))?;

    if wallet_stats.healthy_wallets == 0 {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "No healthy wallets available",
                "code": "WALLET_UNAVAILABLE"
            })),
        ));
    }

    // Check queue capacity
    let queue_stats = state.task_scheduler.get_queue_stats().await
        .map_err(|e| (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": format!("Failed to check queue: {}", e),
                "code": "QUEUE_ERROR"
            })),
        ))?;

    if queue_stats.available_permits == 0 {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Transaction queue is full",
                "code": "QUEUE_FULL"
            })),
        ));
    }

    // Store transaction in database
    tracing::info!("Storing transaction {} in database", transaction_request.id);
    state.database_manager.create_transaction(&transaction_request).await
        .map_err(|e| {
            tracing::error!("Failed to store transaction in database: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to store transaction: {}", e),
                    "code": "DATABASE_ERROR"
                })),
            )
        })?;

    // Submit to task scheduler
    tracing::info!("Submitting transaction {} to task scheduler", transaction_request.id);
    let task_id = state.task_scheduler.schedule_task(transaction_request).await
        .map_err(|e| {
            tracing::error!("Failed to schedule task: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": format!("Failed to schedule transaction: {}", e),
                    "code": "SCHEDULER_ERROR"
                })),
            )
        })?;

    tracing::info!("Transaction {} submitted successfully with task ID {}", transaction_request.id, task_id);

    Ok(Json(SubmitTransactionResponse {
        transaction_id: task_id,
        status: "pending".to_string(),
        message: "Transaction submitted successfully".to_string(),
    }))
}

async fn get_transaction_status(
    State(state): State<ApiState>,
    Path(transaction_id): Path<Uuid>,
) -> Result<Json<TransactionStatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get transaction from database
    let transaction = state.database_manager.get_transaction(transaction_id).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        ))?;

    match transaction {
        Some(tx) => {
            Ok(Json(TransactionStatusResponse {
                transaction_id: tx.id,
                status: tx.status,
                tx_hash: tx.tx_hash,
                block_number: tx.block_number.map(|n| n as u64),
                gas_used: tx.gas_used,
                error_message: tx.error_message,
                created_at: tx.created_at.to_rfc3339(),
                updated_at: tx.updated_at.to_rfc3339(),
            }))
        }
        None => {
            Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Transaction not found"
                }))
            ))
        }
    }
}

async fn get_user_transactions(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<UserTransactionsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Validate address format
    let _user_address = address.parse::<alloy::primitives::Address>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid address format",
                "code": "INVALID_ADDRESS"
            })),
        ))?;

    // Parse query parameters
    let page = params.get("page")
        .and_then(|p| p.parse::<u64>().ok())
        .unwrap_or(1);
    
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u64>().ok())
        .unwrap_or(20)
        .min(100); // Cap at 100

    // Get user transactions from database
    let (transactions, total) = state.database_manager.get_user_transactions(&address, page, limit).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        ))?;

    // Convert to response format
    let transaction_responses: Vec<TransactionStatusResponse> = transactions.into_iter().map(|tx| {
        TransactionStatusResponse {
            transaction_id: tx.id,
            status: tx.status,
            tx_hash: tx.tx_hash,
            block_number: tx.block_number.map(|n| n as u64),
            gas_used: tx.gas_used,
            error_message: tx.error_message,
            created_at: tx.created_at.to_rfc3339(),
            updated_at: tx.updated_at.to_rfc3339(),
        }
    }).collect();
    
    Ok(Json(UserTransactionsResponse {
        transactions: transaction_responses,
        total,
        page,
        limit,
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchTransactionsResponse {
    pub transactions: Vec<TransactionStatusResponse>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
}

async fn search_transactions(
    State(state): State<ApiState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<SearchTransactionsResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::database::TransactionFilters;

    // Parse query parameters
    let page = params.get("page")
        .and_then(|p| p.parse::<u64>().ok())
        .unwrap_or(1);
    
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u64>().ok())
        .unwrap_or(20)
        .min(100); // Cap at 100

    // Build filters from query parameters
    let mut filters = TransactionFilters::new();

    if let Some(status) = params.get("status") {
        filters = filters.with_status(status.clone());
    }

    if let Some(priority) = params.get("priority") {
        filters = filters.with_priority(priority.clone());
    }

    if let Some(user_address) = params.get("user_address") {
        // Validate address
        TransactionValidator::validate_address_string(user_address)
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": e.to_string(),
                    "code": "INVALID_ADDRESS"
                })),
            ))?;
        filters = filters.with_user_address(user_address.clone());
    }

    if let Some(target_contract) = params.get("target_contract") {
        // Validate address
        TransactionValidator::validate_address_string(target_contract)
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": e.to_string(),
                    "code": "INVALID_ADDRESS"
                })),
            ))?;
        filters = filters.with_target_contract(target_contract.clone());
    }

    // Parse time range if provided
    if let (Some(start_str), Some(end_str)) = (params.get("start_time"), params.get("end_time")) {
        use crate::utils::time::TimeUtils;
        let start = TimeUtils::parse_rfc3339(start_str)
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid start_time format: {}", e),
                    "code": "INVALID_TIME"
                })),
            ))?;
        let end = TimeUtils::parse_rfc3339(end_str)
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid end_time format: {}", e),
                    "code": "INVALID_TIME"
                })),
            ))?;
        filters = filters.with_time_range(start, end);
    }

    // Search transactions
    let (transactions, total) = state.database_manager.search_transactions(&filters, page, limit).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        ))?;

    // Convert to response format
    let transaction_responses: Vec<TransactionStatusResponse> = transactions.into_iter().map(|tx| {
        TransactionStatusResponse {
            transaction_id: tx.id,
            status: tx.status,
            tx_hash: tx.tx_hash,
            block_number: tx.block_number.map(|n| n as u64),
            gas_used: tx.gas_used,
            error_message: tx.error_message,
            created_at: tx.created_at.to_rfc3339(),
            updated_at: tx.updated_at.to_rfc3339(),
        }
    }).collect();

    Ok(Json(SearchTransactionsResponse {
        transactions: transaction_responses,
        total,
        page,
        limit,
    }))
}

async fn cancel_transaction(
    State(state): State<ApiState>,
    Path(transaction_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Cancel transaction in task scheduler
    let cancelled = state.task_scheduler.cancel_task(transaction_id).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to cancel transaction: {}", e)
            }))
        ))?;

    if cancelled {
        // Update transaction status in database
        state.database_manager.update_transaction_status(
            transaction_id,
            TransactionStatus::Cancelled,
            None,
            None,
            None,
            Some("Cancelled by user".to_string()),
        ).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to update transaction status: {}", e)
            }))
        ))?;

        Ok(Json(serde_json::json!({
            "transaction_id": transaction_id,
            "status": "cancelled",
            "message": "Transaction cancelled successfully"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Transaction not found or cannot be cancelled"
            }))
        ))
    }
}

// Gas price endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct GasPriceResponse {
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub base_fee: String,
    pub timestamp: String,
    pub block_number: u64,
    pub trend: Option<String>,
}

async fn get_gas_price(
    State(state): State<ApiState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GasPriceResponse>, (StatusCode, Json<serde_json::Value>)> {
    let priority = params.get("priority").map(|s| s.as_str()).unwrap_or("normal");

    if let Some(ref oracle) = state.gas_price_oracle {
        match oracle.get_recommended_gas_price(priority).await {
            Ok(gas_info) => {
                Ok(Json(GasPriceResponse {
                    max_fee_per_gas: gas_info.max_fee_per_gas.to_string(),
                    max_priority_fee_per_gas: gas_info.max_priority_fee_per_gas.to_string(),
                    base_fee: gas_info.base_fee.to_string(),
                    timestamp: gas_info.timestamp.to_rfc3339(),
                    block_number: gas_info.block_number,
                    trend: None,
                }))
            }
            Err(e) => {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to get gas price: {}", e)
                    }))
                ))
            }
        }
    } else {
        // Fallback: get current gas price from provider
        match state.ethereum_provider.get_gas_price().await {
            Ok(gas_price) => {
                let max_fee = gas_price * alloy::primitives::U256::from(110) / alloy::primitives::U256::from(100);
                let priority_fee = gas_price * alloy::primitives::U256::from(10) / alloy::primitives::U256::from(100);
                
                Ok(Json(GasPriceResponse {
                    max_fee_per_gas: max_fee.to_string(),
                    max_priority_fee_per_gas: priority_fee.to_string(),
                    base_fee: gas_price.to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    block_number: 0,
                    trend: None,
                }))
            }
            Err(e) => {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to get gas price: {}", e)
                    }))
                ))
            }
        }
    }
}

// Batch transaction endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchTransactionRequest {
    pub transactions: Vec<SubmitTransactionRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchTransactionResponse {
    pub batch_id: Uuid,
    pub transaction_ids: Vec<Uuid>,
    pub status: String,
    pub message: String,
}

async fn submit_batch_transactions(
    State(state): State<ApiState>,
    Json(payload): Json<BatchTransactionRequest>,
) -> Result<Json<BatchTransactionResponse>, (StatusCode, Json<serde_json::Value>)> {
    if payload.transactions.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Batch must contain at least one transaction"
            })),
        ));
    }

    if payload.transactions.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Batch size exceeds maximum limit of 100 transactions"
            })),
        ));
    }

    let batch_id = Uuid::new_v4();
    let mut transaction_ids = Vec::new();
    let mut errors = Vec::new();

    // Process each transaction in the batch
    for (index, tx_request) in payload.transactions.iter().enumerate() {
        match process_single_transaction(&state, tx_request).await {
            Ok(tx_id) => {
                transaction_ids.push(tx_id);
            }
            Err(e) => {
                errors.push(format!("Transaction {}: {}", index, e));
            }
        }
    }

    let status = if errors.is_empty() {
        "success"
    } else if transaction_ids.is_empty() {
        "failed"
    } else {
        "partial"
    };

    let message = if errors.is_empty() {
        format!("Successfully submitted {} transactions", transaction_ids.len())
    } else {
        format!("Submitted {} transactions, {} failed: {}", 
                transaction_ids.len(), 
                errors.len(),
                errors.join("; "))
    };

    Ok(Json(BatchTransactionResponse {
        batch_id,
        transaction_ids,
        status: status.to_string(),
        message,
    }))
}

// Helper function to process a single transaction
async fn process_single_transaction(
    state: &ApiState,
    payload: &SubmitTransactionRequest,
) -> Result<Uuid, String> {
    // Parse and validate addresses
    let user_address = payload.user_address.parse::<alloy::primitives::Address>()
        .map_err(|_| "Invalid user_address format".to_string())?;

    let target_contract = payload.target_contract.parse::<alloy::primitives::Address>()
        .map_err(|_| "Invalid target_contract format".to_string())?;

    // Parse calldata
    let calldata = if payload.calldata.starts_with("0x") {
        hex::decode(&payload.calldata[2..])
            .map_err(|_| "Invalid calldata format".to_string())?
    } else {
        hex::decode(&payload.calldata)
            .map_err(|_| "Invalid calldata format".to_string())?
    };

    // Parse numeric values
    let value = payload.value.parse::<alloy::primitives::U256>()
        .map_err(|_| "Invalid value format".to_string())?;

    let gas_limit = payload.gas_limit.parse::<alloy::primitives::U256>()
        .map_err(|_| "Invalid gas_limit format".to_string())?;

    let max_fee_per_gas = payload.max_fee_per_gas.parse::<alloy::primitives::U256>()
        .map_err(|_| "Invalid max_fee_per_gas format".to_string())?;

    let max_priority_fee_per_gas = payload.max_priority_fee_per_gas.parse::<alloy::primitives::U256>()
        .map_err(|_| "Invalid max_priority_fee_per_gas format".to_string())?;

    let nonce = payload.nonce.parse::<alloy::primitives::U256>()
        .map_err(|_| "Invalid nonce format".to_string())?;

    let signature_r = payload.signature_r.parse::<alloy::primitives::U256>()
        .map_err(|_| "Invalid signature_r format".to_string())?;

    let signature_s = payload.signature_s.parse::<alloy::primitives::U256>()
        .map_err(|_| "Invalid signature_s format".to_string())?;

    let priority = match payload.priority.as_str() {
        "low" => crate::types::Priority::Low,
        "normal" => crate::types::Priority::Normal,
        "high" => crate::types::Priority::High,
        "critical" => crate::types::Priority::Critical,
        _ => return Err("Invalid priority".to_string()),
    };

    // Create transaction request
    let transaction_request = TransactionRequest::new(
        user_address,
        target_contract,
        alloy::primitives::Bytes::from(calldata),
        value,
        gas_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        nonce,
        crate::types::Signature {
            r: signature_r,
            s: signature_s,
            v: payload.signature_v,
        },
        priority,
    );

    // Store in database
    state.database_manager.create_transaction(&transaction_request).await
        .map_err(|e| format!("Database error: {}", e))?;

    // Submit to task scheduler
    let task_id = state.task_scheduler.schedule_task(transaction_request).await
        .map_err(|e| format!("Scheduler error: {}", e))?;

    Ok(task_id)
}

// Admin endpoints

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueDetailsResponse {
    pub stats: QueueStatsResponse,
    pub pending_tasks: Vec<PendingTaskInfo>,
    pub processing_tasks: Vec<ProcessingTaskInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingTaskInfo {
    pub task_id: Uuid,
    pub priority: String,
    pub created_at: String,
    pub age_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessingTaskInfo {
    pub task_id: Uuid,
    pub priority: String,
    pub started_at: String,
    pub processing_time_seconds: u64,
}

async fn get_queue_details(
    State(state): State<ApiState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<QueueDetailsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let _limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50)
        .min(100);

    let queue_stats = state.task_scheduler.get_queue_stats().await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to get queue stats: {}", e)
            }))
        ))?;

    // For simplicity, we'll return empty lists for pending/processing tasks
    // In a full implementation, you'd fetch actual task details from the scheduler
    Ok(Json(QueueDetailsResponse {
        stats: QueueStatsResponse {
            pending_tasks: queue_stats.pending_tasks,
            processing_tasks: queue_stats.processing_tasks,
            completed_tasks: queue_stats.completed_tasks,
            failed_tasks: queue_stats.failed_tasks,
            available_permits: queue_stats.available_permits,
        },
        pending_tasks: Vec::new(), // Would be populated from scheduler
        processing_tasks: Vec::new(), // Would be populated from scheduler
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletDetailsResponse {
    pub stats: WalletStatsResponse,
    pub wallets: Vec<WalletDetailInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletDetailInfo {
    pub address: String,
    pub balance_wei: Option<String>,
    pub is_active: bool,
    pub is_healthy: bool,
    pub total_transactions: u64,
    pub success_rate: f64,
    pub last_used: Option<String>,
}

async fn get_wallet_details(
    State(state): State<ApiState>,
) -> Result<Json<WalletDetailsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let wallet_stats = state.wallet_pool.get_pool_stats().await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to get wallet stats: {}", e)
            }))
        ))?;

    // Get wallet pool info - this would need to be implemented in WalletPool
    // For now, return empty wallet list
    Ok(Json(WalletDetailsResponse {
        stats: WalletStatsResponse {
            total_wallets: wallet_stats.total_wallets,
            active_wallets: wallet_stats.active_wallets,
            healthy_wallets: wallet_stats.healthy_wallets,
            total_transactions: wallet_stats.total_transactions,
            overall_success_rate: wallet_stats.overall_success_rate,
        },
        wallets: Vec::new(), // Would be populated from wallet pool
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigInfoResponse {
    pub server: ServerConfigInfo,
    pub database: DatabaseConfigInfo,
    pub ethereum: EthereumConfigInfo,
    pub queue: QueueConfigInfo,
    pub security: SecurityConfigInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfigInfo {
    pub host: String,
    pub port: u16,
    pub max_connections: u32,
    pub request_timeout: u64,
    pub rate_limit_per_minute: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseConfigInfo {
    pub url_hidden: String, // URL with password hidden
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EthereumConfigInfo {
    pub rpc_url_hidden: String, // URL with API key hidden
    pub chain_id: u64,
    pub gas_price_multiplier: f64,
    pub confirmation_blocks: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueConfigInfo {
    pub max_queue_size: usize,
    pub worker_threads: usize,
    pub batch_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityConfigInfo {
    pub signature_timeout: u64,
    pub nonce_window: u64,
    pub enable_replay_protection: bool,
}

async fn get_config_info(
    State(state): State<ApiState>,
) -> Result<Json<ConfigInfoResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Hide sensitive information
    let db_url = hide_sensitive_info(&state.config.database.url);
    let eth_url = hide_sensitive_info(&state.config.ethereum.rpc_url);

    Ok(Json(ConfigInfoResponse {
        server: ServerConfigInfo {
            host: state.config.server.host.clone(),
            port: state.config.server.port,
            max_connections: state.config.server.max_connections,
            request_timeout: state.config.server.request_timeout,
            rate_limit_per_minute: state.config.server.rate_limit_per_minute,
        },
        database: DatabaseConfigInfo {
            url_hidden: db_url,
            max_connections: state.config.database.max_connections,
            min_connections: state.config.database.min_connections,
        },
        ethereum: EthereumConfigInfo {
            rpc_url_hidden: eth_url,
            chain_id: state.config.ethereum.chain_id,
            gas_price_multiplier: state.config.ethereum.gas_price_multiplier,
            confirmation_blocks: state.config.ethereum.confirmation_blocks,
        },
        queue: QueueConfigInfo {
            max_queue_size: state.config.queue.max_queue_size,
            worker_threads: state.config.queue.worker_threads,
            batch_size: state.config.queue.batch_size,
        },
        security: SecurityConfigInfo {
            signature_timeout: state.config.security.signature_timeout,
            nonce_window: state.config.security.nonce_window,
            enable_replay_protection: state.config.security.enable_replay_protection,
        },
    }))
}

fn hide_sensitive_info(url: &str) -> String {
    // Simple hiding of passwords/API keys in URLs
    if let Some(at_pos) = url.find('@') {
        if let Some(before_at) = url.get(..at_pos) {
            if let Some(colon_pos) = before_at.rfind(':') {
                let prefix = &url[..colon_pos + 1];
                let after_at = &url[at_pos..];
                return format!("{}***{}", prefix, after_at);
            }
        }
    }
    // For API keys in query params
    if url.contains("api-key") || url.contains("apikey") {
        return url.split('&').map(|part| {
            if part.contains("key") || part.contains("Key") {
                let eq_pos = part.find('=').unwrap_or(part.len());
                format!("{}***", &part[..=eq_pos])
            } else {
                part.to_string()
            }
        }).collect::<Vec<_>>().join("&");
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let state = ApiState {};
        let app = create_router(state);
        
        let request = Request::builder()
            .uri("/health")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_submit_transaction_invalid_address() {
        let state = ApiState {};
        let app = create_router(state);
        
        let payload = SubmitTransactionRequest {
            user_address: "invalid_address".to_string(),
            target_contract: "0x1234567890123456789012345678901234567890".to_string(),
            calldata: "0x".to_string(),
            value: "0".to_string(),
            gas_limit: "21000".to_string(),
            max_fee_per_gas: "20000000000".to_string(),
            max_priority_fee_per_gas: "2000000000".to_string(),
            nonce: "0".to_string(),
            signature_r: "0".to_string(),
            signature_s: "0".to_string(),
            signature_v: 27,
            priority: "normal".to_string(),
        };
        
        let request = Request::builder()
            .uri("/transactions")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&payload).unwrap()))
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_transaction_status() {
        let state = ApiState {};
        let app = create_router(state);
        
        let transaction_id = Uuid::new_v4();
        let request = Request::builder()
            .uri(&format!("/transactions/{}", transaction_id))
            .method("GET")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
