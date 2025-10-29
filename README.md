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
    pub queue: QueueConfig,
    pub log_level: String,
    pub environment: String,
}
```

#### Configuration Options

**Server Configuration:**
- `EXPRESS402_SERVER_HOST`: Server bind address (default: `0.0.0.0`)
- `EXPRESS402_SERVER_PORT`: Server port (default: `8080`)
- `EXPRESS402_SERVER_MAX_CONNECTIONS`: Maximum concurrent connections (default: `1000`)
- `EXPRESS402_SERVER_REQUEST_TIMEOUT`: Request timeout in seconds (default: `30`)
- `EXPRESS402_SERVER_RATE_LIMIT_PER_MINUTE`: Rate limit per minute per IP (default: `100`)

**Database Configuration:**
- `EXPRESS402_DATABASE_URL`: PostgreSQL connection string
- `EXPRESS402_DATABASE_MAX_CONNECTIONS`: Maximum pool size (default: `20`)
- `EXPRESS402_DATABASE_MIN_CONNECTIONS`: Minimum pool size (default: `5`)
- `EXPRESS402_DATABASE_CONNECTION_TIMEOUT`: Connection timeout in seconds (default: `30`)
- `EXPRESS402_DATABASE_IDLE_TIMEOUT`: Idle timeout in seconds (default: `600`)

**Redis Configuration:**
- `EXPRESS402_REDIS_URL`: Redis connection URL (default: `redis://localhost:6379`)
- `EXPRESS402_REDIS_MAX_CONNECTIONS`: Maximum connections (default: `20`)
- `EXPRESS402_REDIS_CONNECTION_TIMEOUT`: Connection timeout (default: `5`)
- `EXPRESS402_REDIS_COMMAND_TIMEOUT`: Command timeout (default: `3`)
- `EXPRESS402_REDIS_KEY_PREFIX`: Key prefix for namespacing (default: `express402:`)

**Ethereum Configuration:**
- `EXPRESS402_ETHEREUM_RPC_URL`: Ethereum RPC endpoint URL
- `EXPRESS402_ETHEREUM_WS_URL`: WebSocket URL for subscriptions (optional)
- `EXPRESS402_ETHEREUM_CHAIN_ID`: Chain ID (default: `1` for mainnet)
- `EXPRESS402_ETHEREUM_GAS_PRICE_MULTIPLIER`: Gas price multiplier (default: `1.1`)
- `EXPRESS402_ETHEREUM_MAX_GAS_PRICE`: Maximum gas price in wei (default: `100000000000`)
- `EXPRESS402_ETHEREUM_MIN_GAS_PRICE`: Minimum gas price in wei (default: `1000000000`)
- `EXPRESS402_ETHEREUM_CONFIRMATION_BLOCKS`: Required confirmation blocks (default: `1`)

**Wallet Configuration:**
- `EXPRESS402_WALLETS_PRIVATE_KEYS`: Comma-separated private keys (without 0x prefix)
- `EXPRESS402_WALLETS_MIN_BALANCE`: Minimum balance in wei before alerting (default: `1 ETH`)
- `EXPRESS402_WALLETS_MAX_CONCURRENT_TRANSACTIONS`: Max concurrent txs per wallet (default: `5`)
- `EXPRESS402_WALLETS_TRANSACTION_TIMEOUT`: Transaction timeout in seconds (default: `60`)
- `EXPRESS402_WALLETS_RETRY_ATTEMPTS`: Retry attempts on failure (default: `3`)
- `EXPRESS402_WALLETS_RETRY_DELAY`: Delay between retries in seconds (default: `5`)

**Security Configuration:**
- `EXPRESS402_SECURITY_SIGNATURE_TIMEOUT`: Signature validity window in seconds (default: `300`)
- `EXPRESS402_SECURITY_NONCE_WINDOW`: Nonce validity window in seconds (default: `3600`)
- `EXPRESS402_SECURITY_MAX_PENDING_TRANSACTIONS`: Max pending transactions per user (default: `1000`)
- `EXPRESS402_SECURITY_ENABLE_REPLAY_PROTECTION`: Enable replay attack protection (default: `true`)

