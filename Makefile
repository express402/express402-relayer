# Express402 Relayer Makefile

.PHONY: help build test run clean docker-build docker-run dev-setup

# Default target
help:
	@echo "Express402 Relayer - Available commands:"
	@echo "  build         - Build the project"
	@echo "  test          - Run tests"
	@echo "  run           - Run the application"
	@echo "  clean         - Clean build artifacts"
	@echo "  docker-build  - Build Docker image"
	@echo "  docker-run    - Run with Docker Compose"
	@echo "  dev-setup     - Setup development environment"
	@echo "  fmt           - Format code"
	@echo "  clippy        - Run clippy linter"
	@echo "  check         - Check code without building"

# Build the project
build:
	cargo build --release

# Run tests
test:
	cargo test

# Run the application
run:
	cargo run

# Clean build artifacts
clean:
	cargo clean

# Format code
fmt:
	cargo fmt

# Run clippy linter
clippy:
	cargo clippy -- -D warnings

# Check code without building
check:
	cargo check

# Build Docker image
docker-build:
	docker build -t express402-relayer .

# Run with Docker Compose
docker-run:
	docker-compose up -d

# Stop Docker Compose
docker-stop:
	docker-compose down

# View Docker logs
docker-logs:
	docker-compose logs -f

# Setup development environment
dev-setup:
	@echo "Setting up development environment..."
	@if [ ! -f .env ]; then cp config.development.env .env; fi
	@echo "Created .env file from development config"
	@echo "Please edit .env file with your configuration"

# Database operations
db-migrate:
	@echo "Running database migrations..."
	@echo "Make sure PostgreSQL is running and configured in .env"

# Install dependencies
install:
	cargo build

# Run in development mode with hot reload
dev:
	RUST_LOG=debug cargo run

# Run benchmarks
bench:
	cargo bench

# Generate documentation
docs:
	cargo doc --open

# Security audit
audit:
	cargo audit

# Update dependencies
update:
	cargo update
