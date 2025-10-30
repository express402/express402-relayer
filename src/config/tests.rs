#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup_test_env() {
        env::set_var("RUST_LOG", "debug");
        env::set_var("DATABASE_URL", "postgresql://test:test@localhost:5432/test");
        env::set_var("REDIS_URL", "redis://localhost:6379");
        env::set_var("ETHEREUM_RPC_URL", "https://mainnet.infura.io/v3/test");
        env::set_var("WALLET_PRIVATE_KEY", "0x1234567890123456789012345678901234567890123456789012345678901234");
        env::set_var("MIN_BALANCE_THRESHOLD", "100000000000000000");
        env::set_var("MAX_WALLETS", "10");
        env::set_var("API_PORT", "8080");
        env::set_var("API_HOST", "0.0.0.0");
    }

    fn cleanup_test_env() {
        env::remove_var("RUST_LOG");
        env::remove_var("DATABASE_URL");
        env::remove_var("REDIS_URL");
        env::remove_var("ETHEREUM_RPC_URL");
        env::remove_var("WALLET_PRIVATE_KEY");
        env::remove_var("MIN_BALANCE_THRESHOLD");
        env::remove_var("MAX_WALLETS");
        env::remove_var("API_PORT");
        env::remove_var("API_HOST");
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.database_url, "postgresql://localhost:5432/express402");
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.ethereum_rpc_url, "https://mainnet.infura.io/v3/your-project-id");
        assert_eq!(config.wallet_private_key, "your-private-key-here");
        assert_eq!(config.min_balance_threshold, "1000000000000000000");
        assert_eq!(config.max_wallets, 5);
        assert_eq!(config.api_port, 8080);
        assert_eq!(config.api_host, "0.0.0.0");
    }

    #[test]
    fn test_config_from_env() {
        setup_test_env();
        
        let config = Config::from_env();
        assert!(config.is_ok());
        
        let config = config.unwrap();
        assert_eq!(config.database_url, "postgresql://test:test@localhost:5432/test");
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.ethereum_rpc_url, "https://mainnet.infura.io/v3/test");
        assert_eq!(config.wallet_private_key, "0x1234567890123456789012345678901234567890123456789012345678901234");
        assert_eq!(config.min_balance_threshold, "100000000000000000");
        assert_eq!(config.max_wallets, 10);
        assert_eq!(config.api_port, 8080);
        assert_eq!(config.api_host, "0.0.0.0");
        
        cleanup_test_env();
    }

    #[test]
    fn test_config_from_file() {
        let config_content = r#"
        database_url = "postgresql://test:test@localhost:5432/test"
        redis_url = "redis://localhost:6379"
        ethereum_rpc_url = "https://mainnet.infura.io/v3/test"
        wallet_private_key = "0x1234567890123456789012345678901234567890123456789012345678901234"
        min_balance_threshold = "100000000000000000"
        max_wallets = 10
        api_port = 8080
        api_host = "0.0.0.0"
        "#;
        
        let config = Config::from_file(config_content);
        assert!(config.is_ok());
        
        let config = config.unwrap();
        assert_eq!(config.database_url, "postgresql://test:test@localhost:5432/test");
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.ethereum_rpc_url, "https://mainnet.infura.io/v3/test");
        assert_eq!(config.wallet_private_key, "0x1234567890123456789012345678901234567890123456789012345678901234");
        assert_eq!(config.min_balance_threshold, "100000000000000000");
        assert_eq!(config.max_wallets, 10);
        assert_eq!(config.api_port, 8080);
        assert_eq!(config.api_host, "0.0.0.0");
    }

    #[test]
    fn test_config_validation() {
        let config = Config {
            database_url: "postgresql://test:test@localhost:5432/test".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            ethereum_rpc_url: "https://mainnet.infura.io/v3/test".to_string(),
            wallet_private_key: "0x1234567890123456789012345678901234567890123456789012345678901234".to_string(),
            min_balance_threshold: "100000000000000000".to_string(),
            max_wallets: 10,
            api_port: 8080,
            api_host: "0.0.0.0".to_string(),
        };
        
        let validation = config.validate();
        assert!(validation.is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.database_url, deserialized.database_url);
        assert_eq!(config.redis_url, deserialized.redis_url);
        assert_eq!(config.ethereum_rpc_url, deserialized.ethereum_rpc_url);
        assert_eq!(config.wallet_private_key, deserialized.wallet_private_key);
        assert_eq!(config.min_balance_threshold, deserialized.min_balance_threshold);
        assert_eq!(config.max_wallets, deserialized.max_wallets);
        assert_eq!(config.api_port, deserialized.api_port);
        assert_eq!(config.api_host, deserialized.api_host);
    }
}