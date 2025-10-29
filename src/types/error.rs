use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayerError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
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
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Timeout error: {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, RelayerError>;