**Queue Configuration:**
- `EXPRESS402_QUEUE_MAX_QUEUE_SIZE`: Maximum queue size (default: `10000`)
- `EXPRESS402_QUEUE_WORKER_THREADS`: Number of worker threads (default: `4`)
- `EXPRESS402_QUEUE_BATCH_SIZE`: Batch processing size (default: `10`)
- `EXPRESS402_QUEUE_PROCESSING_TIMEOUT`: Processing timeout in seconds (default: `300`)

**Logging Configuration:**
- `EXPRESS402_LOG_LEVEL`: Log level (`trace`, `debug`, `info`, `warn`, `error`)
- `EXPRESS402_ENVIRONMENT`: Environment (`development`, `staging`, `production`)
- `RUST_LOG`: Rust log filter (e.g., `info,express402_relayer=debug`)

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

## 💻 Client SDK & Integration Examples

### JavaScript/TypeScript SDK

```typescript
import { Express402Relayer } from '@express402/relayer-sdk';

const relayer = new Express402Relayer({
  apiUrl: 'http://localhost:8080',
  apiKey: 'your-api-key',
});

// Submit a transaction
async function submitTransaction() {
  try {
    const response = await relayer.submitTransaction({
      userAddress: '0x1234...',
      targetContract: '0xabcd...',
      calldata: '0x1234',
      value: '0',
      gasLimit: '21000',
      maxFeePerGas: '20000000000',
      maxPriorityFeePerGas: '2000000000',
      nonce: '0',
      signature: {
        r: '0x1234',
        s: '0x5678',
        v: 27,
      },
      priority: 'normal',
    });
    
    console.log('Transaction submitted:', response.transactionId);
    
    // Poll for status
    const status = await relayer.waitForConfirmation(response.transactionId);
    console.log('Transaction confirmed:', status);
  } catch (error) {
    console.error('Transaction failed:', error);
  }
}
```

### Python SDK

```python
from express402_relayer import RelayerClient

client = RelayerClient(
    api_url="http://localhost:8080",
    api_key="your-api-key"
)

# Submit a transaction
def submit_transaction():
    try:
        response = client.submit_transaction(
            user_address="0x1234...",
            target_contract="0xabcd...",
            calldata="0x1234",
            value="0",
            gas_limit="21000",
            max_fee_per_gas="20000000000",
            max_priority_fee_per_gas="2000000000",
            nonce="0",
            signature_r="0x1234",
            signature_s="0x5678",
            signature_v=27,
            priority="normal"
        )
        
        print(f"Transaction submitted: {response['transaction_id']}")
        
        # Wait for confirmation
        status = client.wait_for_confirmation(response['transaction_id'])
        print(f"Transaction confirmed: {status}")
    except Exception as e:
        print(f"Transaction failed: {e}")
```

### Web3 Integration (Ethers.js)

```javascript
import { ethers } from 'ethers';
import { Express402Relayer } from '@express402/relayer-sdk';

const provider = new ethers.JsonRpcProvider('http://localhost:8545');
const relayer = new Express402Relayer({
  apiUrl: 'http://localhost:8080',
  apiKey: 'your-api-key',
});

async function relayTransaction(contract, functionName, args) {
  // Build transaction
  const iface = contract.interface;
  const calldata = iface.encodeFunctionData(functionName, args);
  
  // Sign with user's wallet
  const message = ethers.solidityPackedKeccak256(
    ['address', 'address', 'bytes', 'uint256'],
    [userAddress, contract.address, calldata, nonce]
  );
  
  const signature = await signer.signMessage(ethers.getBytes(message));
  const { r, s, v } = ethers.Signature.from(signature);
  
  // Submit to relayer
  const response = await relayer.submitTransaction({
    userAddress: userAddress,
    targetContract: contract.address,
    calldata: calldata,
    value: '0',
    gasLimit: '100000',
    maxFeePerGas: await provider.getFeeData().then(f => f.maxFeePerGas.toString()),
    maxPriorityFeePerGas: await provider.getFeeData().then(f => f.maxPriorityFeePerGas.toString()),
    nonce: nonce.toString(),
    signature: { r, s, v },
    priority: 'normal',
  });
  
  return response;
}
```

