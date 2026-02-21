# Allergy Management Smart Contract

A comprehensive, production-ready smart contract for managing patient allergies on the Stellar blockchain using Soroban.

## Overview

This contract provides a secure, HIPAA-compliant system for recording, tracking, and managing patient allergies with severity assessment, drug interaction checking, and cross-sensitivity detection.

## Features

### Core Functionality

- **Allergy Recording**: Record patient allergies with detailed information including allergen type, reaction types, and severity
- **Severity Tracking**: Update allergy severity with full audit trail
- **Allergy Resolution**: Mark allergies as resolved with documentation
- **Drug Interaction Checking**: Check for potential drug-allergy interactions before prescribing
- **Access Control**: Patient-controlled access to allergy information
- **Historical Tracking**: Complete audit trail of all allergy-related changes

### Allergen Types

- `med` - Medication allergies
- `food` - Food allergies
- `env` - Environmental allergies

### Severity Levels

- `mild` - Minor reactions (e.g., mild rash)
- `moderate` - Moderate reactions requiring treatment
- `severe` - Serious reactions requiring immediate medical attention
- `critical` - Life-threatening reactions (e.g., anaphylaxis)

## Architecture

### Data Structures

#### AllergyRecord
```rust
pub struct AllergyRecord {
    pub allergy_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub allergen: String,
    pub allergen_type: Symbol,
    pub reaction_type: Vec<String>,
    pub severity: Symbol,
    pub onset_date: Option<u64>,
    pub recorded_date: u64,
    pub verified: bool,
    pub status: AllergyStatus,
    pub resolution_date: Option<u64>,
    pub resolution_reason: Option<String>,
    pub severity_history: Vec<SeverityUpdate>,
}
```

#### AllergyInteraction
```rust
pub struct AllergyInteraction {
    pub allergy_id: u64,
    pub allergen: String,
    pub severity: Symbol,
    pub reaction_type: Vec<String>,
    pub interaction_type: Symbol,
}
```

## API Reference

### Administrative Functions

#### `initialize`
Initialize the contract with an admin address.

```rust
pub fn initialize(env: Env, admin: Address)
```

### Core Functions

#### `record_allergy`
Record a new allergy for a patient.

```rust
pub fn record_allergy(
    env: Env,
    patient_id: Address,
    provider_id: Address,
    allergen: String,
    allergen_type: Symbol,
    reaction_type: Vec<String>,
    severity: Symbol,
    onset_date: Option<u64>,
    verified: bool,
) -> Result<u64, Error>
```

**Returns**: Unique allergy ID

**Errors**:
- `InvalidAllergenType` - Invalid allergen type provided
- `InvalidSeverity` - Invalid severity level
- `DuplicateAllergy` - Allergy already exists for this patient

#### `update_allergy_severity`
Update the severity level of an existing allergy.

```rust
pub fn update_allergy_severity(
    env: Env,
    allergy_id: u64,
    provider_id: Address,
    new_severity: Symbol,
    reason: String,
) -> Result<(), Error>
```

**Errors**:
- `AllergyNotFound` - Allergy ID doesn't exist
- `AlreadyResolved` - Cannot update resolved allergy
- `InvalidSeverity` - Invalid severity level

#### `resolve_allergy`
Mark an allergy as resolved.

```rust
pub fn resolve_allergy(
    env: Env,
    allergy_id: u64,
    provider_id: Address,
    resolution_date: u64,
    resolution_reason: String,
) -> Result<(), Error>
```

**Errors**:
- `AllergyNotFound` - Allergy ID doesn't exist
- `AlreadyResolved` - Allergy already resolved
- `InvalidDate` - Resolution date is in the future

#### `check_drug_allergy_interaction`
Check for potential drug-allergy interactions.

```rust
pub fn check_drug_allergy_interaction(
    env: Env,
    patient_id: Address,
    drug_name: String,
) -> Result<Vec<AllergyInteraction>, Error>
```

**Returns**: List of potential interactions with severity and reaction information

#### `get_active_allergies`
Retrieve all active allergies for a patient.

```rust
pub fn get_active_allergies(
    env: Env,
    patient_id: Address,
    requester: Address,
) -> Result<Vec<AllergyRecord>, Error>
```

**Errors**:
- `AccessDenied` - Requester doesn't have permission

