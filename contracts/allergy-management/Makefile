.PHONY: help build test clean deploy-testnet deploy-mainnet optimize coverage lint format audit

# Default target
help:
	@echo "Allergy Management Smart Contract - Available Commands:"
	@echo ""
	@echo "Development:"
	@echo "  make build          - Build the contract"
	@echo "  make test           - Run all tests"
	@echo "  make coverage       - Generate test coverage report"
	@echo "  make lint           - Run clippy linter"
	@echo "  make format         - Format code with rustfmt"
	@echo "  make audit          - Run security audit"
	@echo ""
	@echo "Deployment:"
	@echo "  make optimize       - Optimize WASM binary"
	@echo "  make deploy-local   - Deploy to local network"
	@echo "  make deploy-testnet - Deploy to testnet"
	@echo "  make deploy-mainnet - Deploy to mainnet"
	@echo ""
	@echo "Utilities:"
	@echo "  make clean          - Clean build artifacts"
	@echo "  make check          - Run all checks (lint, test, audit)"
	@echo "  make watch          - Watch for changes and run tests"

# Build the contract
build:
	@echo "Building allergy-management contract..."
	cargo build --release --target wasm32-unknown-unknown
	@echo "Build complete!"

# Run tests
test:
	@echo "Running tests..."
	cargo test --verbose
	@echo "Tests complete!"

# Run tests with output
test-verbose:
	@echo "Running tests with output..."
	cargo test -- --nocapture
	@echo "Tests complete!"

# Generate test coverage
coverage:
	@echo "Generating coverage report..."
	@command -v cargo-tarpaulin >/dev/null 2>&1 || { echo "Installing cargo-tarpaulin..."; cargo install cargo-tarpaulin; }
	cargo tarpaulin --out Html --output-dir ./coverage --timeout 300
	@echo "Coverage report generated in ./coverage/index.html"

# Check coverage threshold
check-coverage:
	@echo "Checking coverage threshold (85%)..."
	@cargo tarpaulin --out Json | jq '.files | map(.coverage) | add / length' | \
		awk '{if ($$1 < 85) {print "Coverage below 85%: " $$1 "%"; exit 1} else {print "Coverage: " $$1 "%"}}'

# Run clippy linter
lint:
	@echo "Running clippy..."
	cargo clippy -- -D warnings
	@echo "Linting complete!"

# Format code
format:
	@echo "Formatting code..."
	cargo fmt --all
	@echo "Formatting complete!"

# Check formatting
format-check:
	@echo "Checking code formatting..."
	cargo fmt --all -- --check

# Run security audit
audit:
	@echo "Running security audit..."
	@command -v cargo-audit >/dev/null 2>&1 || { echo "Installing cargo-audit..."; cargo install cargo-audit; }
	cargo audit
	@echo "Security audit complete!"

# Optimize WASM binary
optimize: build
	@echo "Optimizing WASM binary..."
	@command -v soroban >/dev/null 2>&1 || { echo "Error: soroban CLI not found. Please install it first."; exit 1; }
	soroban contract optimize \
		--wasm ../../target/wasm32-unknown-unknown/release/allergy_management.wasm
	@ls -lh ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm
	@echo "Optimization complete!"

# Deploy to local network
deploy-local: optimize
	@echo "Deploying to local network..."
	@soroban network start local 2>/dev/null || echo "Local network already running"
	@CONTRACT_ID=$$(soroban contract deploy \
		--wasm ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm \
		--source test \
		--network local); \
	echo "Contract deployed: $$CONTRACT_ID"; \
	echo "$$CONTRACT_ID" > .contract-id-local; \
	soroban contract invoke \
		--id $$CONTRACT_ID \
		--source test \
		--network local \
		-- initialize \
		--admin $$(soroban keys address test)
	@echo "Deployment complete!"

# Deploy to testnet
deploy-testnet: optimize
	@echo "Deploying to testnet..."
	@if [ -z "$$TESTNET_SECRET_KEY" ]; then \
		echo "Error: TESTNET_SECRET_KEY environment variable not set"; \
		exit 1; \
	fi
	@CONTRACT_ID=$$(soroban contract deploy \
		--wasm ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm \
		--source admin \
		--network testnet); \
	echo "Contract deployed: $$CONTRACT_ID"; \
	echo "$$CONTRACT_ID" > .contract-id-testnet; \
	if [ -n "$$TESTNET_ADMIN_ADDRESS" ]; then \
		soroban contract invoke \
			--id $$CONTRACT_ID \
			--source admin \
			--network testnet \
			-- initialize \
			--admin $$TESTNET_ADMIN_ADDRESS; \
	fi
	@echo "Deployment complete!"

# Deploy to mainnet
deploy-mainnet: optimize
	@echo "WARNING: Deploying to MAINNET!"
	@echo "Press Ctrl+C to cancel, or wait 5 seconds to continue..."
	@sleep 5
	@if [ -z "$$MAINNET_SECRET_KEY" ]; then \
		echo "Error: MAINNET_SECRET_KEY environment variable not set"; \
		exit 1; \
	fi
	@CONTRACT_ID=$$(soroban contract deploy \
		--wasm ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm \
		--source admin \
		--network mainnet); \
	echo "Contract deployed: $$CONTRACT_ID"; \
	echo "$$CONTRACT_ID" > .contract-id-mainnet; \
	if [ -n "$$MAINNET_ADMIN_ADDRESS" ]; then \
		soroban contract invoke \
			--id $$CONTRACT_ID \
			--source admin \
			--network mainnet \
			-- initialize \
			--admin $$MAINNET_ADMIN_ADDRESS; \
	fi
	@echo "Deployment complete!"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -rf coverage/
	rm -f .contract-id-*
	@echo "Clean complete!"

# Run all checks
check: format-check lint test audit check-coverage
	@echo "All checks passed!"

# Watch for changes and run tests
watch:
	@echo "Watching for changes..."
	@command -v cargo-watch >/dev/null 2>&1 || { echo "Installing cargo-watch..."; cargo install cargo-watch; }
	cargo watch -x test

# Install development dependencies
install-deps:
	@echo "Installing development dependencies..."
	cargo install cargo-tarpaulin cargo-audit cargo-watch soroban-cli
	@echo "Dependencies installed!"

# Run benchmarks (if implemented)
bench:
	@echo "Running benchmarks..."
	cargo bench
	@echo "Benchmarks complete!"

# Generate documentation
docs:
	@echo "Generating documentation..."
	cargo doc --no-deps --open
	@echo "Documentation generated!"

# Verify contract
verify:
	@echo "Verifying contract..."
	@if [ ! -f .contract-id-testnet ]; then \
		echo "Error: No testnet deployment found"; \
		exit 1; \
	fi
	@CONTRACT_ID=$$(cat .contract-id-testnet); \
	soroban contract invoke \
		--id $$CONTRACT_ID \
		--network testnet \
		-- --help
	@echo "Contract verified!"
