# x402 Relayer - Relayer Service

A relayer architecture based on the x402 payment protocol that solves the problem of on-chain confirmation delays and significantly improves transaction processing speed through a multi-wallet queue scheduling mechanism.

## 🚀 Project Overview

The x402 Relayer is a middleware service designed to address the slow response time of on-chain transactions after a user signs on the front end. By maintaining a queue scheduling system with multiple wallets, it enables concurrent processing of transaction requests, significantly improving the user experience.

## 🏗️ Architecture Design

### Core Modules

- **API Gateway**: Request routing, authentication, rate limiting
- **Queue Scheduler**: Transaction queuing, priority scheduling, concurrency control
- **Wallet Pool Management**: Multi-wallet polling, balance monitoring, automatic switching
- **Security Verification**: Signature verification, replay attack prevention, prepaid balance check
- **Redis Cache**: Transaction status storage, queue persistence, session management

### Tech Stack

- **Frontend**: Next.js 16 + React 19 + TypeScript + Tailwind CSS
- **Backend**: Node.js + Express + TypeScript
- **Blockchain**: ethers.js + Web3.js
- **Cache**: Redis + Bull Queue
- **Security**: JWT + bcrypt + Multi-factor authentication

## 📋 Features

### ✅ Implemented Features

- [x] Multi-wallet pool management and scheduling
- [x] Transaction queue and concurrency control
- [x] Security verification and replay attack prevention
- [x] Prepaid mechanism and balance management
- [x] WebSocket real-time status push
- [x] RESTful API interface
- [x] Frontend user interface
- [x] SDK encapsulation

### 🔄 Core Workflow

1. **User Signature**: Frontend completes the transaction signature
2. **Request Submission**: Send to the relayer API
3. **Security Verification**: Signature verification, replay check, balance verification
4. **Queue Scheduling**: Add to the processing queue according to priority
5. **Wallet Selection**: Select an available wallet from the wallet pool
6. **Transaction Execution**: Build and send the blockchain transaction
7. **Status Update**: Push transaction status in real-time
8. **Confirmation**: Update the final status after the transaction is confirmed

## 🛠️ Installation and Running

### Environment Requirements

- Node.js 18+
- Redis 6+
- MetaMask Wallet (for testing)

### Install Dependencies

```bash
# Using pnpm (recommended)
pnpm install

# Or use npm
npm install

# Or use yarn
yarn install
```

### Environment Configuration

1. Copy the environment configuration file:
```bash
cp env.example .env
```

2. Edit the `.env` file and configure the necessary parameters:
```env
# Blockchain Network Configuration
RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
CHAIN_ID=1
PRIVATE_KEY_1=your_wallet_private_key_1
PRIVATE_KEY_2=your_wallet_private_key_2
PRIVATE_KEY_3=your_wallet_private_key_3

# Redis Configuration
REDIS_HOST=localhost
REDIS_PORT=6379
REDIS_PASSWORD=

# Server Configuration
PORT=3001
JWT_SECRET=your_jwt_secret_key
API_KEY=your_api_key
```

### Start the Service

```bash
# Development mode
pnpm dev

# Production mode
pnpm build
pnpm start
```

The service will run at the following addresses:
- Frontend UI: http://localhost:3000
- API Service: http://localhost:3001
- WebSocket: ws://localhost:3001

## 📚 API Documentation

### Core API

#### Submit Transaction
```http
POST /api/transaction/submit
Content-Type: application/json

{
  "from": "0x...",
  "to": "0x...",
  "amount": "0.001",
  "signature": "0x...",
  "message": "from:to:amount:timestamp",
  "nonce": "unique_nonce",
  "timestamp": 1234567890,
  "apiKey": "your_api_key",
  "clientId": "client_identifier"
}
```

#### Query Transaction Status
```http
GET /api/transaction/{id}/status
```

#### Get Queue Status
```http
GET /api/queue/status
```

#### Wallet Management
```http
GET /api/wallets
```

#### Prepaid Management
```http
POST /api/prepaid/add
GET /api/prepaid/{clientId}/balance
```

### WebSocket Events

#### Subscribe to Transaction Status
```javascript
ws.send(JSON.stringify({
  type: 'subscribe_transaction',
  transactionId: 'transaction_id'
}));
```

#### Receive Status Updates
```javascript
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if (data.type === 'transaction_update') {
    console.log('Transaction status update:', data.status);
  }
};
```

## 🔧 SDK Usage

### Install SDK

```bash
npm install @x402/relayer-sdk
```

### Basic Usage

