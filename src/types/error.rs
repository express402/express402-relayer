use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayerError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Redis error: {0}")]
    Redis(String),
    
    #[error("Ethereum error: {0}")]
    Ethereum(String),
    
    #[error("Signature verification failed: {0}")]
    SignatureVerification(String),
    
    #[error("Replay attack detected: {0}")]
    ReplayAttack(String),
    
    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),
    
    #[error("Wallet pool error: {0}")]
    WalletPool(String),
    
    #[error("Queue error: {0}")]
    Queue(String),
    
    #[error("Cache error: {0}")]
    Cache(String),
    
    #[error("API error: {0}")]
    Api(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("IO error: {0}")]
    Io(String),
    
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<String> for RelayerError {
    fn from(s: String) -> Self {
        RelayerError::Internal(s)
    }
}

impl From<config::ConfigError> for RelayerError {
    fn from(e: config::ConfigError) -> Self {
        RelayerError::Config(e.to_string())
    }
}

impl From<sqlx::Error> for RelayerError {
    fn from(e: sqlx::Error) -> Self {
        RelayerError::Database(e.to_string())
    }
}

impl From<redis::RedisError> for RelayerError {
    fn from(e: redis::RedisError) -> Self {
        RelayerError::Redis(e.to_string())
    }
}

impl From<serde_json::Error> for RelayerError {
    fn from(e: serde_json::Error) -> Self {
        RelayerError::Serialization(e.to_string())
    }
}

impl From<std::io::Error> for RelayerError {
    fn from(e: std::io::Error) -> Self {
        RelayerError::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, RelayerError>;
