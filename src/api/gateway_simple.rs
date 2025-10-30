use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

use crate::types::{Result, TransactionRequest, TransactionStatus, Priority};

#[derive(Debug, Clone)]
pub struct ApiState {
    // Metrics tracking
    pub total_requests: Arc<AtomicU64>,
    pub successful_requests: Arc<AtomicU64>,
    pub failed_requests: Arc<AtomicU64>,
    pub total_transactions: Arc<AtomicU64>,
    pub pending_transactions: Arc<AtomicU64>,
    pub completed_transactions: Arc<AtomicU64>,
    pub failed_transactions: Arc<AtomicU64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub timestamp: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub message: String,
    pub metrics: SystemMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub total_transactions: u64,
    pub pending_transactions: u64,
    pub completed_transactions: u64,
    pub failed_transactions: u64,
    pub uptime_seconds: u64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionSubmitRequest {
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
pub struct TransactionSubmitResponse {
    pub transaction_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionStatusResponse {
    pub transaction_id: String,
    pub status: String,
    pub message: String,
    pub details: Option<String>,
}

pub fn create_router() -> Router {
    let state = ApiState {
        total_requests: Arc::new(AtomicU64::new(0)),
        successful_requests: Arc::new(AtomicU64::new(0)),
        failed_requests: Arc::new(AtomicU64::new(0)),
        total_transactions: Arc::new(AtomicU64::new(0)),
        pending_transactions: Arc::new(AtomicU64::new(0)),
        completed_transactions: Arc::new(AtomicU64::new(0)),
        failed_transactions: Arc::new(AtomicU64::new(0)),
    };

    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/metrics", get(get_metrics))
        .route("/transactions", post(submit_transaction))
        .route("/transactions/:id", get(get_transaction_status))
        .route("/transactions/:id/cancel", post(cancel_transaction))
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(HealthCheckResponse {
            status: "healthy".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    )
}

async fn get_stats(State(state): State<ApiState>) -> impl IntoResponse {
    let total_requests = state.total_requests.load(Ordering::Relaxed);
    let successful_requests = state.successful_requests.load(Ordering::Relaxed);
    let failed_requests = state.failed_requests.load(Ordering::Relaxed);
    let success_rate = if total_requests > 0 {
        successful_requests as f64 / total_requests as f64
    } else {
        0.0
    };

    let metrics = SystemMetrics {
        total_requests,
        successful_requests,
        failed_requests,
        success_rate,
        total_transactions: state.total_transactions.load(Ordering::Relaxed),
        pending_transactions: state.pending_transactions.load(Ordering::Relaxed),
        completed_transactions: state.completed_transactions.load(Ordering::Relaxed),
        failed_transactions: state.failed_transactions.load(Ordering::Relaxed),
        uptime_seconds: 0, // TODO: Implement actual uptime tracking
        memory_usage_mb: 0.0, // TODO: Implement actual memory usage tracking
        cpu_usage_percent: 0.0, // TODO: Implement actual CPU usage tracking
    };

    (
        StatusCode::OK,
        Json(StatsResponse {
            message: "System statistics retrieved successfully".to_string(),
            metrics,
        }),
    )
}

async fn get_metrics(State(state): State<ApiState>) -> impl IntoResponse {
    // Prometheus format metrics
    let total_requests = state.total_requests.load(Ordering::Relaxed);
    let successful_requests = state.successful_requests.load(Ordering::Relaxed);
    let failed_requests = state.failed_requests.load(Ordering::Relaxed);
    let total_transactions = state.total_transactions.load(Ordering::Relaxed);
    let pending_transactions = state.pending_transactions.load(Ordering::Relaxed);
    let completed_transactions = state.completed_transactions.load(Ordering::Relaxed);
    let failed_transactions = state.failed_transactions.load(Ordering::Relaxed);

    let metrics = format!(
        "# HELP express402_total_requests Total number of API requests
# TYPE express402_total_requests counter
express402_total_requests {{}} {total_requests}

# HELP express402_successful_requests Total number of successful API requests
# TYPE express402_successful_requests counter
express402_successful_requests {{}} {successful_requests}

# HELP express402_failed_requests Total number of failed API requests
# TYPE express402_failed_requests counter
express402_failed_requests {{}} {failed_requests}

# HELP express402_total_transactions Total number of transactions processed
# TYPE express402_total_transactions counter
express402_total_transactions {{}} {total_transactions}

# HELP express402_pending_transactions Current number of pending transactions
# TYPE express402_pending_transactions gauge
express402_pending_transactions {{}} {pending_transactions}

# HELP express402_completed_transactions Total number of completed transactions
# TYPE express402_completed_transactions counter
express402_completed_transactions {{}} {completed_transactions}

# HELP express402_failed_transactions Total number of failed transactions
# TYPE express402_failed_transactions counter
express402_failed_transactions {{}} {failed_transactions}
"
    );

    (
        StatusCode::OK,
        ([(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")], metrics),
    )
}

async fn submit_transaction(
    State(state): State<ApiState>,
    Json(payload): Json<TransactionSubmitRequest>,
) -> impl IntoResponse {
    // Increment request counters
    state.total_requests.fetch_add(1, Ordering::Relaxed);
    
    // TODO: Implement actual transaction submission
    // For now, return a placeholder response
    let transaction_id = Uuid::new_v4().to_string();
    
    // Increment transaction counters
    state.total_transactions.fetch_add(1, Ordering::Relaxed);
    state.pending_transactions.fetch_add(1, Ordering::Relaxed);
    state.successful_requests.fetch_add(1, Ordering::Relaxed);
    
    (
        StatusCode::ACCEPTED,
        Json(TransactionSubmitResponse {
            transaction_id,
            status: "pending".to_string(),
            message: "Transaction submitted successfully".to_string(),
        }),
    )
}

async fn get_transaction_status(
    State(state): State<ApiState>,
    Path(transaction_id): Path<String>,
) -> impl IntoResponse {
    // Increment request counters
    state.total_requests.fetch_add(1, Ordering::Relaxed);
    state.successful_requests.fetch_add(1, Ordering::Relaxed);
    
    // TODO: Implement actual transaction status lookup
    // For now, return a placeholder response
    (
        StatusCode::OK,
        Json(TransactionStatusResponse {
            transaction_id,
            status: "pending".to_string(),
            message: "Transaction status retrieved".to_string(),
            details: Some("Implementation pending".to_string()),
        }),
    )
}

async fn cancel_transaction(
    State(state): State<ApiState>,
    Path(transaction_id): Path<String>,
) -> impl IntoResponse {
    // Increment request counters
    state.total_requests.fetch_add(1, Ordering::Relaxed);
    state.successful_requests.fetch_add(1, Ordering::Relaxed);
    
    // TODO: Implement actual transaction cancellation
    // For now, return a placeholder response
    (
        StatusCode::OK,
        Json(TransactionStatusResponse {
            transaction_id,
            status: "cancelled".to_string(),
            message: "Transaction cancellation requested".to_string(),
            details: Some("Implementation pending".to_string()),
        }),
    )
}
