# Deployment Guide

This guide provides step-by-step instructions for deploying the Allergy Management smart contract to various Stellar networks.

## Prerequisites

### Required Tools

1. **Rust** (1.74.0 or later)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
```

2. **Soroban CLI**
```bash
cargo install --locked soroban-cli --features opt
```

3. **Stellar CLI** (optional)
```bash
cargo install --locked stellar-cli
```

### Environment Setup

Create a `.env` file in the project root:

```bash
# Testnet Configuration
TESTNET_SECRET_KEY=your_testnet_secret_key
TESTNET_ADMIN_ADDRESS=your_testnet_admin_address
TESTNET_RPC_URL=https://soroban-testnet.stellar.org

# Mainnet Configuration (for production)
MAINNET_SECRET_KEY=your_mainnet_secret_key
MAINNET_ADMIN_ADDRESS=your_mainnet_admin_address
MAINNET_RPC_URL=https://soroban-mainnet.stellar.org
```

## Build Process

### 1. Development Build

```bash
cd contracts/contracts/allergy-management
cargo build
```

### 2. Production Build

```bash
# Build for WASM target
cargo build --release --target wasm32-unknown-unknown

# Optimize the WASM binary
soroban contract optimize \
  --wasm ../../target/wasm32-unknown-unknown/release/allergy_management.wasm
```

The optimized WASM will be at:
```
../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm
```

### 3. Using Makefile

```bash
# Build and optimize in one command
make optimize

# Run all checks before deployment
make check
```

## Local Development Network

### Start Local Network

```bash
# Start Stellar local network
soroban network start local

# Or using Docker
docker run --rm -it \
  -p 8000:8000 \
  --name stellar \
  stellar/quickstart:testing \
  --local \
  --enable-soroban-rpc
```

### Deploy Locally

```bash
# Using Makefile
make deploy-local

# Or manually
CONTRACT_ID=$(soroban contract deploy \
  --wasm ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm \
  --source test \
  --network local)

echo "Contract ID: $CONTRACT_ID"

# Initialize the contract
soroban contract invoke \
  --id $CONTRACT_ID \
  --source test \
  --network local \
  -- initialize \
  --admin $(soroban keys address test)
```

### Test Locally

```bash
# Generate test addresses
PATIENT=$(soroban keys generate patient --network local)
PROVIDER=$(soroban keys generate provider --network local)

# Grant access
soroban contract invoke \
  --id $CONTRACT_ID \
  --source patient \
  --network local \
  -- grant_access \
  --patient_id $PATIENT \
  --provider_id $PROVIDER

# Record an allergy
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network local \
  -- record_allergy \
  --patient_id $PATIENT \
  --provider_id $PROVIDER \
  --allergen "Penicillin" \
  --allergen_type "med" \
  --reaction_type '["rash", "hives"]' \
  --severity "moderate" \
  --onset_date 1234567890 \
  --verified true
```

## Testnet Deployment

### Configure Testnet

```bash
# Add testnet network
soroban network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"

# Create or import admin key
soroban keys generate admin --network testnet
# Or import existing key
soroban keys add admin --secret-key <YOUR_SECRET_KEY>

# Fund the account (get testnet XLM)
curl "https://friendbot.stellar.org?addr=$(soroban keys address admin)"
```

### Deploy to Testnet

```bash
# Using Makefile
export TESTNET_SECRET_KEY=$(soroban keys show admin)
export TESTNET_ADMIN_ADDRESS=$(soroban keys address admin)
make deploy-testnet

# Or manually
CONTRACT_ID=$(soroban contract deploy \
  --wasm ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm \
  --source admin \
  --network testnet)

echo "Testnet Contract ID: $CONTRACT_ID"
echo $CONTRACT_ID > .contract-id-testnet

# Initialize
soroban contract invoke \
  --id $CONTRACT_ID \
  --source admin \
  --network testnet \
  -- initialize \
  --admin $(soroban keys address admin)
```

### Verify Testnet Deployment

```bash
# Check contract exists
soroban contract invoke \
  --id $(cat .contract-id-testnet) \
  --network testnet \
  -- --help

# Test basic functionality
PATIENT=$(soroban keys address patient)
PROVIDER=$(soroban keys address provider)

soroban contract invoke \
  --id $(cat .contract-id-testnet) \
  --source patient \
  --network testnet \
  -- grant_access \
  --patient_id $PATIENT \
  --provider_id $PROVIDER
```

## Mainnet Deployment

### Pre-Deployment Checklist

- [ ] All tests passing (>85% coverage)
- [ ] Security audit completed
- [ ] Testnet deployment successful
- [ ] Integration tests passed
- [ ] Documentation updated
- [ ] Admin keys secured (hardware wallet recommended)
- [ ] Backup and recovery plan in place
- [ ] Monitoring and alerting configured

### Configure Mainnet

```bash
# Add mainnet network
soroban network add mainnet \
  --rpc-url https://soroban-mainnet.stellar.org \
  --network-passphrase "Public Global Stellar Network ; September 2015"

