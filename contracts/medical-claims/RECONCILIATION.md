# Financial Reconciliation Feature (#392)

## Overview

The medical-claims contract now includes automated financial reconciliation matching between medical claims and billing records from the financial-records contract. This feature addresses revenue cycle leakage and audit failures by providing transactional linking between claims and payments.

## Key Features

### 1. Reconciliation Status Tracking

Claims now track their reconciliation status through four states:

- **Pending**: Claim has been submitted but no payments have been applied
- **PartiallyPaid**: Some payments have been made but the claim is not fully reconciled
- **FullyReconciled**: All expected payments have been received and matched
- **Disputed**: There is a discrepancy between claim and payment amounts

### 2. Payment-to-Claim Linking

The `reconcile_claim` function provides transactional linking between payment records and claims:

```rust
pub fn reconcile_claim(
    env: Env,
    claim_id: u64,
    payment_idx: u32,
    is_insurer_payment: bool,
    financial_record_owner: Address,
    financial_record_idx: u32,
    caller: Address,
) -> Result<(), Error>
```

**Features:**
- Links payment records from financial-records contract to specific claims
- Transactional operation - both claim and payment update or neither does
- Prevents double reconciliation of the same payment
- Emits `ClaimReconciled` event with payment details and outstanding balance
- Supports both insurer and patient payments

**Authorization:**
- Insurer payments: Only the registered insurer for the claim can reconcile
- Patient payments: Only the patient associated with the claim can reconcile

### 3. Unreconciled Claims Query

The `get_unreconciled_claims` function returns claims older than a configurable threshold:

```rust
pub fn get_unreconciled_claims(
    env: Env,
    insurer_id: Address,
) -> Result<Vec<u64>, Error>
```

**Features:**
- Returns claim IDs for unreconciled claims older than the threshold
- Configurable threshold (default: 24 hours / 86400 seconds)
- Only accessible by registered insurers
- Filters out fully reconciled claims automatically

### 4. Dispute Management

The `mark_claim_disputed` function allows marking claims with reconciliation discrepancies:

```rust
pub fn mark_claim_disputed(
    env: Env,
    claim_id: u64,
    caller: Address,
) -> Result<(), Error>
```

**Authorization:** Only the insurer or provider can mark a claim as disputed

### 5. Configurable Reconciliation Threshold

Administrators can update the reconciliation threshold:

```rust
pub fn set_reconciliation_threshold(
    env: Env,
    admin: Address,
    threshold: u64,
) -> Result<(), Error>
```

## Data Model Changes

### Enhanced Payment Records

**InsurerPaymentRecord:**
```rust
pub struct InsurerPaymentRecord {
    pub payment_date: u64,
    pub payment_amount: i128,
    pub payment_reference_hash: BytesN<32>,
    pub reconciled: bool,                        // NEW
    pub financial_record_owner: Option<Address>, // NEW
    pub financial_record_idx: Option<u32>,       // NEW
}
```

**PatientPaymentRecord:**
```rust
pub struct PatientPaymentRecord {
    pub payment_date: u64,
    pub payment_amount: i128,
    pub reconciled: bool,                        // NEW
    pub financial_record_owner: Option<Address>, // NEW
    pub financial_record_idx: Option<u32>,       // NEW
}
```

### Reconciliation Event

```rust
pub struct ClaimReconciledEvent {
    pub claim_id: u64,
    pub payment_amount: i128,
    pub claim_amount: i128,
    pub outstanding_balance: i128,
    pub reconciliation_status: ReconciliationStatus,
}
```

## Initialization Changes

The `initialize` function now requires additional parameters:

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    access_control_id: Address,
    financial_records_id: Address,      // NEW
    reconciliation_threshold: u64,      // NEW
) -> Result<(), Error>
```

## Usage Examples

### Reconciling an Insurer Payment

```rust
// After processing a payment
client.process_payment(
    &claim_id,
    &insurer,
    &10000,
    &payment_date,
    &payment_ref_hash,
);

// Link to financial record
client.reconcile_claim(
    &claim_id,
    &0,              // payment index
    &true,           // is insurer payment
    &fr_owner,       // financial record owner
    &fr_idx,         // financial record index
    &insurer,        // caller
);
```

### Reconciling a Patient Payment

```rust
// After patient payment
client.apply_patient_payment(
    &claim_id,
    &patient,
    &2000,
    &payment_date,
);

// Link to financial record
client.reconcile_claim(
    &claim_id,
    &0,              // payment index
    &false,          // is patient payment
    &fr_owner,       // financial record owner
    &fr_idx,         // financial record index
    &patient,        // caller
);
```

### Querying Unreconciled Claims

```rust
// Get claims older than threshold
let unreconciled_claims = client.get_unreconciled_claims(&insurer_id);

for claim_id in unreconciled_claims.iter() {
    // Process unreconciled claim
}
```

### Marking a Claim as Disputed

```rust
client.mark_claim_disputed(&claim_id, &insurer);
```

## Error Handling

New error codes:
- `PaymentNotFound`: Payment index does not exist
- `PaymentAlreadyReconciled`: Attempting to reconcile an already reconciled payment
- `InvalidReconciliationAmount`: Payment amount doesn't match expected values

## Acceptance Criteria ✓

- [x] Claim and payment linking is transactional (both update or neither does)
- [x] Partial payments are tracked with outstanding balance
- [x] Query for unreconciled claims works correctly
- [x] Events include claim amount, payment amount, and outstanding balance

## Testing

The implementation includes comprehensive tests:
- `test_reconcile_insurer_payment`: Verifies insurer payment reconciliation
- `test_reconcile_patient_payment`: Verifies patient payment reconciliation
- `test_reconcile_payment_already_reconciled`: Prevents double reconciliation
- `test_mark_claim_disputed`: Tests dispute marking functionality
- `test_get_unreconciled_claims`: Verifies unreconciled claims query
- `test_set_reconciliation_threshold`: Tests threshold configuration
- `test_unauthorized_cannot_mark_disputed`: Verifies authorization checks

## Migration Notes

Existing deployments will need to:
1. Redeploy with the new `initialize` signature including `financial_records_id` and `reconciliation_threshold`
2. Update client code to handle new payment record fields
3. Subscribe to `ClaimReconciled` events for audit trails
4. Implement periodic queries for unreconciled claims

## Future Enhancements

Potential improvements:
- Automatic reconciliation based on payment reference matching
- Bulk reconciliation operations
- Reconciliation reports and analytics
- Integration with external payment processors
- Automated dispute resolution workflows
