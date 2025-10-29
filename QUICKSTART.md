# Quick Start Guide

## Prerequisites

- Rust 1.75+
- PostgreSQL 13+
- Redis 6+
- Docker & Docker Compose (optional)

## Development Setup

### 1. Clone and Setup

```bash
git clone <repository-url>
cd express402-relayer
make dev-setup
```

### 2. Configure Environment

Edit the `.env` file with your configuration:

```bash
# Database
EXPRESS402_DATABASE_URL=postgresql://postgres:password@localhost:5432/express402_relayer_dev

# Redis
EXPRESS402_REDIS_URL=redis://localhost:6379

# Ethereum (use testnet for development)
EXPRESS402_ETHEREUM_RPC_URL=http://localhost:8545
EXPRESS402_ETHEREUM_CHAIN_ID=1337

# Wallets (test private keys)
EXPRESS402_WALLETS_PRIVATE_KEYS=ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

### 3. Start Services

#### Option A: Using Docker Compose (Recommended)

```bash
make docker-run
```

This will start:
- PostgreSQL database
- Redis cache
- Express402 Relayer service
- Prometheus metrics
- Grafana dashboards

#### Option B: Manual Setup

1. Start PostgreSQL and Redis
2. Run migrations: `make db-migrate`
3. Start the service: `make run`

### 4. Verify Installation

Check health endpoint:
```bash
curl http://localhost:8080/health
```

View metrics:
```bash
curl http://localhost:8080/metrics
```

Access Grafana dashboard:
- URL: http://localhost:3000
- Username: admin
- Password: admin

## API Usage

### Submit Transaction

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

### Check Transaction Status

```bash
curl http://localhost:8080/transactions/{transaction-id}
```

### Get Statistics

```bash
curl http://localhost:8080/stats
```

## Development Commands

```bash
make build      # Build the project
make test       # Run tests
make run        # Run the application
make fmt        # Format code
make clippy     # Run linter
make clean      # Clean build artifacts
make docs       # Generate documentation
```

## Monitoring

- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000
- **Health Check**: http://localhost:8080/health
- **Metrics**: http://localhost:8080/metrics

## Troubleshooting

### Common Issues

1. **Database Connection Failed**
   - Check PostgreSQL is running
   - Verify connection string in `.env`
   - Ensure database exists

2. **Redis Connection Failed**
   - Check Redis is running
   - Verify Redis URL in `.env`

3. **Ethereum Provider Issues**
   - Check RPC URL is accessible
   - Verify API key if using external provider
   - Use local testnet for development

### Logs

View application logs:
```bash
# Docker
make docker-logs

# Local
RUST_LOG=debug cargo run
```

## Production Deployment

1. Use production configuration
2. Set up proper database and Redis instances
3. Configure monitoring and alerting
4. Set up load balancing
5. Enable TLS/SSL
6. Configure backup strategies