#### `get_all_allergies`
Retrieve all allergies (active and resolved) for a patient.

```rust
pub fn get_all_allergies(
    env: Env,
    patient_id: Address,
    requester: Address,
) -> Result<Vec<AllergyRecord>, Error>
```

### Access Control Functions

#### `grant_access`
Grant a provider access to view patient allergies.

```rust
pub fn grant_access(env: Env, patient_id: Address, provider_id: Address)
```

#### `revoke_access`
Revoke a provider's access to patient allergies.

```rust
pub fn revoke_access(env: Env, patient_id: Address, provider_id: Address)
```

## Security Features

### Authentication & Authorization
- All write operations require authentication via `require_auth()`
- Patient-controlled access grants for viewing allergy data
- Provider verification for recording and updating allergies
- Admin role for system-level operations

### Data Privacy
- Access control enforced at the contract level
- Patients always have access to their own data
- Explicit access grants required for providers
- Audit trail for all modifications

### Input Validation
- Allergen type validation (med, food, env)
- Severity level validation (mild, moderate, severe, critical)
- Date validation (no future dates for resolution)
- Duplicate allergy prevention

## Usage Examples

### Recording an Allergy

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source provider \
  -- record_allergy \
  --patient_id <PATIENT_ADDRESS> \
  --provider_id <PROVIDER_ADDRESS> \
  --allergen "Penicillin" \
  --allergen_type "med" \
  --reaction_type '["rash", "hives"]' \
  --severity "moderate" \
  --onset_date 1234567890 \
  --verified true
```

### Checking Drug Interactions

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  -- check_drug_allergy_interaction \
  --patient_id <PATIENT_ADDRESS> \
  --drug_name "Amoxicillin"
```

### Updating Severity

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source provider \
  -- update_allergy_severity \
  --allergy_id 0 \
  --provider_id <PROVIDER_ADDRESS> \
  --new_severity "severe" \
  --reason "Patient experienced anaphylactic reaction"
```

## Testing

The contract includes comprehensive test coverage (>85%) covering:

- Allergy recording and retrieval
- Severity updates with history tracking
- Allergy resolution
- Drug interaction checking
- Access control and permissions
- Duplicate prevention
- Input validation
- Edge cases and error conditions

### Running Tests

```bash
# Run all tests
cargo test -p allergy-management

# Run with output
cargo test -p allergy-management -- --nocapture

# Run specific test
cargo test -p allergy-management test_record_allergy
```

### Test Coverage

```bash
# Install tarpaulin for coverage
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin -p allergy-management --out Html
```

## Building

### Development Build

```bash
cargo build -p allergy-management
```

### Optimized Build

```bash
cargo build -p allergy-management --release --target wasm32-unknown-unknown
```

### Optimize WASM

```bash
soroban contract optimize \
  --wasm target/wasm32-unknown-unknown/release/allergy_management.wasm
```

## Deployment

### Testnet Deployment

```bash
# Build optimized contract
cargo build -p allergy-management --release --target wasm32-unknown-unknown

# Deploy to testnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/allergy_management.wasm \
  --source admin \
  --network testnet

# Initialize contract
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS>
```

### Mainnet Deployment

```bash
# Build and optimize
cargo build -p allergy-management --release --target wasm32-unknown-unknown
soroban contract optimize \
  --wasm target/wasm32-unknown-unknown/release/allergy_management.wasm

# Deploy to mainnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/allergy_management.optimized.wasm \
  --source admin \
  --network mainnet

# Initialize
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network mainnet \
  -- initialize \
  --admin <ADMIN_ADDRESS>
```

## CI/CD Integration

The contract includes GitHub Actions workflows for:

- Automated testing on push/PR
- Code coverage reporting
- Security audits
- Optimized builds
- Deployment automation

See `.github/workflows/allergy-management.yml` for details.

## Performance Considerations

- Efficient storage using Soroban's persistent storage
- Indexed patient allergy lookups
- Minimal on-chain computation for interaction checks
- Optimized WASM binary size

## Future Enhancements

- Cross-sensitivity database expansion
- Machine learning-based interaction prediction
- Integration with external drug databases
- Multi-language support for allergen names
- Genetic marker tracking for allergy predisposition

## License

[Your License Here]

## Support

For issues, questions, or contributions, please contact [Your Contact Info].