```typescript
import { X402RelayerSDK, RelayerUtils } from '@x402/relayer-sdk';

// Initialize SDK
const relayer = new X402RelayerSDK({
  apiUrl: 'http://localhost:3001',
  apiKey: 'your-api-key',
  wsUrl: 'ws://localhost:3001'
});

// Connect to WebSocket
relayer.connectWebSocket();

// Listen for events
relayer.on('connected', () => {
  console.log('WebSocket connected successfully');
});

relayer.on('message', (data) => {
  console.log('Received message:', data);
});

// Submit transaction
const message = RelayerUtils.generateMessage(
  '0x...', '0x...', '0.001', Date.now()
);
const signature = await wallet.signMessage(message);

const result = await relayer.submitTransaction({
  from: '0x...',
  to: '0x...',
  amount: '0.001',
  signature,
  message,
  nonce: RelayerUtils.generateNonce(),
  timestamp: Date.now(),
  clientId: 'client_id'
});

if (result.success) {
  // Subscribe to transaction status
  relayer.subscribeToTransaction(result.transactionId);
}
```

## 🔒 Security Features

### Multi-Factor Authentication Mechanism

1. **API Key Verification**: Ensure the request source is legitimate
2. **Signature Verification**: Verify the validity of the user's signature
3. **Replay Attack Protection**: Nonce and timestamp-based anti-replay mechanism
4. **Rate Limiting**: Prevent malicious requests and DDoS attacks
5. **Prepaid Mechanism**: Ensure users have sufficient balance to pay for transaction fees

### Trust Model

- **Decentralized Verification**: All signature verifications are done on-chain
- **Transparent Operations**: All transaction statuses are queryable in real-time
- **Rollback Mechanism**: Supports state rollback on transaction failure
- **Balance Monitoring**: Real-time monitoring of wallet balances with automatic switching

## 🚀 Performance Optimization

### Concurrent Processing

- **Multi-Wallet Polling**: Supports concurrent transaction processing with multiple wallets
- **Queue Scheduling**: Intelligent scheduling algorithms to optimize processing order
- **Connection Pooling**: Redis connection pool to improve cache performance

### Monitoring Metrics

- Queue length and processing speed
- Wallet usage and balance status
- Transaction success rate and average confirmation time
- System resource usage

## 🤝 Compatibility with x402 Protocol

### Protocol Integration

- **Standard Interface**: Fully compatible with the x402 payment protocol
- **Signature Format**: Supports standard Ethereum signature format
- **Transaction Structure**: Maintains consistency with the native x402 transaction structure
- **State Synchronization**: Real-time synchronization of on-chain transaction status

### Extended Features

- **Batch Processing**: Supports batch transaction processing
- **Priority Queue**: Supports setting transaction priorities
- **Custom Gas**: Supports custom gas prices and limits

## 📊 Monitoring and Logging

### System Monitoring

- Real-time queue status monitoring
- Wallet pool health status check
- Transaction success rate statistics
- Performance metrics monitoring

### Logging

- Transaction processing logs
- Error and exception logs
- Security event logs
- Performance analysis logs

## 🔄 Deployment and Operations

### Docker Deployment

```dockerfile
FROM node:18-alpine
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
RUN npm run build
EXPOSE 3000 3001
CMD ["npm", "start"]
```

### Production Environment Configuration

- Use environment variables to manage sensitive configurations
- Configure a Redis cluster to improve availability
- Set up load balancing and a reverse proxy
- Configure monitoring and alerting systems

## 📝 Development Guide

### Project Structure

```
express402-relayer/
├── app/                 # Next.js frontend application
│   ├── page.tsx        # Main page
│   ├── layout.tsx      # Layout component
│   └── globals.css     # Global styles
├── lib/                # Core library files
│   ├── redis.ts        # Redis client
│   ├── wallet-pool.ts  # Wallet pool management
│   ├── transaction-queue.ts # Transaction queue
│   ├── security.ts     # Security verification
│   └── sdk.ts          # SDK encapsulation
├── server.ts           # Express server
├── docs/               # Documentation
└── package.json        # Project configuration
```

### Development Commands

```bash
# Development mode
pnpm dev

# Build project
pnpm build

# Lint code
pnpm lint

# Type check
pnpm type-check
```

## 🐛 Troubleshooting

### Common Issues

1. **Redis Connection Failure**: Check if the Redis service is running
2. **Insufficient Wallet Balance**: Ensure the wallet has enough ETH to pay for gas
3. **Signature Verification Failure**: Check the message format and signing algorithm
4. **WebSocket Disconnected**: Check the network connection and firewall settings

### Debug Mode

```bash
# Enable debug logs
DEBUG=x402-relayer:* pnpm dev
```

## 📄 License

MIT License

## 🤝 Contribution Guide

Issues and Pull Requests are welcome!

1. Fork the project
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

## 📞 Contact

- Project Address: https://github.com/your-org/express402-relayer
- Feedback: https://github.com/your-org/express402-relayer/issues
- Documentation: https://docs.x402-relayer.com

---

**Note**: This is a demo project. Please conduct a thorough security audit and testing before using it in a production environment.