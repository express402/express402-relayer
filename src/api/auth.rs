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
pub struct AuthManager {
    api_keys: Arc<RwLock<HashMap<String, ApiKeyInfo>>>,
    jwt_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub key: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub rate_limit: RateLimit,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
    pub requests_per_day: u32,
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    limits: RateLimit,
}

impl AuthManager {
    pub fn new(jwt_secret: String) -> Self {
        Self {
            api_keys: Arc::new(RwLock::new(HashMap::new())),
            jwt_secret,
        }
    }

    pub async fn add_api_key(&self, api_key: ApiKeyInfo) -> Result<(), RelayerError> {
        let mut keys = self.api_keys.write().await;
        keys.insert(api_key.key.clone(), api_key);
        Ok(())
    }

    pub async fn validate_api_key(&self, key: &str) -> Result<Option<ApiKeyInfo>, RelayerError> {
        let mut keys = self.api_keys.write().await;
        
        if let Some(api_key) = keys.get_mut(key) {
            if !api_key.is_active {
                return Ok(None);
            }
            
            // Update last used time
            api_key.last_used = Some(chrono::Utc::now());
            Ok(Some(api_key.clone()))
        } else {
            Ok(None)
        }
    }

    pub async fn revoke_api_key(&self, key: &str) -> Result<bool, RelayerError> {
        let mut keys = self.api_keys.write().await;
        
        if let Some(api_key) = keys.get_mut(key) {
            api_key.is_active = false;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn get_api_key_stats(&self) -> Result<ApiKeyStats, RelayerError> {
        let keys = self.api_keys.read().await;
        
        let total_keys = keys.len();
        let active_keys = keys.values().filter(|k| k.is_active).count();
        let inactive_keys = total_keys - active_keys;
        
        Ok(ApiKeyStats {
            total_keys,
            active_keys,
            inactive_keys,
        })
    }
}

impl RateLimiter {
    pub fn new(limits: RateLimit) -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            limits,
        }
    }

    pub async fn check_rate_limit(&self, identifier: &str) -> Result<bool, RelayerError> {
        let mut requests = self.requests.write().await;
        let now = Instant::now();
        
        // Get or create request history for this identifier
        let request_history = requests.entry(identifier.to_string()).or_insert_with(Vec::new);
        
        // Clean up old requests (older than 1 day)
        let cutoff = now - Duration::from_secs(86400); // 24 hours
        request_history.retain(|&time| time > cutoff);
        
        // Check daily limit
        if request_history.len() >= self.limits.requests_per_day as usize {
            return Ok(false);
        }
        
        // Check hourly limit
        let hour_cutoff = now - Duration::from_secs(3600); // 1 hour
        let hourly_requests = request_history.iter().filter(|&&time| time > hour_cutoff).count();
        if hourly_requests >= self.limits.requests_per_hour as usize {
            return Ok(false);
        }
        
        // Check minute limit
        let minute_cutoff = now - Duration::from_secs(60); // 1 minute
        let minute_requests = request_history.iter().filter(|&&time| time > minute_cutoff).count();
        if minute_requests >= self.limits.requests_per_minute as usize {
            return Ok(false);
        }
        
        // Add current request
        request_history.push(now);
        
        Ok(true)
    }

    pub async fn get_rate_limit_stats(&self, identifier: &str) -> Result<RateLimitStats, RelayerError> {
        let requests = self.requests.read().await;
        let now = Instant::now();
        
        if let Some(request_history) = requests.get(identifier) {
            let minute_cutoff = now - Duration::from_secs(60);
            let hour_cutoff = now - Duration::from_secs(3600);
            let day_cutoff = now - Duration::from_secs(86400);
            
            let minute_requests = request_history.iter().filter(|&&time| time > minute_cutoff).count();
            let hour_requests = request_history.iter().filter(|&&time| time > hour_cutoff).count();
            let day_requests = request_history.iter().filter(|&&time| time > day_cutoff).count();
            
            Ok(RateLimitStats {
                identifier: identifier.to_string(),
                requests_last_minute: minute_requests as u32,
                requests_last_hour: hour_requests as u32,
                requests_last_day: day_requests as u32,
                limit_per_minute: self.limits.requests_per_minute,
                limit_per_hour: self.limits.requests_per_hour,
                limit_per_day: self.limits.requests_per_day,
            })
        } else {
            Ok(RateLimitStats {
                identifier: identifier.to_string(),
                requests_last_minute: 0,
                requests_last_hour: 0,
                requests_last_day: 0,
                limit_per_minute: self.limits.requests_per_minute,
                limit_per_hour: self.limits.requests_per_hour,
                limit_per_day: self.limits.requests_per_day,
            })
        }
    }
}

