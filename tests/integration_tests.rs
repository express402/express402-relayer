use express402_relayer::api::gateway_simple::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tower::ServiceExt;
use hyper::body::to_bytes;

fn create_test_router() -> Router {
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

#[tokio::test]
async fn test_full_api_workflow() {
    let app = create_test_router();

    // Test health check
    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test stats endpoint
    let request = Request::builder()
        .uri("/stats")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test metrics endpoint
    let request = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test transaction submission
    let transaction_request = json!({
        "user_address": "0x1234567890123456789012345678901234567890",
        "target_contract": "0x0987654321098765432109876543210987654321",
        "calldata": "0x1234",
        "value": "1000000000000000000",
        "gas_limit": "21000",
        "max_fee_per_gas": "20000000000",
        "max_priority_fee_per_gas": "2000000000",
        "nonce": "1",
        "signature_r": "0x1234567890123456789012345678901234567890123456789012345678901234",
        "signature_s": "0x0987654321098765432109876543210987654321098765432109876543210987",
        "signature_v": 27,
        "priority": "normal"
    });

    let request = Request::builder()
        .uri("/transactions")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&transaction_request).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = to_bytes(response.into_body()).await.unwrap();
    let submit_response: TransactionSubmitResponse = serde_json::from_slice(&body).unwrap();
    let transaction_id = submit_response.transaction_id;

    // Test transaction status query
    let request = Request::builder()
        .uri(&format!("/transactions/{}", transaction_id))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test transaction cancellation
    let request = Request::builder()
        .uri(&format!("/transactions/{}/cancel", transaction_id))
        .method("POST")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_error_handling() {
    let app = create_test_router();

    // Test invalid JSON in transaction submission
    let request = Request::builder()
        .uri("/transactions")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from("invalid json"))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    // Should handle JSON parsing error gracefully
    assert!(response.status() != StatusCode::ACCEPTED);

    // Test non-existent transaction status
    let request = Request::builder()
        .uri("/transactions/non-existent-id")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_concurrent_requests() {
    let app = create_test_router();

    // Test concurrent health checks
    let mut handles = vec![];
    for _ in 0..10 {
        let app = app.clone();
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .uri("/health")
                .method("GET")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            response.status()
        });
        handles.push(handle);
    }

    for handle in handles {
        let status = handle.await.unwrap();
        assert_eq!(status, StatusCode::OK);
    }
}

#[tokio::test]
async fn test_metrics_consistency() {
    let app = create_test_router();

    // Submit a transaction
    let transaction_request = json!({
        "user_address": "0x1234567890123456789012345678901234567890",
        "target_contract": "0x0987654321098765432109876543210987654321",
        "calldata": "0x1234",
        "value": "1000000000000000000",
        "gas_limit": "21000",
        "max_fee_per_gas": "20000000000",
        "max_priority_fee_per_gas": "2000000000",
        "nonce": "1",
        "signature_r": "0x1234567890123456789012345678901234567890123456789012345678901234",
        "signature_s": "0x0987654321098765432109876543210987654321098765432109876543210987",
        "signature_v": 27,
        "priority": "normal"
    });

    let request = Request::builder()
        .uri("/transactions")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&transaction_request).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Check metrics
    let request = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body()).await.unwrap();
    let metrics_text = String::from_utf8(body.to_vec()).unwrap();
    
    // Should have incremented counters
    assert!(metrics_text.contains("express402_total_requests 1"));
    assert!(metrics_text.contains("express402_successful_requests 1"));
    assert!(metrics_text.contains("express402_total_transactions 1"));
    assert!(metrics_text.contains("express402_pending_transactions 1"));
}