### React Hook Example

```typescript
import { useState, useEffect } from 'react';
import { useRelayer } from '@express402/relayer-react';

function TransactionButton() {
  const { submitTransaction, status, error } = useRelayer({
    apiUrl: 'http://localhost:8080',
    apiKey: 'your-api-key',
  });
  
  const handleSubmit = async () => {
    await submitTransaction({
      userAddress: '0x1234...',
      targetContract: '0xabcd...',
      calldata: '0x1234',
      // ... other fields
    });
  };
  
  return (
    <div>
      <button onClick={handleSubmit} disabled={status === 'processing'}>
        {status === 'processing' ? 'Processing...' : 'Submit Transaction'}
      </button>
      {error && <div className="error">{error}</div>}
      {status === 'confirmed' && <div>Transaction confirmed!</div>}
    </div>
  );
}
```

## 🎯 Use Cases & Examples

### Use Case 1: DEX Trading

```typescript
// Fast token swaps without waiting for confirmation
async function executeSwap(tokenIn: string, tokenOut: string, amount: BigNumber) {
  const swapCalldata = encodeSwapFunction(tokenIn, tokenOut, amount);
  
  const result = await relayer.submitTransaction({
    userAddress: userAddress,
    targetContract: dexRouterAddress,
    calldata: swapCalldata,
    priority: 'high', // Priority for time-sensitive trades
  });
  
  // Return immediately, transaction processing in background
  return result.transactionId;
}
```

### Use Case 2: NFT Minting

```typescript
// Batch NFT minting with priority queue
async function mintNFTs(quantity: number) {
  const promises = Array.from({ length: quantity }, (_, i) => 
    relayer.submitTransaction({
      userAddress: userAddress,
      targetContract: nftContractAddress,
      calldata: encodeMintFunction(i),
      priority: i === 0 ? 'urgent' : 'normal', // First mint is urgent
    })
  );
  
  return Promise.all(promises);
}
```

### Use Case 3: Gasless Transactions

```typescript
// Users submit transactions without paying gas
async function submitGaslessTransaction(transaction: TransactionRequest) {
  // User signs the transaction
  const signature = await signTransaction(transaction);
  
  // Relayer pays for gas and executes
  const result = await relayer.submitTransaction({
    ...transaction,
    signature,
    priority: 'normal',
  });
  
  return result;
}
```

## 🔍 Error Handling Guide