pub async fn auth_middleware(
    State(auth_manager): State<Arc<AuthManager>>,
    State(rate_limiter): State<Arc<RateLimiter>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract API key from headers
    let api_key = headers
        .get("x-api-key")
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|auth| auth.strip_prefix("Bearer "))
        });

    let api_key = match api_key {
        Some(key) => key,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    // Validate API key
    let api_key_info = match auth_manager.validate_api_key(api_key).await {
        Ok(Some(info)) => info,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    // Check rate limit
    let identifier = format!("{}:{}", api_key_info.name, api_key);
    match rate_limiter.check_rate_limit(&identifier).await {
        Ok(true) => {},
        Ok(false) => return Err(StatusCode::TOO_MANY_REQUESTS),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    // Add API key info to request extensions
    let mut request = request;
    request.extensions_mut().insert(api_key_info);

    Ok(next.run(request).await)
}

pub async fn cors_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let response = next.run(request).await;
    
    // Add CORS headers
    let mut response = response;
    let headers = response.headers_mut();
    
    headers.insert("access-control-allow-origin", "*".parse().unwrap());
    headers.insert("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap());
    headers.insert("access-control-allow-headers", "Content-Type, Authorization, X-API-Key".parse().unwrap());
    headers.insert("access-control-max-age", "86400".parse().unwrap());
    
    Ok(response)
}

pub async fn logging_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    
    let response = next.run(request).await;
    let duration = start.elapsed();
    
    tracing::info!(
        "{} {} - {} - {}ms",
        method,
        uri,
        response.status(),
        duration.as_millis()
    );
    
    Ok(response)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyStats {
    pub total_keys: usize,
    pub active_keys: usize,
    pub inactive_keys: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitStats {
    pub identifier: String,
    pub requests_last_minute: u32,
    pub requests_last_hour: u32,
    pub requests_last_day: u32,
    pub limit_per_minute: u32,
    pub limit_per_hour: u32,
    pub limit_per_day: u32,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            requests_per_minute: 100,
            requests_per_hour: 1000,
            requests_per_day: 10000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auth_manager() {
        let auth_manager = AuthManager::new("test_secret".to_string());
        
        let api_key = ApiKeyInfo {
            key: "test_key".to_string(),
            name: "test".to_string(),
            permissions: vec!["read".to_string(), "write".to_string()],
            rate_limit: RateLimit::default(),
            created_at: chrono::Utc::now(),
            last_used: None,
            is_active: true,
        };
        
        auth_manager.add_api_key(api_key).await.unwrap();
        
        let validated = auth_manager.validate_api_key("test_key").await.unwrap();
        assert!(validated.is_some());
        
        let stats = auth_manager.get_api_key_stats().await.unwrap();
        assert_eq!(stats.total_keys, 1);
        assert_eq!(stats.active_keys, 1);
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let rate_limiter = RateLimiter::new(RateLimit {
            requests_per_minute: 2,
            requests_per_hour: 10,
            requests_per_day: 100,
        });
        
        let identifier = "test_user";
        
        // First request should be allowed
        assert!(rate_limiter.check_rate_limit(identifier).await.unwrap());
        
        // Second request should be allowed
        assert!(rate_limiter.check_rate_limit(identifier).await.unwrap());
        
        // Third request should be rate limited
        assert!(!rate_limiter.check_rate_limit(identifier).await.unwrap());
    }

    #[tokio::test]
    async fn test_rate_limit_stats() {
        let rate_limiter = RateLimiter::new(RateLimit::default());
        let identifier = "test_user";
        
        let stats = rate_limiter.get_rate_limit_stats(identifier).await.unwrap();
        assert_eq!(stats.identifier, identifier);
        assert_eq!(stats.requests_last_minute, 0);
    }
}
