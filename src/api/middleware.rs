use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

use crate::types::RelayerError;

#[derive(Debug, Clone)]
pub struct MiddlewareManager {
    request_counters: Arc<RwLock<HashMap<String, RequestCounter>>>,
    error_counters: Arc<RwLock<HashMap<String, ErrorCounter>>>,
}

#[derive(Debug, Clone)]
pub struct RequestCounter {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub last_request_time: Option<Instant>,
    pub average_response_time: Duration,
}

#[derive(Debug, Clone)]
pub struct ErrorCounter {
    pub total_errors: u64,
    pub error_types: HashMap<u16, u64>,
    pub last_error_time: Option<Instant>,
}

impl MiddlewareManager {
    pub fn new() -> Self {
        Self {
            request_counters: Arc::new(RwLock::new(HashMap::new())),
            error_counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn record_request(&self, identifier: &str, success: bool, response_time: Duration) {
        let mut counters = self.request_counters.write().await;
        let counter = counters.entry(identifier.to_string()).or_insert_with(|| RequestCounter {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            last_request_time: None,
            average_response_time: Duration::from_millis(0),
        });

        counter.total_requests += 1;
        if success {
            counter.successful_requests += 1;
        } else {
            counter.failed_requests += 1;
        }
        counter.last_request_time = Some(Instant::now());

        // Update average response time
        let total_time = counter.average_response_time.as_millis() as u64 * (counter.total_requests - 1)
            + response_time.as_millis() as u64;
        counter.average_response_time = Duration::from_millis(total_time / counter.total_requests);
    }

    pub async fn record_error(&self, identifier: &str, status_code: u16) {
        let mut counters = self.error_counters.write().await;
        let counter = counters.entry(identifier.to_string()).or_insert_with(|| ErrorCounter {
            total_errors: 0,
            error_types: HashMap::new(),
            last_error_time: None,
        });

        counter.total_errors += 1;
        *counter.error_types.entry(status_code).or_insert(0) += 1;
        counter.last_error_time = Some(Instant::now());
    }

    pub async fn get_stats(&self) -> Result<MiddlewareStats, RelayerError> {
        let request_counters = self.request_counters.read().await;
        let error_counters = self.error_counters.read().await;

        let mut total_requests = 0u64;
        let mut total_successful = 0u64;
        let mut total_failed = 0u64;
        let mut total_errors = 0u64;

        for counter in request_counters.values() {
            total_requests += counter.total_requests;
            total_successful += counter.successful_requests;
            total_failed += counter.failed_requests;
        }

        for counter in error_counters.values() {
            total_errors += counter.total_errors;
        }

        let success_rate = if total_requests > 0 {
            total_successful as f64 / total_requests as f64
        } else {
            0.0
        };

        Ok(MiddlewareStats {
            total_requests,
            total_successful,
            total_failed,
            total_errors,
            success_rate,
            active_identifiers: request_counters.len(),
        })
    }
}

pub async fn metrics_middleware(
    State(manager): State<Arc<MiddlewareManager>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let start = Instant::now();
    let identifier = extract_identifier(&request);
    
    let response = next.run(request).await;
    let duration = start.elapsed();
    
    let success = response.status().is_success();
    manager.record_request(&identifier, success, duration).await;
    
    if !success {
        manager.record_error(&identifier, response.status().as_u16()).await;
    }
    
    Ok(response)
}

pub async fn request_id_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let request_id = uuid::Uuid::new_v4().to_string();
    request.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap(),
    );
    
    Ok(next.run(request).await)
}

pub async fn timeout_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let timeout_duration = Duration::from_secs(30);
    
    match tokio::time::timeout(timeout_duration, next.run(request)).await {
        Ok(response) => Ok(response),
        Err(_) => Err(StatusCode::REQUEST_TIMEOUT),
    }
}

