# Contract Upgrade Guide

This upgrade guide walks operators through the Soroban contract upgrade process, covering schema migration, governance workflows, rollback, and testnet dry runs.

## 1. Pre-upgrade checklist

Before upgrading a live contract, confirm the following:

- Schema compatibility: new contract data schemas must read existing state and migrate gracefully.
- Test coverage: unit tests for contract entry points, migration logic, and rollback behavior.
- Staging deployment: deploy the upgrade to a testnet or local network before production.
- Governance readiness: admin and multisig keys are available and approved for upgrade proposals.

Example checklist commands:

```bash
# Run unit tests for the contract and shared modules
cargo test --workspace --package upgrade-governance

# Build the new WASM for review
soroban contract build --wasm-path target/wasm32-unknown-unknown/release/upgrade_wasm.wasm
```

## 2. Governance proposal submission

Upgrades should be submitted through the configured multi-sig governance process.

1. Create the upgrade proposal payload with the new WASM hash.
2. Submit the proposal to the governance contract.
3. Record the proposal ID and all required approvers.

Example proposal submission:

```bash
NEW_WASM_HASH=$(soroban contract hash target/wasm32-unknown-unknown/release/patient_registry.wasm)
PROPOSAL_ID=upgrade-patient-registry-$(date +%s)

soroban contract invoke \
  --id <UPGRADE_GOVERNANCE_CONTRACT> \
  --source admin \
  --network testnet \
  -- create_upgrade_proposal \
  --proposal_id "$PROPOSAL_ID" \
  --contract_name "patient_registry" \
  --new_wasm_hash "$NEW_WASM_HASH" \
  --description "Upgrade patient registry schema and governance path"
```

## 3. Multi-sig approval collection

Collect signatures from the configured multi-sig participants.

- Determine the minimum signer threshold from the governance contract.
- Notify approvers with the proposal ID and expected timeline.
- Track approval deadlines and handle missing signers before timeout.

Example approval command:

```bash
soroban contract invoke \
  --id <UPGRADE_GOVERNANCE_CONTRACT> \
  --source signer1 \
  --network testnet \
  -- approve_upgrade \
  --proposal_id "$PROPOSAL_ID"
```

If a proposal is not approved within the configured timeout window, cancel or resubmit it:

```bash
soroban contract invoke \
  --id <UPGRADE_GOVERNANCE_CONTRACT> \
  --source admin \
  --network testnet \
  -- cancel_upgrade_proposal \
  --proposal_id "$PROPOSAL_ID"
```

## 4. Schema migration entry point

A contract upgrade should include a dedicated migration entry point such as `migrate_schema`.

The entry point should:

- Detect the current schema version in contract storage.
- Apply incremental migrations in a deterministic order.
- Preserve existing data and reject incompatible states.

Example invocation:

```bash
soroban contract invoke \
  --id <PATIENT_REGISTRY_CONTRACT> \
  --source admin \
  --network testnet \
  -- migrate_schema \
  --target_version 2
```

Example migration logic pattern in Rust:

```rust
pub fn migrate_schema(env: Env, target_version: u32) {
    let current_version: u32 = env.storage().get(&MigrationVersionKey).unwrap_or(1);
    if current_version >= target_version {
        return;
    }

    if current_version == 1 && target_version == 2 {
        migrate_v1_to_v2(&env);
    }

    env.storage().set(&MigrationVersionKey, &target_version);
}
```

## 5. Rollback procedure

If a post-upgrade verification fails, follow rollback procedures immediately:

- Halt dependent operations if possible.
- Restore the previous WASM and schema state.
- Revert any invalid or incomplete data migrations.
- Document the root cause and update the upgrade checklist.

Example rollback command:

```bash
PREVIOUS_WASM_HASH=<previous_hash>

soroban contract invoke \
  --id <UPGRADE_GOVERNANCE_CONTRACT> \
  --source admin \
  --network testnet \
  -- rollback_upgrade \
  --proposal_id "$PROPOSAL_ID" \
  --previous_wasm_hash "$PREVIOUS_WASM_HASH"
```

## 6. Testnet dry run

Simulate the upgrade in a testnet environment before executing on production.

Steps:

1. Deploy the new WASM to testnet.
2. Submit and approve a governance proposal in testnet.
3. Execute the `migrate_schema` entry point.
4. Run end-to-end verification scenarios.

Dry run example:

```bash
# Deploy the upgrade contract on testnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/patient_registry.wasm \
  --source admin \
  --network testnet

# Execute migration and validate a sample record
soroban contract invoke \
  --id <PATIENT_REGISTRY_CONTRACT> \
  --source admin \
  --network testnet \
  -- migrate_schema \
  --target_version 2

soroban contract invoke \
  --id <PATIENT_REGISTRY_CONTRACT> \
  --source admin \
  --network testnet \
  -- get_patient_record \
  --patient_id "P12345"
```

## 7. Verification after upgrade

Confirm the upgrade succeeded by checking:

- Governance proposal status is marked as executed.
- Migration version storage is updated.
- Existing records are accessible and correct.
- Event logs show the upgrade and migration occurred.

Example verification command:

```bash
soroban contract invoke \
  --id <UPGRADE_GOVERNANCE_CONTRACT> \
  --source admin \
  --network testnet \
  -- get_upgrade_status \
  --proposal_id "$PROPOSAL_ID"
```

## 8. Useful notes

- Keep a changelog of schema versions and release notes.
- Avoid breaking migrations during a single upgrade window.
- Use feature flags or compatibility checks if rolling out state changes gradually.
