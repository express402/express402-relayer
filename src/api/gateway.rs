use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::{RelayerError, Result, TransactionRequest, TransactionStatus};

#[derive(Debug, Clone)]
pub struct ApiState {
    // Add your state here - this would include references to your services
    // pub wallet_pool: Arc<WalletPool>,
    // pub task_scheduler: Arc<TaskScheduler>,
    // pub cache_manager: Arc<CacheManager<TransactionResult>>,
    // etc.
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
    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/transactions", post(submit_transaction))
        .route("/transactions/:id", get(get_transaction_status))
        .route("/users/:address/transactions", get(get_user_transactions))
        .route("/transactions/:id/cancel", post(cancel_transaction))
        .with_state(state)
}

async fn health_check(State(state): State<ApiState>) -> Result<Json<HealthCheckResponse>> {
    let mut services = HashMap::new();
    
    // Check wallet pool health
    services.insert("wallet_pool".to_string(), ServiceStatus {
        status: "healthy".to_string(),
        message: None,
    });
    
    // Check queue health
    services.insert("queue".to_string(), ServiceStatus {
        status: "healthy".to_string(),
        message: None,
    });
    
    // Check cache health
    services.insert("cache".to_string(), ServiceStatus {
        status: "healthy".to_string(),
        message: None,
    });
    
    // Check database health
    services.insert("database".to_string(), ServiceStatus {
        status: "healthy".to_string(),
        message: None,
    });

    Ok(Json(HealthCheckResponse {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        services,
    }))
}

async fn get_stats(State(state): State<ApiState>) -> Result<Json<StatsResponse>> {
    // This would gather stats from all services
    // For now, return mock data
    
    Ok(Json(StatsResponse {
        queue_stats: QueueStatsResponse {
            pending_tasks: 0,
            processing_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            available_permits: 10,
        },
        wallet_stats: WalletStatsResponse {
            total_wallets: 5,
            active_wallets: 5,
            healthy_wallets: 5,
            total_transactions: 1000,
            overall_success_rate: 0.95,
        },
        cache_stats: CacheStatsResponse {
            memory_entries: 100,
            redis_connected: true,
            hit_rate: 0.85,
        },
        transaction_stats: TransactionStatsResponse {
            total_transactions: 1000,
            pending_transactions: 10,
            confirmed_transactions: 950,
            failed_transactions: 40,
            average_gas_used: Some(21000.0),
        },
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

    // Parse and validate addresses
    let user_address = payload.user_address.parse::<alloy::primitives::Address>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid user_address format"
            })),
        ))?;

    let target_contract = payload.target_contract.parse::<alloy::primitives::Address>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid target_contract format"
            })),
        ))?;

    // Parse calldata
    let calldata = if payload.calldata.starts_with("0x") {
        hex::decode(&payload.calldata[2..])
            .map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Invalid calldata format"
                })),
            ))?
    } else {
        hex::decode(&payload.calldata)
            .map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Invalid calldata format"
                })),
            ))?
    };

    // Parse numeric values
    let value = payload.value.parse::<alloy::primitives::U256>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid value format"
            })),
        ))?;

    let gas_limit = payload.gas_limit.parse::<alloy::primitives::U256>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid gas_limit format"
            })),
        ))?;

    let max_fee_per_gas = payload.max_fee_per_gas.parse::<alloy::primitives::U256>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid max_fee_per_gas format"
            })),
        ))?;

    let max_priority_fee_per_gas = payload.max_priority_fee_per_gas.parse::<alloy::primitives::U256>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid max_priority_fee_per_gas format"
            })),
        ))?;

    let nonce = payload.nonce.parse::<alloy::primitives::U256>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid nonce format"
            })),
        ))?;

    // Parse signature
    let signature_r = payload.signature_r.parse::<alloy::primitives::U256>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid signature_r format"
            })),
        ))?;

    let signature_s = payload.signature_s.parse::<alloy::primitives::U256>()
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid signature_s format"
            })),
        ))?;

    // Parse priority
    let priority = match payload.priority.as_str() {
        "low" => crate::types::Priority::Low,
        "normal" => crate::types::Priority::Normal,
        "high" => crate::types::Priority::High,
        "critical" => crate::types::Priority::Critical,
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid priority. Must be one of: low, normal, high, critical"
            })),
        )),
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

    // TODO: Submit to task scheduler
    // let task_id = state.task_scheduler.schedule_task(transaction_request).await?;

    // For now, return a mock response
    Ok(Json(SubmitTransactionResponse {
        transaction_id: transaction_request.id,
        status: "pending".to_string(),
        message: "Transaction submitted successfully".to_string(),
    }))
}

async fn get_transaction_status(
    State(state): State<ApiState>,
    Path(transaction_id): Path<Uuid>,
) -> Result<Json<TransactionStatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    // TODO: Get transaction status from persistence layer
    // For now, return a mock response
    
    Ok(Json(TransactionStatusResponse {
        transaction_id,
        status: "pending".to_string(),
        tx_hash: None,
        block_number: None,
        gas_used: None,
        error_message: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }))
}

async fn get_user_transactions(
    State(state): State<ApiState>,
    Path(address): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<UserTransactionsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Parse query parameters
    let page = params.get("page")
        .and_then(|p| p.parse::<u64>().ok())
        .unwrap_or(1);
    
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u64>().ok())
        .unwrap_or(20)
        .min(100); // Cap at 100

    // TODO: Get user transactions from persistence layer
    // For now, return a mock response
    
    Ok(Json(UserTransactionsResponse {
        transactions: vec![],
        total: 0,
        page,
        limit,
    }))
}

async fn cancel_transaction(
    State(state): State<ApiState>,
    Path(transaction_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // TODO: Cancel transaction in task scheduler
    // For now, return a mock response
    
    Ok(Json(serde_json::json!({
        "transaction_id": transaction_id,
        "status": "cancelled",
        "message": "Transaction cancelled successfully"
    })))
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
