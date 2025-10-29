use crate::types::{RelayerError, Result};
use std::time::Duration;
use tokio::time::sleep;

/// Retry configuration for operations
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub retryable_errors: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            retryable_errors: vec![
                "network".to_string(),
                "timeout".to_string(),
                "temporary".to_string(),
            ],
        }
    }
}

/// Retry helper with exponential backoff
pub struct RetryHelper;

impl RetryHelper {
    /// Execute a function with retry logic (generic error version)
    pub async fn retry_with_backoff<F, Fut, T, E>(
        config: &RetryConfig,
        mut operation: F,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>> + Send,
        E: std::fmt::Display,
    {
        let mut delay = config.initial_delay;
        let mut last_error: Option<E> = None;

        for attempt in 0..=config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);

                    // Check if this is the last attempt
                    if attempt >= config.max_retries {
                        break;
                    }

                    // Check if error is retryable
                    let error_string = if let Some(ref err) = last_error {
                        format!("{}", err)
                    } else {
                        "Unknown error".to_string()
                    };
                    let is_retryable = config.retryable_errors.is_empty()
                        || config.retryable_errors.iter().any(|pattern| error_string.contains(pattern));

                    if !is_retryable {
                        break;
                    }

                    // Exponential backoff
                    tracing::warn!(
                        "Operation failed (attempt {}/{}), retrying after {:?}: {}",
                        attempt + 1,
                        config.max_retries + 1,
                        delay,
                        error_string
                    );

                    sleep(delay).await;

                    // Calculate next delay
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * config.backoff_multiplier)
                            .min(config.max_delay.as_secs_f64())
                    );
                }
            }
        }

        Err(RelayerError::Internal(format!(
            "Operation failed after {} retries: {}",
            config.max_retries + 1,
            last_error
                .map(|e| format!("{}", e))
                .unwrap_or_else(|| "Unknown error".to_string())
        )))
    }

    /// Execute a function with retry logic (async closure version)
    pub async fn retry<F, Fut, T>(
        config: &RetryConfig,
        mut operation: F,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut delay = config.initial_delay;
        let mut last_error: Option<RelayerError> = None;

        for attempt in 0..=config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e.clone());

                    // Check if this is the last attempt
                    if attempt >= config.max_retries {
                        break;
                    }

                    // Check if error is retryable
                    let error_string = format!("{}", &e);
                    let is_retryable = config.retryable_errors.is_empty()
                        || config.retryable_errors.iter().any(|pattern| error_string.contains(pattern));

                    if !is_retryable {
                        break;
                    }

                    // Exponential backoff
                    tracing::warn!(
                        "Operation failed (attempt {}/{}), retrying after {:?}: {}",
                        attempt + 1,
                        config.max_retries + 1,
                        delay,
                        error_string
                    );

                    sleep(delay).await;

                    // Calculate next delay
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * config.backoff_multiplier)
                            .min(config.max_delay.as_secs_f64())
                    );
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            RelayerError::Internal("Operation failed after retries".to_string())
        }))
    }
}

/// Check if an error is retryable
pub fn is_retryable_error(error: &RelayerError) -> bool {
    match error {
        RelayerError::Ethereum(_) => true,
        RelayerError::Database(_) => true,
        RelayerError::Redis(_) => true,
        RelayerError::Network(_) => true,
        RelayerError::Timeout(_) => true,
        RelayerError::WalletPool(_) => {
            // Wallet pool errors might be retryable if it's a temporary unavailability
            true
        }
        _ => false,
    }
}

/// Context wrapper for adding context to errors
pub struct ErrorContext {
    pub operation: String,
    pub details: Vec<(String, String)>,
}

impl ErrorContext {
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            details: Vec::new(),
        }
    }

    pub fn add_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push((key.into(), value.into()));
        self
    }

    pub fn format(&self) -> String {
        let mut parts = vec![format!("Operation: {}", self.operation)];
        for (key, value) in &self.details {
            parts.push(format!("{}: {}", key, value));
        }
        parts.join(", ")
    }

    pub fn wrap_error<E: std::error::Error>(self, error: E) -> RelayerError {
        RelayerError::Internal(format!("{} - {}", self.format(), error))
    }
}

