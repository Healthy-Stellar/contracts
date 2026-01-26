# contracts 
# Decentralized Healthcare System on Stellar

A blockchain-based healthcare management system built with Soroban smart contracts on the Stellar network, enabling secure, transparent, and efficient healthcare data management.

## Overview

This decentralized healthcare system leverages Stellar's Soroban smart contracts to provide a trustless, HIPAA-compliant solution for managing electronic health records (EHR), patient data, medical appointments, and healthcare provider interactions. The system ensures data privacy, interoperability, and patient sovereignty over personal health information.

## Key Features

- **Patient Data Sovereignty**: Patients maintain complete control over their health records with cryptographic access management
- **Secure Health Records**: Encrypted storage of medical records with granular permission controls
- **Provider Verification**: Blockchain-based credential verification for healthcare providers
- **Appointment Management**: Decentralized scheduling and management of medical appointments
- **Medical History Tracking**: Immutable audit trail of all medical interactions and treatments
- **Insurance Integration**: Smart contract-based claims processing and verification
- **Prescription Management**: Secure digital prescription issuance and tracking
- **Consent Management**: Patient-controlled data sharing with healthcare providers and institutions

## Architecture

### Smart Contracts

The system consists of multiple interconnected Soroban smart contracts:

- `patient_registry.rs` - Patient identity and profile management
- `provider_registry.rs` - Healthcare provider credentials and verification
- `health_records.rs` - Electronic health record storage and access control
- `appointments.rs` - Appointment scheduling and management
- `prescriptions.rs` - Digital prescription issuance and pharmacy verification
- `insurance_claims.rs` - Automated claims processing and settlement
- `consent_manager.rs` - Patient consent and data sharing permissions

### Technology Stack

- **Blockchain**: Stellar Network
- **Smart Contracts**: Soroban (Rust-based)
- **Development Framework**: Soroban SDK
- **Testing**: Soroban Test Framework
- **Deployment**: Stellar CLI

## Prerequisites

Before you begin, ensure you have the following installed:

- Rust (1.74.0 or later)
- Soroban CLI
- Stellar CLI
- Node.js (18.x or later) - for frontend integration
- Docker (optional, for local Stellar network)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Soroban CLI
cargo install --locked soroban-cli

# Install Stellar CLI
cargo install --locked stellar-cli
```

## Installation

1. Clone the repository:
```bash
git clone https://github.com/yourusername/stellar-healthcare-system.git
cd stellar-healthcare-system
```

2. Install dependencies:
```bash
cargo build
```

3. Configure your environment:
```bash
cp .env.example .env
# Edit .env with your configuration
```

## Configuration

Create a `.env` file in the root directory:

```env
STELLAR_NETWORK=testnet
SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
ADMIN_SECRET_KEY=your_secret_key_here
CONTRACT_WASM_HASH=your_contract_hash
```

## Deployment

### Local Development Network

1. Start a local Stellar network:
```bash
stellar network start local
```

2. Build the smart contracts:
```bash
soroban contract build
```

3. Deploy to local network:
```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/patient_registry.wasm \
  --source admin \
  --network local
```

### Testnet Deployment

1. Build optimized contracts:
```bash
soroban contract optimize --wasm target/wasm32-unknown-unknown/release/patient_registry.wasm
```

2. Deploy to Stellar Testnet:
```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/patient_registry.optimized.wasm \
  --source admin \
  --network testnet
```

3. Initialize contracts:
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS>
```

## Usage

### Patient Registration

```bash
soroban contract invoke \
  --id <PATIENT_REGISTRY_CONTRACT> \
  --source patient \
  --network testnet \
  -- register_patient \
  --patient_id "P12345" \
  --name "John Doe" \
  --dob "1990-01-01" \
  --encrypted_data <ENCRYPTED_HEALTH_DATA>
```

### Provider Verification

```bash
soroban contract invoke \
  --id <PROVIDER_REGISTRY_CONTRACT> \
  --source provider \
  --network testnet \
  -- register_provider \
  --provider_id "DR001" \
  --name "Dr. Jane Smith" \
  --specialty "Cardiology" \
  --credentials <CREDENTIAL_HASH>
```

### Hospital Configuration

```bash
soroban contract invoke \
  --id <HOSPITAL_REGISTRY_CONTRACT> \
  --source hospital \
  --network testnet \
  -- register_hospital \
  --wallet <HOSPITAL_WALLET> \
  --name "Regional Medical Center" \
  --location "789 Pine Rd" \
  --metadata "Accredited, trauma level II"
```

```bash
soroban contract invoke \
  --id <HOSPITAL_REGISTRY_CONTRACT> \
  --source hospital \
  --network testnet \
  -- set_hospital_config \
  --wallet <HOSPITAL_WALLET> \
  --config <CONFIG_XDR>
```

### Grant Data Access

```bash
soroban contract invoke \
  --id <CONSENT_MANAGER_CONTRACT> \
  --source patient \
  --network testnet \
  -- grant_access \
  --patient_id "P12345" \
  --provider_id "DR001" \
  --duration_days 30 \
  --access_level "read"
```

## Testing

Run the complete test suite:

```bash
cargo test
```

Run specific test modules:

```bash
cargo test patient_registry
cargo test health_records
```

Run integration tests:

```bash
cargo test --test integration_tests
```
