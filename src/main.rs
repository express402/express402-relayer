use express402_relayer::{
    api::{create_router, ApiState},
    config::Config,
    types::RelayerError,
};
use axum::{
    middleware,
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), RelayerError> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Express402 Relayer Service");

    // Load configuration
    let config = Config::from_env()
        .map_err(|e| RelayerError::Config(e.to_string()))?;

    info!("Configuration loaded successfully");

    // Initialize services
    let api_state = initialize_services(config).await?;

    // Create the router with middleware
    let app = create_router(api_state)
        .layer(middleware::from_fn(cors_middleware))
        .layer(middleware::from_fn(logging_middleware))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(middleware::from_fn(security_headers_middleware));

    // Start the server
    let listener = TcpListener::bind(format!("{}:{}", config.server.host, config.server.port))
        .await
        .map_err(|e| RelayerError::Internal(e.to_string()))?;

    info!("Server listening on {}:{}", config.server.host, config.server.port);

    // Start the server
    axum::serve(listener, app)
        .await
        .map_err(|e| RelayerError::Internal(e.to_string()))?;

    Ok(())
}

async fn initialize_services(config: Config) -> Result<ApiState, RelayerError> {
    info!("Initializing services...");

    // Initialize database connection pool
    info!("Connecting to database...");
    let database_manager = express402_relayer::cache::persistence::DatabaseManager::new(&config.database.url)
        .await?;
    
    // Run database migrations
    info!("Running database migrations...");
    database_manager.run_migrations().await?;
    
    // Test database connection
    database_manager.health_check().await?;
    info!("Database connection established successfully");

    // Initialize Redis cache
    info!("Connecting to Redis...");
    let redis_cache = express402_relayer::cache::RedisCache::new(
        &config.redis.url,
        config.redis.key_prefix.clone(),
        std::time::Duration::from_secs(3600), // 1 hour default TTL
    )?;
    
    redis_cache.connect().await?;
    info!("Redis connection established successfully");

    // Initialize memory cache
    let memory_cache = express402_relayer::cache::MemoryCache::new(
        10000, // max 10k entries
        std::time::Duration::from_secs(1800), // 30 minutes default TTL
        std::time::Duration::from_secs(300), // cleanup every 5 minutes
    );

    // Initialize cache manager
    let cache_manager = express402_relayer::cache::CacheManager::new(
        memory_cache,
        Some(redis_cache),
        true, // use Redis
    );

    // Initialize Ethereum provider
    info!("Connecting to Ethereum provider...");
    let ethereum_provider = alloy::providers::ProviderBuilder::new()
        .on_http(config.ethereum.rpc_url.parse()
            .map_err(|e| RelayerError::Ethereum(format!("Invalid RPC URL: {}", e)))?);
    info!("Ethereum provider connected successfully");

    // Initialize wallet pool
    info!("Initializing wallet pool...");
    let wallet_pool_config = express402_relayer::types::WalletPoolConfig {
        max_concurrent_transactions: config.wallets.max_concurrent_transactions,
        min_balance: config.wallets.min_balance,
        transaction_timeout: config.wallets.transaction_timeout,
        retry_attempts: config.wallets.retry_attempts,
        retry_delay: config.wallets.retry_delay,
    };
    let wallet_pool = express402_relayer::wallet::pool::WalletPool::new(wallet_pool_config);

    // Add wallets from configuration
    for private_key_str in &config.wallets.private_keys {
        let private_key_bytes = hex::decode(private_key_str)
            .map_err(|e| RelayerError::WalletPool(format!("Invalid private key: {}", e)))?;
        
        let private_key = alloy::signers::k256::ecdsa::SigningKey::from_bytes(&private_key_bytes.into())
            .map_err(|e| RelayerError::WalletPool(format!("Invalid private key format: {}", e)))?;
        
        wallet_pool.add_wallet(private_key).await?;
    }
    info!("Wallet pool initialized with {} wallets", config.wallets.private_keys.len());

    // Initialize task scheduler
    info!("Initializing task scheduler...");
    let task_scheduler = express402_relayer::queue::scheduler::TaskScheduler::new(
        config.queue.worker_threads,
        config.queue.max_queue_size,
        std::time::Duration::from_secs(config.queue.processing_timeout),
    );
    info!("Task scheduler initialized successfully");

    // Initialize security services
    info!("Initializing security services...");
    let signature_verifier = express402_relayer::security::signature::SignatureVerifier::new(
        alloy::primitives::U256::from(config.ethereum.chain_id),
        alloy::primitives::Address::ZERO, // TODO: Set actual verifying contract address
    );
    info!("Security services initialized successfully");

    // Initialize API state
    let api_state = ApiState {
        database_manager: Arc::new(database_manager),
        cache_manager: Arc::new(cache_manager),
        ethereum_provider: Arc::new(ethereum_provider),
        wallet_pool: Arc::new(wallet_pool),
        task_scheduler: Arc::new(task_scheduler),
        signature_verifier: Arc::new(signature_verifier),
        config: Arc::new(config),
    };

    info!("All services initialized successfully");
    Ok(api_state)
}

// Middleware functions (simplified versions)
async fn cors_middleware(
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let response = next.run(request).await;
    let mut response = response;
    let headers = response.headers_mut();
    
    headers.insert("access-control-allow-origin", "*".parse().unwrap());
    headers.insert("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap());
    headers.insert("access-control-allow-headers", "Content-Type, Authorization, X-API-Key".parse().unwrap());
    
    response
}

async fn logging_middleware(
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let start = std::time::Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    
    let response = next.run(request).await;
    let duration = start.elapsed();
    
    info!(
        "{} {} - {} - {}ms",
        method,
        uri,
        response.status(),
        duration.as_millis()
    );
    
    response
}

async fn request_id_middleware(
    mut request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    request.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap(),
    );
    
    next.run(request).await
}

async fn security_headers_middleware(
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let response = next.run(request).await;
    let mut response = response;
    let headers = response.headers_mut();
    
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("x-xss-protection", "1; mode=block".parse().unwrap());
    
    response
}
