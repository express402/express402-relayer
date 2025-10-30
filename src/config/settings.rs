use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::Result;

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

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let mut settings = config::Config::builder()
            .add_source(config::Environment::with_prefix("EXPRESS402"))
            .build()?;

        settings.try_deserialize().map_err(|e| RelayerError::Config(e.to_string()))
    }

    pub fn from_file(path: &str) -> Result<Self> {
        let mut settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("EXPRESS402"))
            .build()?;

        settings.try_deserialize().map_err(|e| RelayerError::Config(e.to_string()))
    }

    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }

    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }

    /// Validate configuration and return any errors found
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Validate server config
        if self.server.port == 0 {
            errors.push(ValidationError {
                field: "server.port".to_string(),
                message: "Port must be greater than 0".to_string(),
            });
        }

        if self.server.max_connections == 0 {
            errors.push(ValidationError {
                field: "server.max_connections".to_string(),
                message: "Max connections must be greater than 0".to_string(),
            });
        }

        if self.server.request_timeout == 0 {
            errors.push(ValidationError {
                field: "server.request_timeout".to_string(),
                message: "Request timeout must be greater than 0".to_string(),
            });
        }

        // Validate database config
        if self.database.url.is_empty() {
            errors.push(ValidationError {
                field: "database.url".to_string(),
                message: "Database URL cannot be empty".to_string(),
            });
        }

        if self.database.max_connections == 0 {
            errors.push(ValidationError {
                field: "database.max_connections".to_string(),
                message: "Max connections must be greater than 0".to_string(),
            });
        }

        if self.database.min_connections > self.database.max_connections {
            errors.push(ValidationError {
                field: "database.min_connections".to_string(),
                message: "Min connections cannot exceed max connections".to_string(),
            });
        }

        // Validate Ethereum config
        if self.ethereum.rpc_url.is_empty() {
            errors.push(ValidationError {
                field: "ethereum.rpc_url".to_string(),
                message: "Ethereum RPC URL cannot be empty".to_string(),
            });
        }

        if self.ethereum.gas_price_multiplier <= 0.0 {
            errors.push(ValidationError {
                field: "ethereum.gas_price_multiplier".to_string(),
                message: "Gas price multiplier must be greater than 0".to_string(),
            });
        }

        if self.ethereum.min_gas_price >= self.ethereum.max_gas_price {
            errors.push(ValidationError {
                field: "ethereum.gas_price".to_string(),
                message: "Min gas price must be less than max gas price".to_string(),
            });
        }

        if self.ethereum.confirmation_blocks == 0 {
            errors.push(ValidationError {
                field: "ethereum.confirmation_blocks".to_string(),
                message: "Confirmation blocks must be greater than 0".to_string(),
            });
        }

        // Validate wallet config
        if self.wallets.private_keys.is_empty() {
            errors.push(ValidationError {
                field: "wallets.private_keys".to_string(),
                message: "At least one wallet private key is required".to_string(),
            });
        }

        if self.wallets.max_concurrent_transactions == 0 {
            errors.push(ValidationError {
                field: "wallets.max_concurrent_transactions".to_string(),
                message: "Max concurrent transactions must be greater than 0".to_string(),
            });
        }

        // Validate queue config
        if self.queue.max_queue_size == 0 {
            errors.push(ValidationError {
                field: "queue.max_queue_size".to_string(),
                message: "Max queue size must be greater than 0".to_string(),
            });
        }

        if self.queue.worker_threads == 0 {
            errors.push(ValidationError {
                field: "queue.worker_threads".to_string(),
                message: "Worker threads must be greater than 0".to_string(),
            });
        }

        if self.queue.batch_size == 0 {
            errors.push(ValidationError {
                field: "queue.batch_size".to_string(),
                message: "Batch size must be greater than 0".to_string(),
            });
        }

        // Validate security config
        if self.security.signature_timeout == 0 {
            errors.push(ValidationError {
                field: "security.signature_timeout".to_string(),
                message: "Signature timeout must be greater than 0".to_string(),
            });
        }

        if self.security.nonce_window == 0 {
            errors.push(ValidationError {
                field: "security.nonce_window".to_string(),
                message: "Nonce window must be greater than 0".to_string(),
            });
        }

        // Validate log level
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.log_level.as_str()) {
            errors.push(ValidationError {
                field: "log_level".to_string(),
                message: format!(
                    "Invalid log level: {}. Must be one of: {}",
                    self.log_level,
                    valid_log_levels.join(", ")
                ),
            });
        }

        errors
    }

    /// Validate and return error if validation fails
    pub fn validate_or_error(&self) -> Result<()> {
        let errors = self.validate();
        if !errors.is_empty() {
            let error_messages: Vec<String> = errors
                .iter()
                .map(|e| format!("{}: {}", e.field, e.message))
                .collect();
            return Err(crate::types::RelayerError::Config(
                format!("Configuration validation failed:\n{}", error_messages.join("\n"))
            ));
        }
        Ok(())
    }
}
