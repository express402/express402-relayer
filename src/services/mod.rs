use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    config::Config,
    database::DatabaseManager,
    cache::{RedisCache, MemoryCache, CacheManager},
    wallet::{WalletPool, WalletPoolConfig},
    queue::{TaskScheduler, ConcurrencyLimits},
    security::{SignatureVerifier, ReplayProtection, BalanceChecker},
    api::{ApiState, create_router},
    types::{RelayerError, Result},
};

// Type alias for Ethereum provider to simplify usage
pub type EthereumProvider = alloy::providers::RootProvider<
    alloy::providers::fillers::JoinFill<alloy::providers::fillers::RecommendedFiller>
>;

pub struct ServiceManager {
    pub config: Config,
    pub database: DatabaseManager,
    pub redis_cache: Option<RedisCache>,
    pub memory_cache: MemoryCache<String>,
    pub cache_manager: CacheManager<String>,
    pub wallet_pool: WalletPool,
    pub task_scheduler: TaskScheduler,
    pub signature_verifier: SignatureVerifier,
    pub replay_protection: ReplayProtection,
    pub ethereum_provider: Arc<EthereumProvider>,
    pub balance_checker: Option<BalanceChecker<EthereumProvider>>,
}

impl ServiceManager {
    pub async fn new(config: Config) -> Result<Self> {
        tracing::info!("Initializing services...");

        // Initialize database
        let database = DatabaseManager::new(&config).await?;
        database.run_migrations().await?;
        database.check_connection().await?;
        tracing::info!("Database initialized successfully");

        // Initialize Redis cache
        let redis_cache = match RedisCache::new(
            &config.redis.url,
            config.redis.key_prefix.clone(),
            std::time::Duration::from_secs(300), // 5 minutes default TTL
        ) {
            Ok(cache) => {
                if let Err(e) = cache.connect().await {
                    tracing::warn!("Failed to connect to Redis: {}", e);
                    None
                } else {
                    tracing::info!("Redis cache initialized successfully");
                    Some(cache)
                }
            }
            Err(e) => {
                tracing::warn!("Failed to initialize Redis cache: {}", e);
                None
            }
        };

        // Initialize memory cache
        let memory_cache = MemoryCache::new(
            1000, // max 1000 entries
            std::time::Duration::from_secs(300), // 5 minutes TTL
            std::time::Duration::from_secs(60), // cleanup every minute
        );

        // Initialize cache manager
        let cache_manager = CacheManager::new(
            memory_cache.clone(),
            redis_cache.clone(),
            redis_cache.is_some(),
        );

        // Initialize wallet pool
        let wallet_config = WalletPoolConfig {
            min_wallets: config.wallets.max_concurrent_transactions as usize,
            max_wallets: (config.wallets.max_concurrent_transactions * 2) as usize,
            balance_threshold: alloy::primitives::U256::from(config.wallets.min_balance),
            rotation_interval: config.wallets.retry_delay,
            health_check_interval: 60, // 1 minute
        };
        let wallet_pool = WalletPool::new(wallet_config);

        // Initialize task scheduler
        let concurrency_limits = ConcurrencyLimits {
            max_concurrent_tasks: config.queue.worker_threads,
            max_cpu_usage: 80.0,
            max_memory_usage: 80.0,
            max_network_usage: 80.0,
            task_timeout: std::time::Duration::from_secs(config.queue.processing_timeout),
        };
        let task_scheduler = TaskScheduler::new(
            config.queue.worker_threads,
            config.queue.max_queue_size,
            std::time::Duration::from_secs(config.queue.processing_timeout),
        );

        // Initialize signature verifier
        let signature_verifier = SignatureVerifier::new(
            alloy::primitives::U256::from(config.ethereum.chain_id),
            alloy::primitives::Address::ZERO, // This should be the relayer contract address
        );

        // Initialize replay protection
        let replay_protection = ReplayProtection::new(
            std::time::Duration::from_secs(config.security.nonce_window),
            std::time::Duration::from_secs(300), // 5 minutes cleanup interval
        );

        // Initialize Ethereum provider
        tracing::info!("Initializing Ethereum provider: {}", config.ethereum.rpc_url);
        let ethereum_provider = Arc::new(
            alloy::providers::ProviderBuilder::new()
                .on_http(
                    alloy::providers::HttpProviderBuilder::new()
                        .with_url(&config.ethereum.rpc_url)
                        .map_err(|e| RelayerError::Ethereum(format!("Failed to create HTTP provider: {}", e)))?
                )
        );

        // Test provider connection
        match ethereum_provider.get_chain_id().await {
            Ok(chain_id) => {
                tracing::info!("Ethereum provider connected successfully. Chain ID: {}", chain_id);
                if chain_id != alloy::primitives::U64::from(config.ethereum.chain_id) {
                    tracing::warn!(
                        "Chain ID mismatch: configured {}, provider returned {}",
                        config.ethereum.chain_id,
                        chain_id
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to connect to Ethereum provider: {}. Continuing without balance checking.", e);
            }
        }

        // Initialize balance checker
        let balance_checker = Some(BalanceChecker::new(
            Arc::clone(&ethereum_provider),
            alloy::primitives::U256::from(config.wallets.min_balance),
            std::time::Duration::from_secs(60), // 1 minute cache TTL
        ));

        tracing::info!("All services initialized successfully");

        Ok(Self {
            config,
            database,
            redis_cache,
            memory_cache,
            cache_manager,
            wallet_pool,
            task_scheduler,
            signature_verifier,
            replay_protection,
            ethereum_provider,
            balance_checker,
        })
    }

    pub async fn start_background_tasks(&self) -> Result<()> {
        tracing::info!("Starting background tasks...");

        // Start wallet balance monitoring if balance_checker is available
        if let Some(ref balance_checker) = self.balance_checker {
            use crate::wallet::monitor::WalletMonitor;
            use tokio::time::Duration;
            
            // Get wallet addresses from wallet pool
            // Note: This assumes WalletPool has a method to get wallet addresses
            // For now, we'll create an empty monitor and add wallets later
            let wallets = Arc::new(RwLock::new(vec![])); // Empty initially
            
            let monitor = WalletMonitor::new(
                Arc::clone(&self.ethereum_provider),
                wallets,
                Duration::from_secs(60), // Check every minute
                alloy::primitives::U256::from(self.config.wallets.min_balance),
                10, // Max nonce gap
            );

            // Start monitoring in background
            if let Err(e) = monitor.start_monitoring().await {
                tracing::warn!("Failed to start wallet monitoring: {}", e);
            } else {
                tracing::info!("Wallet monitoring started");
            }
        }

        // Start cache cleanup tasks
        // These are already started in their constructors

        // Start replay protection cleanup
        // This is already started in its constructor

        tracing::info!("Background tasks started successfully");
        Ok(())
    }

    pub fn create_api_state(&self) -> ApiState {
        ApiState {
            database_manager: Arc::new(self.database.clone()),
            cache_manager: Arc::new(self.cache_manager.clone()),
            ethereum_provider: Arc::clone(&self.ethereum_provider),
            wallet_pool: Arc::new(self.wallet_pool.clone()),
            task_scheduler: Arc::new(self.task_scheduler.clone()),
            signature_verifier: Arc::new(self.signature_verifier.clone()),
            replay_protection: Arc::new(self.replay_protection.clone()),
            config: Arc::new(self.config.clone()),
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down services...");

        // Close database connections
        self.database.get_pool().close().await;

        // Close Redis connections
        if let Some(ref redis_cache) = self.redis_cache {
            // Redis connections are automatically closed when dropped
        }

        tracing::info!("Services shutdown completed");
        Ok(())
    }

    pub async fn health_check(&self) -> Result<ServiceHealth> {
        let mut health = ServiceHealth {
            overall_status: "healthy".to_string(),
            services: std::collections::HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        // Check database
        match self.database.check_connection().await {
            Ok(_) => {
                health.services.insert("database".to_string(), "healthy".to_string());
            }
            Err(e) => {
                health.services.insert("database".to_string(), format!("unhealthy: {}", e));
                health.overall_status = "degraded".to_string();
            }
        }

        // Check Redis
        if let Some(ref redis_cache) = self.redis_cache {
            match redis_cache.ping().await {
                Ok(true) => {
                    health.services.insert("redis".to_string(), "healthy".to_string());
                }
                Ok(false) => {
                    health.services.insert("redis".to_string(), "unhealthy: ping failed".to_string());
                    health.overall_status = "degraded".to_string();
                }
                Err(e) => {
                    health.services.insert("redis".to_string(), format!("unhealthy: {}", e));
                    health.overall_status = "degraded".to_string();
                }
            }
        } else {
            health.services.insert("redis".to_string(), "disabled".to_string());
        }

        // Check wallet pool
        let wallet_stats = self.wallet_pool.get_pool_stats().await.unwrap_or_default();
        if wallet_stats.healthy_wallets > 0 {
            health.services.insert("wallet_pool".to_string(), "healthy".to_string());
        } else {
            health.services.insert("wallet_pool".to_string(), "unhealthy: no healthy wallets".to_string());
            health.overall_status = "degraded".to_string();
        }

        // Check task scheduler
        let queue_stats = self.task_scheduler.get_queue_stats().await.unwrap_or_default();
        if queue_stats.available_permits > 0 {
            health.services.insert("task_scheduler".to_string(), "healthy".to_string());
        } else {
            health.services.insert("task_scheduler".to_string(), "unhealthy: no available permits".to_string());
            health.overall_status = "degraded".to_string();
        }

        // Check Ethereum provider
        match self.ethereum_provider.get_chain_id().await {
            Ok(chain_id) => {
                health.services.insert("ethereum_provider".to_string(), 
                    format!("healthy (Chain ID: {})", chain_id));
            }
            Err(e) => {
                health.services.insert("ethereum_provider".to_string(), 
                    format!("unhealthy: {}", e));
                health.overall_status = "degraded".to_string();
            }
        }

        Ok(health)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ServiceHealth {
    pub overall_status: String,
    pub services: std::collections::HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Clone for ServiceManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            database: self.database.clone(),
            redis_cache: self.redis_cache.clone(),
            memory_cache: self.memory_cache.clone(),
            cache_manager: self.cache_manager.clone(),
            wallet_pool: self.wallet_pool.clone(),
            task_scheduler: self.task_scheduler.clone(),
            signature_verifier: self.signature_verifier.clone(),
            replay_protection: self.replay_protection.clone(),
            ethereum_provider: Arc::clone(&self.ethereum_provider),
            balance_checker: self.balance_checker.clone(),
        }
    }
}