### Common Error Responses

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error message",
    "details": "Additional error details",
    "request_id": "unique-request-id"
  }
}
```

### Error Codes Reference

| Error Code | HTTP Status | Description | Solution |
|------------|-------------|-------------|----------|
| `INVALID_SIGNATURE` | 400 | Signature verification failed | Verify signature format and signer |
| `INSUFFICIENT_BALANCE` | 402 | User balance too low | Add funds to user account |
| `INVALID_NONCE` | 400 | Nonce already used or invalid | Use correct nonce value |
| `RATE_LIMITED` | 429 | Too many requests | Implement rate limiting |
| `WALLET_UNAVAILABLE` | 503 | No available wallets | Wait and retry |
| `NETWORK_ERROR` | 502 | Blockchain network error | Check RPC endpoint |
| `QUEUE_FULL` | 503 | Transaction queue is full | Retry later |
| `TIMEOUT` | 504 | Transaction timeout | Check network conditions |
| `INVALID_PARAMS` | 400 | Invalid transaction parameters | Verify all fields |

### Error Handling Best Practices

```typescript
async function submitWithRetry(transaction: TransactionRequest, maxRetries = 3) {
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      return await relayer.submitTransaction(transaction);
    } catch (error) {
      if (error.code === 'RATE_LIMITED') {
        // Exponential backoff
        await sleep(Math.pow(2, attempt) * 1000);
        continue;
      }
      
      if (error.code === 'WALLET_UNAVAILABLE' && attempt < maxRetries) {
        // Wait and retry
        await sleep(5000);
        continue;
      }
      
      // Non-retryable error
      throw error;
    }
  }
  
  throw new Error('Max retries exceeded');
}
```

## ⚙️ Advanced Features

### Priority Queue Management

Transactions are processed based on priority levels:

- **`urgent`**: Critical transactions processed immediately
- **`high`**: High-priority transactions processed next
- **`normal`**: Standard priority (default)
- **`low`**: Lower priority, processed when queue is available

Priority can be set dynamically based on transaction characteristics:

```typescript
const priority = determinePriority({
  value: transactionValue,
  gasPrice: gasPrice,
  timestamp: timestamp,
  userTier: userTier,
});
```

### Wallet Pool Rotation Strategies

The relayer supports multiple rotation strategies:

1. **Round Robin**: Even distribution across wallets
2. **Load Based**: Based on current wallet load
3. **Success Rate**: Prioritize wallets with higher success rates
4. **Balance Priority**: Prioritize wallets with higher balances

### Batch Processing

Multiple transactions can be submitted in a single batch:

```typescript
const batch = await relayer.submitBatch([
  transaction1,
  transaction2,
  transaction3,
]);

// Monitor batch status
const batchStatus = await relayer.getBatchStatus(batch.batchId);
```

### Transaction Callbacks

Register callbacks for transaction status updates:

```typescript
relayer.onTransactionStatusChange((status) => {
  console.log(`Transaction ${status.transactionId}: ${status.status}`);
  
  if (status.status === 'confirmed') {
    // Handle confirmation
    handleConfirmation(status);
  }
});
```

### WebSocket Subscriptions

Subscribe to real-time transaction updates:

```typescript
const subscription = await relayer.subscribe({
  transactionId: 'tx-id',
  onUpdate: (update) => {
    console.log('Status update:', update);
  },
  onError: (error) => {
    console.error('Subscription error:', error);
  },
});
```

### Rate Limiting & Throttling

The relayer implements multiple rate limiting strategies:

- **Per IP**: Limit requests per IP address
- **Per API Key**: Limit requests per API key
- **Per User**: Limit requests per user address
- **Global**: Global rate limit across all clients

Rate limit headers are included in responses:

```http
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1640995200
```

### Transaction Retry Logic

Automatic retry on transient failures:

- **Network Errors**: Retry with exponential backoff
- **Gas Price Issues**: Retry with adjusted gas price
- **Nonce Conflicts**: Retry after nonce synchronization
- **Wallet Failures**: Retry with different wallet

Retry configuration:

```typescript
const config = {
  maxRetries: 3,
  retryDelay: 5000, // milliseconds
  retryBackoff: 'exponential',
  retryableErrors: ['NETWORK_ERROR', 'TIMEOUT', 'WALLET_UNAVAILABLE'],
};
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

### Best Practices

#### Transaction Submission
1. **Use Appropriate Priority**: Set priority based on transaction importance
2. **Batch Transactions**: Group related transactions for better throughput
3. **Handle Errors Gracefully**: Implement retry logic with exponential backoff
4. **Monitor Status**: Use WebSocket subscriptions for real-time updates
5. **Set Reasonable Timeouts**: Configure timeouts based on network conditions

#### Wallet Management
1. **Balance Monitoring**: Regularly check wallet balances
2. **Nonce Management**: Keep track of nonces to avoid conflicts
3. **Gas Price Optimization**: Monitor gas prices and adjust multipliers
4. **Wallet Rotation**: Use multiple wallets for better throughput
5. **Health Checks**: Implement wallet health monitoring