# Import admin key (use hardware wallet for production)
soroban keys add admin --secret-key <YOUR_MAINNET_SECRET_KEY>

# Verify sufficient XLM balance
soroban keys address admin
# Check balance at: https://stellar.expert/explorer/public/account/<ADDRESS>
```

### Deploy to Mainnet

```bash
# Final verification
make check

# Deploy (with confirmation prompt)
export MAINNET_SECRET_KEY=$(soroban keys show admin)
export MAINNET_ADMIN_ADDRESS=$(soroban keys address admin)
make deploy-mainnet

# Or manually
CONTRACT_ID=$(soroban contract deploy \
  --wasm ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm \
  --source admin \
  --network mainnet)

echo "Mainnet Contract ID: $CONTRACT_ID"
echo $CONTRACT_ID > .contract-id-mainnet

# Initialize
soroban contract invoke \
  --id $CONTRACT_ID \
  --source admin \
  --network mainnet \
  -- initialize \
  --admin $(soroban keys address admin)
```

### Post-Deployment

1. **Verify Deployment**
```bash
soroban contract invoke \
  --id $(cat .contract-id-mainnet) \
  --network mainnet \
  -- --help
```

2. **Document Deployment**
```bash
cat > deployment-record.txt << EOF
Deployment Record
=================
Date: $(date)
Network: Mainnet
Contract ID: $(cat .contract-id-mainnet)
Admin Address: $(soroban keys address admin)
Git Commit: $(git rev-parse HEAD)
WASM Hash: $(sha256sum ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm)
EOF
```

3. **Set Up Monitoring**
- Configure event monitoring
- Set up alerting for suspicious activity
- Monitor contract invocations
- Track gas usage

4. **Backup**
- Store contract ID securely
- Backup admin keys (hardware wallet + paper backup)
- Document recovery procedures

## Upgrade Process

### Preparing an Upgrade

1. **Test Thoroughly**
```bash
# Run full test suite
cargo test -p allergy-management

# Run integration tests
make integration-test

# Security audit
make audit
```

2. **Deploy New Version to Testnet**
```bash
make deploy-testnet
# Test new version thoroughly
```

3. **Plan Migration**
- Document breaking changes
- Prepare data migration scripts
- Notify users of upgrade schedule

### Executing Upgrade

```bash
# Deploy new contract version
NEW_CONTRACT_ID=$(soroban contract deploy \
  --wasm ../../target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm \
  --source admin \
  --network mainnet)

# Migrate data if necessary
# Update client applications with new contract ID
# Monitor for issues
```

## Troubleshooting

### Common Issues

1. **Insufficient Balance**
```bash
# Check balance
stellar account --account $(soroban keys address admin)

# Fund account (testnet)
curl "https://friendbot.stellar.org?addr=$(soroban keys address admin)"
```

2. **Network Connection Issues**
```bash
# Test RPC connection
curl -X POST https://soroban-testnet.stellar.org \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
```

3. **Contract Already Exists**
```bash
# Check if contract is already deployed
soroban contract invoke --id <CONTRACT_ID> --network testnet -- --help
```

4. **Authorization Errors**
```bash
# Verify key is correct
soroban keys show admin
soroban keys address admin

# Check account exists and is funded
stellar account --account $(soroban keys address admin)
```

### Getting Help

- GitHub Issues: [Your Repo URL]
- Discord: [Your Discord]
- Email: support@[your-domain].com

## Cost Estimation

### Testnet
- Deployment: Free (testnet XLM from friendbot)
- Operations: Free

### Mainnet
- Deployment: ~0.5 XLM (varies)
- Contract initialization: ~0.1 XLM
- Per operation: ~0.00001 XLM
- Storage: ~0.0001 XLM per entry

### Optimization Tips
- Batch operations when possible
- Use efficient data structures
- Minimize storage writes
- Optimize WASM size

## Security Considerations

### Key Management
- Use hardware wallets for mainnet admin keys
- Never commit private keys to version control
- Rotate keys regularly
- Use multi-signature for critical operations

### Access Control
- Limit admin privileges
- Implement time-locks for critical operations
- Monitor admin actions
- Have incident response plan

### Monitoring
- Set up event monitoring
- Alert on unusual activity
- Track contract metrics
- Regular security audits

## Rollback Plan

In case of critical issues:

1. **Immediate Response**
   - Pause contract operations (if implemented)
   - Notify users
   - Assess impact

2. **Rollback**
   - Deploy previous version
   - Update client applications
   - Restore data if necessary

3. **Post-Mortem**
   - Document incident
   - Identify root cause
   - Implement fixes
   - Update procedures

---

For more information, see:
- [README.md](README.md) - General documentation
- [SECURITY.md](SECURITY.md) - Security policies
- [Soroban Documentation](https://soroban.stellar.org/docs)
