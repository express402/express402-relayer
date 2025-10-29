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
- **Cache**: Transaction status storage, queue persistence, session management

## 🛠️ Installation and Running

### Environment Requirements

- Rust 1.60+

### Install Dependencies

```bash
cargo build
```

### Run Tests

```bash
cargo test
```

## 🤝 Contribution Guide

Issues and Pull Requests are welcome!

1. Fork the project
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

## 📄 License

MIT License
