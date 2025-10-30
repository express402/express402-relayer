#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, U256};
    use alloy::signers::k256::ecdsa::SigningKey;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn create_test_wallet_info() -> WalletInfo {
        WalletInfo {
            address: Address::from_str("0x1234567890123456789012345678901234567890").unwrap(),
            private_key: SigningKey::random(&mut rand::thread_rng()),
            balance: U256::from(1000000000000000000u64), // 1 ETH
            nonce: 5,
            is_healthy: true,
            last_used: std::time::SystemTime::now(),
        }
    }

    fn create_test_pool_config() -> WalletPoolConfig {
        WalletPoolConfig {
            min_balance_threshold: U256::from(100000000000000000u64), // 0.1 ETH
            max_wallets: 10,
            rotation_strategy: RotationStrategy::RoundRobin,
            health_check_interval: std::time::Duration::from_secs(60),
            balance_check_interval: std::time::Duration::from_secs(30),
        }
    }

    #[test]
    fn test_wallet_info_creation() {
        let wallet_info = create_test_wallet_info();
        assert_eq!(wallet_info.balance, U256::from(1000000000000000000u64));
        assert_eq!(wallet_info.nonce, 5);
        assert!(wallet_info.is_healthy);
    }

    #[test]
    fn test_wallet_pool_config_creation() {
        let config = create_test_pool_config();
        assert_eq!(config.max_wallets, 10);
        assert_eq!(config.rotation_strategy, RotationStrategy::RoundRobin);
        assert_eq!(config.health_check_interval, std::time::Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_wallet_pool_creation() {
        let config = create_test_pool_config();
        let pool = WalletPool::new(config);
        
        assert_eq!(pool.wallets.len(), 0);
        assert_eq!(pool.current_index, 0);
    }

    #[tokio::test]
    async fn test_wallet_pool_add_wallet() {
        let config = create_test_pool_config();
        let mut pool = WalletPool::new(config);
        let wallet_info = create_test_wallet_info();
        
        pool.add_wallet(wallet_info.clone()).await;
        assert_eq!(pool.wallets.len(), 1);
        assert_eq!(pool.wallets[0].address, wallet_info.address);
    }
}