#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_health_check() {
        let app = create_test_router();
        let request = Request::builder()
            .uri("/health")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body()).await.unwrap();
        let health_response: HealthCheckResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(health_response.status, "healthy");
        assert!(!health_response.timestamp.is_empty());
        assert!(!health_response.version.is_empty());
    }

    #[tokio::test]
    async fn test_stats_endpoint() {
        let app = create_test_router();
        let request = Request::builder()
            .uri("/stats")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body()).await.unwrap();
        let stats_response: StatsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(stats_response.message, "System statistics retrieved successfully");
        assert_eq!(stats_response.metrics.total_requests, 0);
        assert_eq!(stats_response.metrics.success_rate, 0.0);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let app = create_test_router();
        let request = Request::builder()
            .uri("/metrics")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body()).await.unwrap();
        let metrics_text = String::from_utf8(body.to_vec()).unwrap();
        
        // Check for Prometheus format
        assert!(metrics_text.contains("express402_total_requests"));
        assert!(metrics_text.contains("express402_successful_requests"));
        assert!(metrics_text.contains("express402_failed_requests"));
        assert!(metrics_text.contains("express402_total_transactions"));
    }
}