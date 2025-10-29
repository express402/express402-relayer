use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: u32,
    pub request_timeout: u64, // seconds
    pub rate_limit_per_minute: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            max_connections: 1000,
            request_timeout: 30,
            rate_limit_per_minute: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: u64, // seconds
    pub idle_timeout: u64, // seconds
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/express402_relayer".to_string(),
            max_connections: 20,
            min_connections: 5,
            connection_timeout: 30,
            idle_timeout: 600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout: u64, // seconds
    pub command_timeout: u64, // seconds
    pub key_prefix: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: 20,
            connection_timeout: 5,
            command_timeout: 3,
            key_prefix: "express402:".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumConfig {
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub chain_id: u64,
    pub gas_price_multiplier: f64,
    pub max_gas_price: u64,
    pub min_gas_price: u64,
    pub confirmation_blocks: u64,
}

impl Default for EthereumConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            ws_url: Some("ws://localhost:8546".to_string()),
            chain_id: 1,
            gas_price_multiplier: 1.1,
            max_gas_price: 100000000000, // 100 gwei
            min_gas_price: 1000000000,   // 1 gwei
            confirmation_blocks: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub private_keys: Vec<String>,
    pub min_balance: u64, // wei
    pub max_concurrent_transactions: u32,
    pub transaction_timeout: u64, // seconds
    pub retry_attempts: u32,
    pub retry_delay: u64, // seconds
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            private_keys: vec![],
            min_balance: 1000000000000000000, // 1 ETH
            max_concurrent_transactions: 5,
            transaction_timeout: 60,
            retry_attempts: 3,
            retry_delay: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub signature_timeout: u64, // seconds
    pub nonce_window: u64, // seconds
    pub max_pending_transactions: u32,
    pub enable_replay_protection: bool,
    pub trusted_contracts: Vec<Address>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            signature_timeout: 300, // 5 minutes
            nonce_window: 3600, // 1 hour
            max_pending_transactions: 1000,
            enable_replay_protection: true,
            trusted_contracts: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    pub max_queue_size: usize,
    pub worker_threads: usize,
    pub batch_size: usize,
    pub processing_timeout: u64, // seconds
    pub priority_weights: HashMap<String, u8>,
}

impl Default for QueueConfig {
    fn default() -> Self {
        let mut priority_weights = HashMap::new();
        priority_weights.insert("low".to_string(), 1);
        priority_weights.insert("normal".to_string(), 2);
        priority_weights.insert("high".to_string(), 3);
        priority_weights.insert("critical".to_string(), 4);

        Self {
            max_queue_size: 10000,
            worker_threads: 4,
            batch_size: 10,
            processing_timeout: 300,
            priority_weights,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub ethereum: EthereumConfig,
    pub wallets: WalletConfig,
    pub security: SecurityConfig,
    pub queue: QueueConfig,
    pub log_level: String,
    pub environment: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            ethereum: EthereumConfig::default(),
            wallets: WalletConfig::default(),
            security: SecurityConfig::default(),
            queue: QueueConfig::default(),
            log_level: "info".to_string(),
            environment: "development".to_string(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, config::ConfigError> {
        let mut settings = config::Config::builder()
            .add_source(config::Environment::with_prefix("EXPRESS402"))
            .build()?;

        settings.try_deserialize()
    }

    pub fn from_file(path: &str) -> Result<Self, config::ConfigError> {
        let mut settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("EXPRESS402"))
            .build()?;

        settings.try_deserialize()
    }

    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }

    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }
}
