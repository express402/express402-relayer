use express402_relayer::{
    api::{create_router, ApiState},
    config::Config,
    services::ServiceManager,
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
    let service_manager = Arc::new(ServiceManager::new(config.clone()).await?);
    
    // Start background tasks
    service_manager.start_background_tasks().await?;
    
    // Create API state
    let api_state = service_manager.create_api_state();

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

    // Setup graceful shutdown
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Received shutdown signal");
    };

    // Start the server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .map_err(|e| RelayerError::Internal(e.to_string()))?;

    // Shutdown services
    service_manager.shutdown().await?;

    info!("Server shutdown completed");
    Ok(())
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
