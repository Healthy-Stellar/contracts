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

The system is organised as a Cargo workspace. Each contract lives in its own crate under `contracts/`:

| Crate | Description |
|---|---|
| `contracts/patient-registry` | Patient identity and profile management |
| `contracts/provider-registry` | Healthcare provider credentials and verification |
| `contracts/health-records` | Electronic health record storage and access control |
| `contracts/doctor-registry` | Doctor registration and credential management |
| `contracts/hospital-registry` | Hospital registration and configuration |
| `contracts/insurer-registry` | Insurance provider registry |
| `contracts/prescription-management` | Digital prescription issuance and pharmacy verification |
| `contracts/medical-claims` | Automated claims processing and settlement |
| `contracts/access-control` | Patient consent and data-sharing permissions |
| `contracts/allergy-management` | Allergy record management |
| `contracts/allergy-tracking` | Real-time allergy tracking and alerting |
| `contracts/care-plan` | Patient care-plan management |
| `contracts/clinical-guideline` | Clinical guideline publication |
| `contracts/clinical-trial` | Clinical trial enrollment and tracking |
| `contracts/dental-records` | Dental record storage |
| `contracts/emergency-medical-info` | Emergency medical information registry |
| `contracts/financial-records` | Patient financial and billing records |
| `contracts/hai-tracking` | Hospital-acquired infection tracking |
| `contracts/healthcare-analytics` | On-chain analytics aggregation |
| `contracts/healthcare-credentialing` | Provider credentialing workflows |
| `contracts/hospital-discharge-management` | Discharge planning and management |
| `contracts/imaging-radiology` | Medical imaging and radiology record management |
| `contracts/immunization-registry` | Immunization records |
| `contracts/lab-management` | Lab test ordering and result management |
| `contracts/medical-device-tracking` | Medical device inventory and tracking |
| `contracts/mental-health` | Mental health record management |
| `contracts/multisig-governance` | Multi-signature governance for admin actions |
| `contracts/nutrition-care-management` | Nutrition and dietary care management |
| `contracts/pacs-integration` | PACS system integration |
| `contracts/patient-vitals` | Patient vital-signs tracking |
| `contracts/prenatal-pediatric` | Prenatal and pediatric care records |
| `contracts/prior-authorization` | Insurance prior-authorization workflows |
| `contracts/referral` | Patient referral management |
| `contracts/rehabilitation-services` | Rehabilitation program tracking |
| `contracts/telemedicine` | Telemedicine session management |
| `contracts/upgrade-governance` | Contract upgrade governance |
| `contracts/zk-eligibility-verifier` | Zero-knowledge eligibility verification |

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
git clone https://github.com/KingFRANKHOOD/contracts.git
cd contracts
```

Or via SSH:
```bash
git clone git@github.com:KingFRANKHOOD/contracts.git
cd contracts
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

## Upgrade Guide

A step-by-step upgrade workflow is available in `docs/upgrade-guide.md`, including schema migration, multi-sig governance submission, rollback procedures, and testnet dry run examples.

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