pub async fn compression_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let response = next.run(request).await;
    
    // Add compression headers
    let mut response = response;
    let headers = response.headers_mut();
    
    headers.insert("content-encoding", "gzip".parse().unwrap());
    
    Ok(response)
}

pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let response = next.run(request).await;
    
    // Add security headers
    let mut response = response;
    let headers = response.headers_mut();
    
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("x-xss-protection", "1; mode=block".parse().unwrap());
    headers.insert("strict-transport-security", "max-age=31536000; includeSubDomains".parse().unwrap());
    headers.insert("content-security-policy", "default-src 'self'".parse().unwrap());
    
    Ok(response)
}

fn extract_identifier(request: &Request) -> String {
    // Extract identifier from request (IP, user agent, etc.)
    let headers = request.headers();
    
    // Try to get client IP
    if let Some(ip) = headers.get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|h| h.to_str().ok()) {
        return ip.to_string();
    }
    
    // Fallback to user agent
    if let Some(user_agent) = headers.get("user-agent")
        .and_then(|h| h.to_str().ok()) {
        return user_agent.to_string();
    }
    
    "unknown".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MiddlewareStats {
    pub total_requests: u64,
    pub total_successful: u64,
    pub total_failed: u64,
    pub total_errors: u64,
    pub success_rate: f64,
    pub active_identifiers: usize,
}

pub struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: Duration,
    state: Arc<RwLock<CircuitBreakerState>>,
}

#[derive(Debug, Clone)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
        }
    }

    pub async fn call<F, R>(&self, f: F) -> Result<R, RelayerError>
    where
        F: FnOnce() -> Result<R, RelayerError>,
    {
        let current_state = self.state.read().await.clone();
        
        match current_state {
            CircuitBreakerState::Open => {
                return Err(RelayerError::Internal("Circuit breaker is open".to_string()));
            }
            CircuitBreakerState::HalfOpen => {
                // Allow one request to test if service is back
                match f() {
                    Ok(result) => {
                        // Success, close the circuit
                        let mut state = self.state.write().await;
                        *state = CircuitBreakerState::Closed;
                        Ok(result)
                    }
                    Err(e) => {
                        // Still failing, keep circuit open
                        let mut state = self.state.write().await;
                        *state = CircuitBreakerState::Open;
                        Err(e)
                    }
                }
            }
            CircuitBreakerState::Closed => {
                match f() {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        // Increment failure count and potentially open circuit
                        self.handle_failure().await;
                        Err(e)
                    }
                }
            }
        }
    }

    async fn handle_failure(&self) {
        // This is a simplified implementation
        // In a real implementation, you'd track failure counts per time window
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Open;
        
        // Schedule recovery attempt
        let state_clone = Arc::clone(&self.state);
        let recovery_timeout = self.recovery_timeout;
        
        tokio::spawn(async move {
            tokio::time::sleep(recovery_timeout).await;
            let mut state = state_clone.write().await;
            *state = CircuitBreakerState::HalfOpen;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_middleware_manager() {
        let manager = MiddlewareManager::new();
        
        manager.record_request("test", true, Duration::from_millis(100)).await;
        manager.record_request("test", false, Duration::from_millis(200)).await;
        
        let stats = manager.get_stats().await.unwrap();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.total_successful, 1);
        assert_eq!(stats.total_failed, 1);
        assert_eq!(stats.success_rate, 0.5);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let circuit_breaker = CircuitBreaker::new(3, Duration::from_secs(1));
        
        // Test successful call
        let result = circuit_breaker.call(|| Ok("success")).await;
        assert!(result.is_ok());
        
        // Test failing call
        let result = circuit_breaker.call(|| Err(RelayerError::Internal("test error".to_string()))).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_error_counter() {
        let manager = MiddlewareManager::new();
        
        manager.record_error("test", 404).await;
        manager.record_error("test", 500).await;
        manager.record_error("test", 404).await;
        
        let stats = manager.get_stats().await.unwrap();
        assert_eq!(stats.total_errors, 3);
    }
}
