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

- Rust 1.75+
- PostgreSQL 13+
- Redis 6+
- Docker & Docker Compose (optional)

### Quick Start

```bash
# Clone the repository
git clone <repository-url>
cd express402-relayer

# Setup development environment
make dev-setup

# Start with Docker Compose (recommended)
make docker-run

# Or run manually
make build
make run
```

### Development Setup

1. **Configure Environment**
   ```bash
   cp config.development.env .env
   # Edit .env with your configuration
   ```

2. **Start Services**
   ```bash
   # Using Docker Compose
   make docker-run
   
   # Or manually start PostgreSQL and Redis, then:
   make run
   ```

3. **Verify Installation**
   ```bash
   curl http://localhost:8080/health
   ```

### Available Commands

```bash
make build      # Build the project
make test       # Run tests
make run        # Run the application
make fmt        # Format code
make clippy     # Run linter
make clean      # Clean build artifacts
make docs       # Generate documentation
make docker-run # Start with Docker Compose
make docker-stop # Stop Docker Compose
```

### API Usage

Submit a transaction:
```bash
curl -X POST http://localhost:8080/transactions \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "user_address": "0x1234567890123456789012345678901234567890",
    "target_contract": "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
    "calldata": "0x1234",
    "value": "0",
    "gas_limit": "21000",
    "max_fee_per_gas": "20000000000",
    "max_priority_fee_per_gas": "2000000000",
    "nonce": "0",
    "signature_r": "0x1234",
    "signature_s": "0x5678",
    "signature_v": 27,
    "priority": "normal"
  }'
```

Check transaction status:
```bash
curl http://localhost:8080/transactions/{transaction-id}
```

Get system statistics:
```bash
curl http://localhost:8080/stats
```

## 🚀 Deployment and Operations

### Docker Deployment

The project includes comprehensive Docker support:

```bash
# Build Docker image
make docker-build

# Run with Docker Compose
make docker-run

# View logs
make docker-logs

# Stop services
make docker-stop
```

**Services included:**
- Express402 Relayer (port 8080)
- PostgreSQL database (port 5432)
- Redis cache (port 6379)
- Prometheus metrics (port 9090)
- Grafana dashboards (port 3000)

### Monitoring and Logging

**Built-in Monitoring:**
- Health check endpoint: `/health`
- Metrics endpoint: `/metrics`
- System statistics: `/stats`

**External Monitoring:**
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (admin/admin)
- **Health Dashboard**: Real-time system status
- **Transaction Metrics**: Throughput, latency, error rates
- **Wallet Pool Monitoring**: Health, balance, rotation stats

**Logging:**
- Structured JSON logging with tracing
- Configurable log levels via environment variables
- Request/response logging with correlation IDs
- Error tracking and alerting

### Production Deployment

1. **Environment Configuration**
   ```bash
   cp config.example.env .env
   # Configure production settings
   ```

2. **Database Setup**
   ```bash
   # Create production database
   createdb express402_relayer_prod
   
   # Run migrations
   make db-migrate
   ```

3. **Security Configuration**
   - Use strong API keys
   - Enable TLS/SSL
   - Configure firewall rules
   - Set up rate limiting

4. **Monitoring Setup**
   - Configure Prometheus scraping
   - Set up Grafana dashboards
   - Configure alerting rules
   - Set up log aggregation

5. **Load Balancing**
   - Use multiple relayer instances
   - Configure health checks
   - Set up failover mechanisms

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