#### Security Best Practices
1. **Private Key Security**: Store private keys securely (HSM recommended)
2. **Signature Validation**: Always verify signatures before processing
3. **Replay Protection**: Enable nonce and timestamp validation
4. **Rate Limiting**: Implement client-side rate limiting
5. **Input Validation**: Validate all transaction parameters

#### Performance Tuning
1. **Connection Pooling**: Optimize database and Redis connection pools
2. **Batch Processing**: Process transactions in batches
3. **Queue Management**: Monitor queue depth and adjust workers
4. **Caching**: Cache frequently accessed data
5. **Monitoring**: Set up comprehensive monitoring and alerting

## ❓ Frequently Asked Questions (FAQ)

### General Questions

**Q: What is the difference between Express402 Relayer and traditional relayers?**
A: Express402 Relayer uses a multi-wallet queue system for concurrent processing, enabling much higher throughput and lower latency compared to single-wallet relayers.

**Q: How fast can transactions be processed?**
A: With optimized configuration, the relayer can process 1,000+ transactions per second with sub-second latency.

**Q: Is the relayer production-ready?**
A: Yes, the relayer includes production features like monitoring, logging, error handling, and graceful shutdown.

### Technical Questions

**Q: How are wallet private keys stored?**
A: Private keys are stored securely in memory. For production, consider using Hardware Security Modules (HSM) or secure key management services.

**Q: What happens if a wallet runs out of balance?**
A: The relayer automatically monitors wallet balances and switches to available wallets. Low balance alerts are sent when thresholds are reached.

**Q: How are transaction conflicts handled?**
A: The relayer uses nonce management and replay protection to prevent conflicts. Transactions are queued and processed sequentially per wallet.

**Q: Can I use my own RPC endpoint?**
A: Yes, configure `EXPRESS402_ETHEREUM_RPC_URL` with your preferred RPC endpoint.

**Q: How does priority queue work?**
A: Transactions are ordered by priority (urgent > high > normal > low) and processed accordingly. Higher priority transactions are processed first.

### Integration Questions

**Q: Do you provide SDKs?**
A: Yes, SDKs are available for JavaScript/TypeScript, Python, and Rust. More languages coming soon.

**Q: How do I integrate with my dApp?**
A: Use the provided SDKs or make direct HTTP requests to the API endpoints. See the Integration Examples section.

**Q: Can I use this with MetaMask?**
A: Yes, transactions can be signed with MetaMask and submitted to the relayer. See the Web3 Integration example.

### Troubleshooting

**Q: Transactions are failing with "WALLET_UNAVAILABLE"**
A: Check wallet balances and ensure wallets have sufficient ETH. Verify wallet private keys are correct.

**Q: Getting "RATE_LIMITED" errors**
A: Implement rate limiting on your client side. Use exponential backoff for retries.

**Q: Database connection errors**
A: Verify PostgreSQL is running and connection string is correct. Check connection pool settings.

**Q: Redis connection errors**
A: Ensure Redis is running and accessible. Check network connectivity and Redis configuration.

## 📈 Roadmap

### Current Features
- ✅ Multi-wallet pool management
- ✅ Priority-based queue scheduling
- ✅ Transaction retry logic
- ✅ Rate limiting and throttling
- ✅ Real-time monitoring
- ✅ Docker deployment
- ✅ REST API
- ✅ WebSocket subscriptions

### Planned Features
- 🔄 WebSocket API for real-time updates
- 🔄 GraphQL API support
- 🔄 Additional SDK languages (Go, Java)
- 🔄 Advanced analytics dashboard
- 🔄 Multi-chain support (Polygon, Arbitrum, Optimism)
- 🔄 Transaction batching optimization
- 🔄 Gas price oracle integration
- 🔄 Decentralized wallet management

### Long-term Goals
- 🎯 Support for Layer 2 solutions
- 🎯 Cross-chain transaction relaying
- 🎯 Decentralized relayer network
- 🎯 Advanced MEV protection
- 🎯 Zero-knowledge proof integration

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
