# Add financial reconciliation matching between medical-claims and billing records (#392)

## Summary

This PR implements automated financial reconciliation matching between the medical-claims and financial-records contracts to address revenue cycle leakage and audit failures.

## Changes

### Core Functionality
- **`reconcile_claim()`**: Links payment records from financial-records to specific claims with transactional guarantees
- **`get_unreconciled_claims()`**: Query function returning claims older than a configurable threshold
- **`mark_claim_disputed()`**: Allows marking claims with reconciliation discrepancies
- **`set_reconciliation_threshold()`**: Admin function to configure the reconciliation time threshold

### Reconciliation Status Tracking
Updated `ReconciliationStatus` enum with four states:
- **Pending**: Claim submitted but no payments applied
- **PartiallyPaid**: Some payments made but not fully reconciled
- **FullyReconciled**: All expected payments received and matched
- **Disputed**: Discrepancy between claim and payment amounts

### Enhanced Payment Records
Both `InsurerPaymentRecord` and `PatientPaymentRecord` now include:
- `reconciled: bool` - Tracks if payment has been reconciled
- `financial_record_owner: Option<Address>` - Links to financial record owner
- `financial_record_idx: Option<u32>` - Links to specific financial record

### Events
- **`ClaimReconciledEvent`**: Emitted on successful reconciliation with:
  - claim_id
  - payment_amount
  - claim_amount
  - outstanding_balance
  - reconciliation_status

### Contract Initialization
Updated `initialize()` to include:
- `financial_records_id: Address` - Address of financial-records contract
- `reconciliation_threshold: u64` - Time threshold for unreconciled claims (default: 24 hours)

## Acceptance Criteria ✓

- [x] Claim and payment linking is transactional (both update or neither does)
- [x] Partial payments are tracked with outstanding balance
- [x] Query for unreconciled claims works correctly
- [x] Events include claim amount, payment amount, and outstanding balance

## Testing

Added comprehensive test coverage:
- `test_reconcile_insurer_payment` - Verifies insurer payment reconciliation
- `test_reconcile_patient_payment` - Verifies patient payment reconciliation
- `test_reconcile_payment_already_reconciled` - Prevents double reconciliation
- `test_mark_claim_disputed` - Tests dispute marking
- `test_get_unreconciled_claims` - Verifies unreconciled claims query
- `test_set_reconciliation_threshold` - Tests threshold configuration
- `test_unauthorized_cannot_mark_disputed` - Verifies authorization

All 18 tests pass successfully.

## Documentation

Added `RECONCILIATION.md` with:
- Feature overview and key capabilities
- Data model changes
- Usage examples
- Error handling
- Migration notes
- Future enhancement suggestions

## Breaking Changes

⚠️ The `initialize()` function signature has changed. Existing deployments will need to:
1. Redeploy with new initialization parameters
2. Update client code to handle new payment record fields
3. Subscribe to `ClaimReconciled` events for audit trails

## Files Changed

- `contracts/medical-claims/src/lib.rs` - Core reconciliation implementation
- `contracts/medical-claims/src/types.rs` - Updated data structures and error codes
- `contracts/medical-claims/src/test.rs` - Comprehensive test coverage
- `contracts/medical-claims/RECONCILIATION.md` - Feature documentation

## Related Issues

Closes #392
