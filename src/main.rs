use express402_relayer::{
    api::gateway_simple,
    config::Config,
    types::RelayerError,
};
use axum::{
    middleware,
    response::Response,
    http::Request,
};
use tower::ServiceExt;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), RelayerError> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Express402 Relayer Service");

    // Load configuration with defaults
    let config = Config::default();
    info!("Using default configuration");

    // For now, use simplified router without services
    let app = gateway_simple::create_router();

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
    let std_listener = listener.into_std().map_err(|e| RelayerError::Internal(e.to_string()))?;
    axum::Server::from_tcp(std_listener)
        .map_err(|e| RelayerError::Internal(e.to_string()))?
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal)
        .await
        .map_err(|e| RelayerError::Internal(e.to_string()))?;

    // Services shutdown not needed for simplified version

    info!("Server shutdown completed");
    Ok(())
}


// Middleware functions (simplified versions)
async fn cors_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next<axum::body::Body>,
) -> Response {
    let response = next.run(request).await;
    let mut response = response;
    let headers = response.headers_mut();
    
    headers.insert("access-control-allow-origin", "*".parse().unwrap());
    headers.insert("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap());
    headers.insert("access-control-allow-headers", "Content-Type, Authorization, X-API-Key".parse().unwrap());
    
    response
}

async fn logging_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next<axum::body::Body>,
) -> Response {
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
    mut request: Request<axum::body::Body>,
    next: middleware::Next<axum::body::Body>,
) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    request.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap(),
    );
    
    next.run(request).await
}

async fn security_headers_middleware(
    request: Request<axum::body::Body>,
    next: middleware::Next<axum::body::Body>,
) -> Response {
    let response = next.run(request).await;
    let mut response = response;
    let headers = response.headers_mut();
    
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("x-xss-protection", "1; mode=block".parse().unwrap());
    
    response
}
