# x402 Relayer - High-Performance Transaction Relayer Service

A sophisticated relayer architecture based on the x402 payment protocol that solves on-chain confirmation delays and significantly improves transaction processing speed through an advanced multi-wallet queue scheduling mechanism.

## 🚀 Project Overview

The x402 Relayer is a high-performance middleware service designed to address the slow response time of on-chain transactions after user frontend signing. By maintaining an intelligent queue scheduling system with multiple wallets, it enables concurrent processing of transaction requests, dramatically improving user experience and transaction throughput.

## 🏗️ Technical Implementation Architecture

### Core Module Structure

```
src/
├── lib.rs                 # Library entry point
├── main.rs               # Application entry point
├── api/                  # API Gateway Module
│   ├── mod.rs
│   ├── gateway.rs        # Request routing
│   ├── auth.rs           # Authentication & authorization
│   └── middleware.rs     # Middleware (rate limiting, logging)
├── queue/                # Queue Scheduling Module
│   ├── mod.rs
│   ├── scheduler.rs      # Task scheduler
│   ├── priority.rs       # Priority management
│   └── concurrency.rs    # Concurrency control
├── wallet/               # Wallet Pool Management
│   ├── mod.rs
│   ├── pool.rs           # Wallet pool
│   ├── monitor.rs        # Balance monitoring
│   └── rotation.rs       # Rotation strategy
├── security/             # Security Verification
│   ├── mod.rs
│   ├── signature.rs      # Signature verification
│   ├── replay.rs         # Replay attack prevention
│   └── balance.rs        # Prepaid balance checking
├── cache/                # Cache Module
│   ├── mod.rs
│   ├── redis.rs          # Redis cache
│   ├── memory.rs         # Memory cache
│   └── persistence.rs    # Persistent storage
├── config/               # Configuration Management
│   ├── mod.rs
│   └── settings.rs       # Configuration structures
├── types/                # Type Definitions
│   ├── mod.rs
│   ├── transaction.rs    # Transaction types
│   ├── wallet.rs         # Wallet types
│   └── error.rs          # Error types
└── utils/                # Utility Functions
    ├── mod.rs
    ├── crypto.rs         # Cryptographic utilities
    └── time.rs           # Time utilities
```

### Technology Stack

#### Core Dependencies
- **Web Framework**: `axum` - High-performance async web framework
- **Async Runtime**: `tokio` - Async runtime
- **Blockchain Interaction**: `alloy` - Ethereum client library
- **Database**: `sqlx` + `PostgreSQL` - Persistent storage
- **Cache**: `redis` + `tokio-redis` - Caching and session management
- **Message Queue**: `tokio-redis` - Redis-based queue system
- **Configuration**: `config` + `serde` - Configuration parsing
- **Logging**: `tracing` + `tracing-subscriber` - Structured logging
- **Error Handling**: `eyre` + `thiserror` - Error handling
- **Serialization**: `serde` + `serde_json` - JSON serialization
- **Cryptography**: `secp256k1` + `k256` - Elliptic curve cryptography
- **Time**: `chrono` - Time handling

### Core Functionality Implementation

#### API Gateway Module
- Request routing and load balancing
- Authentication and authorization middleware
- Rate limiting and circuit breaker mechanisms
- Request/response logging

#### Queue Scheduling System
- Priority-based task queue
- Concurrency control and resource management
- Task retry and failure handling
- Dynamic load balancing

#### Wallet Pool Management
- Multi-wallet rotation strategy
- Real-time balance monitoring
- Automatic failover
- Wallet health checks

#### Security Verification
- EIP-712 signature verification
- Replay attack prevention (timestamp + nonce)
- Prepaid balance checking
- Transaction parameter validation

#### Cache System
- Redis distributed cache
- Memory LRU cache
- Transaction status persistence
- Session management

### Data Model Design

#### Transaction Request
```rust
pub struct TransactionRequest {
    pub id: Uuid,
    pub user_address: Address,
    pub target_contract: Address,
    pub calldata: Bytes,
    pub value: U256,
    pub gas_limit: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub nonce: U256,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub priority: Priority,
}
```

#### Wallet Information
```rust
pub struct WalletInfo {
    pub address: Address,
    pub private_key: SecretKey,
    pub balance: U256,
    pub nonce: U256,
    pub is_active: bool,
    pub last_used: DateTime<Utc>,
    pub success_rate: f64,
}
```

### Configuration Management

#### Environment Configuration
```rust
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub ethereum: EthereumConfig,
    pub wallets: WalletConfig,
    pub security: SecurityConfig,
}
```

## 🛠️ Installation and Running

### Environment Requirements

- Rust 1.60+
- PostgreSQL 13+
- Redis 6+

### Install Dependencies

```bash
cargo build
```

### Run Tests

```bash
cargo test
```

### Run Application

```bash
cargo run
```

## 🚀 Deployment and Operations

### Docker Deployment
- Multi-stage build for optimized image size
- Health checks and monitoring
- Hot configuration reload

### Monitoring and Logging
- Prometheus metrics collection
- Grafana dashboards
- Structured log output
- Distributed tracing

## ⚡ Performance Optimization Strategies

### Concurrency Processing
- Async non-blocking I/O
- Connection pool management
- Batch processing optimization

### Cache Strategy
- Multi-level cache architecture
- Cache warming and invalidation
- Distributed cache consistency

### Database Optimization
- Read/write separation
- Sharding strategy
- Index optimization

## 🔒 Security Considerations

### Private Key Management
- Hardware Security Module (HSM)
- Key rotation strategy
- Access control

### Network Security
- TLS encrypted transmission
- API rate limiting and protection
- Input validation and filtering

## 🤝 Contribution Guide

Issues and Pull Requests are welcome!

1. Fork the project
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

## 📄 License

MIT License
