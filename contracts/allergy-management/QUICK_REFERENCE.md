# Quick Reference Card

## Contract Functions

### Administrative

```rust
initialize(env: Env, admin: Address)
```
Initialize contract with admin address.

---

### Core Operations

#### Record Allergy
```rust
record_allergy(
    env: Env,
    patient_id: Address,
    provider_id: Address,
    allergen: String,
    allergen_type: Symbol,      // "med", "food", "env"
    reaction_type: Vec<String>,
    severity: Symbol,            // "mild", "moderate", "severe", "critical"
    onset_date: Option<u64>,
    verified: bool
) -> Result<u64, Error>
```

#### Update Severity
```rust
update_allergy_severity(
    env: Env,
    allergy_id: u64,
    provider_id: Address,
    new_severity: Symbol,
    reason: String
) -> Result<(), Error>
```

#### Resolve Allergy
```rust
resolve_allergy(
    env: Env,
    allergy_id: u64,
    provider_id: Address,
    resolution_date: u64,
    resolution_reason: String
) -> Result<(), Error>
```

#### Check Interactions
```rust
check_drug_allergy_interaction(
    env: Env,
    patient_id: Address,
    drug_name: String
) -> Result<Vec<AllergyInteraction>, Error>
```

#### Get Active Allergies
```rust
get_active_allergies(
    env: Env,
    patient_id: Address,
    requester: Address
) -> Result<Vec<AllergyRecord>, Error>
```

---

### Access Control

```rust
grant_access(env: Env, patient_id: Address, provider_id: Address)
revoke_access(env: Env, patient_id: Address, provider_id: Address)
```

---

## Valid Values

### Allergen Types
- `med` - Medication
- `food` - Food
- `env` - Environmental

### Severity Levels
- `mild` - Minor reactions
- `moderate` - Moderate reactions
- `severe` - Serious reactions
- `critical` - Life-threatening

### Allergy Status
- `Active` - Currently active
- `Resolved` - No longer active

---

## Error Codes

| Code | Error | Description |
|------|-------|-------------|
| 1 | AllergyNotFound | Allergy ID doesn't exist |
| 2 | Unauthorized | Caller not authorized |
| 3 | InvalidSeverity | Invalid severity level |
| 4 | InvalidAllergenType | Invalid allergen type |
| 5 | AlreadyResolved | Allergy already resolved |
| 6 | InvalidDate | Invalid date provided |
| 7 | DuplicateAllergy | Allergy already exists |
| 8 | AccessDenied | No permission to access |

---

## CLI Commands

### Deploy
```bash
make deploy-testnet
make deploy-mainnet
```

### Test
```bash
make test
make coverage
```

### Build
```bash
make build
make optimize
```

### Audit
```bash
make audit
make check
```

---

## Common Workflows

### 1. Record New Allergy
```bash
# Grant access
soroban contract invoke --id $ID --source patient \
  -- grant_access --patient_id $PATIENT --provider_id $PROVIDER

# Record allergy
soroban contract invoke --id $ID --source provider \
  -- record_allergy \
  --patient_id $PATIENT \
  --provider_id $PROVIDER \
  --allergen "Penicillin" \
  --allergen_type "med" \
  --reaction_type '["rash"]' \
  --severity "moderate" \
  --verified true
```

### 2. Check Drug Safety
```bash
soroban contract invoke --id $ID \
  -- check_drug_allergy_interaction \
  --patient_id $PATIENT \
  --drug_name "Amoxicillin"
```

### 3. Update Severity
```bash
soroban contract invoke --id $ID --source provider \
  -- update_allergy_severity \
  --allergy_id 0 \
  --provider_id $PROVIDER \
  --new_severity "severe" \
  --reason "Worsening symptoms"
```

### 4. View Allergies
```bash
soroban contract invoke --id $ID --source provider \
  -- get_active_allergies \
  --patient_id $PATIENT \
  --requester $PROVIDER
```

---

## Environment Variables

```bash
# Testnet
export TESTNET_SECRET_KEY=<key>
export TESTNET_ADMIN_ADDRESS=<address>

# Mainnet
export MAINNET_SECRET_KEY=<key>
export MAINNET_ADMIN_ADDRESS=<address>
```

---

## File Structure

```
allergy-management/
├── src/
│   ├── lib.rs          # Main contract
│   ├── types.rs        # Data structures
│   ├── storage.rs      # Storage ops
│   ├── validation.rs   # Validation
│   └── test.rs         # Tests
├── Cargo.toml
├── Makefile
├── README.md
├── DEPLOYMENT.md
├── EXAMPLES.md
├── SECURITY.md
└── QUICK_REFERENCE.md  # This file
```

---

## Key Features

✅ Multiple allergen types  
✅ Severity classification  
✅ Drug interaction checking  
✅ Historical tracking  
✅ >85% test coverage  
✅ Production ready  
✅ CI/CD integrated  
✅ Comprehensive security  

---

## Support

- **Docs**: README.md, DEPLOYMENT.md, EXAMPLES.md
- **Security**: SECURITY.md
- **Summary**: IMPLEMENTATION_SUMMARY.md

---

**Version**: 1.0.0  
**Status**: Production Ready ✅
