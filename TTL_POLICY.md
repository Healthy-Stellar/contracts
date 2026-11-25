# Storage TTL Bump Policy

## Overview

This document defines the repository-wide storage TTL (Time-To-Live) bump policy for the decentralized healthcare system. It ensures consistent data retention across all smart contracts and prevents silent data expiry.

## Problem Statement

Previously, TTL extension was inconsistent across contracts:

- Some contracts (patient-registry, pacs-integration) aggressively bumped keys
- Most contracts (35+) relied on default Soroban TTL or didn't explicitly manage it
- This risked silent data expiry for critical healthcare records

## Solution: Retention Classes

We define three retention classes based on data criticality:

### 0. Instance Storage (every contract)

**Used for:** admin, id counters, registry pointers, config — and the contract-instance record itself.

- **Extend to:** the network maximum entry TTL, read at runtime from the network config
  (`env.storage().max_ttl()`, i.e. `max_entry_ttl`), so the value follows whatever the network
  the contract is deployed on allows.
- **Threshold:** maximum − `MAX_TTL_REBUMP_MARGIN` (17,280 ledgers, ~1 day), so repeated calls
  within a day do not pay for redundant extensions.
- **Policy:** call `ttl_config::extend_instance_ttl(&env)` at the top of **every state-changing
  `#[contractimpl]` entry point** (including `__constructor` / `initialize`).

If the instance entry is archived, **every** call to the contract fails until someone restores it.
CI (`scripts/check-instance-ttl.sh`, job *Instance TTL Check*) fails when a contract crate never
references `extend_instance_ttl`.

### 1. Critical Retention Class

**Used for:** Patient records, medical history, prescriptions, clinical trials, allergy records

- **Extend to (helpers):** the network maximum entry TTL, read from the network config at
  runtime. `extend_critical_ttl` / `extend_critical_ttl_if_exists` no longer stop at ~31 days:
  a patient who does not touch their record between visits keeps it live for the full network
  maximum (several months on mainnet).
- **Threshold (helpers):** maximum − 17,280 ledgers (~1 day)
- **Floor constants:** `critical::LEDGER_BUMP_AMOUNT` / `MIN_TTL_LEDGERS` = 535,680 ledgers
  (~31 days), `critical::LEDGER_THRESHOLD` = 518,400 — kept for callers that pass explicit values
  to `extend_ttl`; prefer the helpers.
- **Policy:** Bump on every write and read operation

**Contracts using Critical:**

- patient-registry
- pacs-integration
- allergy-management
- health-records (recommended)
- prescription-management (recommended)
- clinical-trial (recommended)

### 2. Operational Retention Class

**Used for:** Temporary records, session data, intermediate states, audit logs

- **Bump Amount:** 120,960 ledgers (~7 days at 5s/ledger)
- **Threshold:** 60,480 ledgers (~3.5 days)
- **Minimum TTL:** 120,960 ledgers
- **Policy:** Bump on write operations; optional on reads

**Recommended for:**

- telemedicine (session data)
- medical-claims (temporary states)
- referral (intermediate states)

### 3. Ephemeral Retention Class

**Used for:** Persistent counters, temporary caches, transient state

- **Bump Amount:** 17,280 ledgers (~1 day at 5s/ledger)
- **Threshold:** 8,640 ledgers (~12 hours)
- **Minimum TTL:** 17,280 ledgers
- **Policy:** Bump on write operations only

**Recommended for:**

- Persistent counters that can be rebuilt (instance-storage counters follow the Instance policy above)
- Temporary caches
- Session tokens

## Implementation

### Centralized Configuration

All TTL constants are defined in `contracts/ttl-config/src/lib.rs`:

```rust
pub const MAX_TTL_REBUMP_MARGIN: u32 = 17_280; // ~1 day

pub fn max_entry_ttl(env: &Env) -> u32; // network max, from config

pub mod critical {
    // Floor values; the helpers extend to the network maximum.
    pub const LEDGER_BUMP_AMOUNT: u32 = 535_680;
    pub const LEDGER_THRESHOLD: u32 = 518_400;
}

pub mod operational {
    pub const LEDGER_BUMP_AMOUNT: u32 = 120_960;
    pub const LEDGER_THRESHOLD: u32 = 60_480;
}

pub mod ephemeral {
    pub const LEDGER_BUMP_AMOUNT: u32 = 17_280;
    pub const LEDGER_THRESHOLD: u32 = 8_640;
}
```

### Helper Functions

The `ttl-config` crate provides helper functions for easy TTL management:

```rust
// Extend the contract instance (admin, counters, registry pointers, instance record)
extend_instance_ttl(env);

// Extend TTL for a key
extend_critical_ttl(env, &key);
extend_operational_ttl(env, &key);
extend_ephemeral_ttl(env, &key);

// Conditionally extend if key exists
extend_critical_ttl_if_exists(env, &key);
extend_operational_ttl_if_exists(env, &key);
extend_ephemeral_ttl_if_exists(env, &key);
```

### Usage Pattern

**On Write Operations:**

```rust
pub fn save_record(env: &Env, record: &Record) {
    let key = DataKey::Record(record.id);
    env.storage().persistent().set(&key, record);
    extend_critical_ttl(env, &key);  // Always bump on write
}
```

**On Read Operations (Critical Data):**

```rust
pub fn get_record(env: &Env, record_id: u64) -> Result<Record, Error> {
    let key = DataKey::Record(record_id);
    let result = env.storage().persistent().get(&key).ok_or(Error::NotFound);

    if result.is_ok() {
        extend_critical_ttl_if_exists(env, &key);  // Bump on successful read
    }

    result
}
```

## Migration Guide

### For Existing Contracts

1. **Add dependency** to `Cargo.toml`:

   ```toml
   [dependencies]
   ttl-config = { path = "../ttl-config" }
   ```

2. **Replace local constants** with imports:

   ```rust
   use ttl_config::critical::{LEDGER_BUMP_AMOUNT, LEDGER_THRESHOLD};
   ```

3. **Add TTL bumping** to storage functions:
   - Write operations: Always bump
   - Read operations: Bump if critical data

4. **Test** that TTL is extended:
   - Verify snapshots include TTL extension calls
   - Add tests for TTL bump behavior

### For New Contracts

1. Add `ttl-config` dependency
2. Import appropriate retention class
3. Implement TTL bumping in storage layer
4. Document retention class choice in contract README

## Testing

### TTL Bump Verification

Each contract should include tests verifying TTL bumping:

```rust
#[test]
fn test_record_ttl_bumped_on_write() {
    let env = Env::default();
    let contract = setup(&env);

    // Write a record
    contract.save_record(&record);

    // Verify TTL was extended (check snapshots)
    // TTL should be >= LEDGER_BUMP_AMOUNT
}

#[test]
fn test_record_ttl_bumped_on_read() {
    let env = Env::default();
    let contract = setup(&env);

    // Write and read a record
    contract.save_record(&record);
    let retrieved = contract.get_record(record.id);

    // Verify TTL was extended on read
}
```

### Snapshot Testing

Test snapshots capture TTL extension calls. Example from allergy-management:

```json
{
  "ledger": {...},
  "storage": {
    "persistent": [
      {
        "key": "Allergy(1)",
        "value": {...},
        "ttl_extended": true
      }
    ]
  }
}
```

## Monitoring & Maintenance

The `Extend Contract TTLs` workflow runs weekly (Mondays 06:00 UTC) as a backstop for contracts
that are not called often enough for the on-chain bumps to keep them live. Scheduled runs are
skipped while the manifest is `not-yet-deployed`; any other failure opens (or comments on) an
issue titled `Extend Contract TTLs failed (<network>)`.

### Key Metrics

- **TTL Expiry Rate:** Monitor contracts for unexpected data loss
- **Bump Frequency:** Verify bumps occur at expected intervals
- **Storage Growth:** Track persistent storage size per contract

### Alerts

Set up alerts for:

- Records approaching TTL expiry without bumps
- Contracts with no TTL bump activity
- Unexpected storage deletions

## Manual TTL Extension

When the automated `Extend Contract TTLs` workflow fails, or you need to extend
TTLs outside the schedule, run `scripts/extend-ttls.sh` directly.

### 1. Prerequisites

- The [Stellar CLI](https://developers.stellar.org/docs/tools/cli/stellar-cli) (`stellar`) installed and on `PATH`.
- A `deployments/<network>.json` manifest containing the contract IDs to extend (e.g. `deployments/mainnet.json`).
- A Stellar CLI identity/secret with enough XLM to pay the extension fees for every contract in the manifest.

### 2. Identity / secret to use

- **Mainnet:** use the identity backing the `MAINNET_DEPLOYER_IDENTITY` GitHub Actions secret — the same
  identity the scheduled job authenticates with. Get the secret value from whoever holds repo/org secret
  access (repo admin or the deployment owner), then import it locally before running the script:

  ```bash
  stellar keys add mainnet-deployer --secret-key   # paste the MAINNET_DEPLOYER_IDENTITY secret value
  ```

  Never commit this secret or paste it into a shell history file that gets synced anywhere. Prefer an
  interactive prompt (as above) over passing `--secret-key` inline.

- **Testnet:** use the identity backing `TESTNET_DEPLOYER_IDENTITY`, imported the same way under a
  different local key name (e.g. `testnet-deployer`).

### 3. Run the script

```bash
# Dry run first — logs what would be extended without submitting any transactions.
./scripts/extend-ttls.sh \
  --network mainnet \
  --identity mainnet-deployer \
  --ledgers-to-extend 535680 \
  --dry-run

# Then the real run.
./scripts/extend-ttls.sh \
  --network mainnet \
  --identity mainnet-deployer \
  --ledgers-to-extend 535680
```

Key flags (see `./scripts/extend-ttls.sh --help` for the full list):

- `--network <name>` — `mainnet` or `testnet`.
- `--identity <name>` — the local CLI identity name from step 2 (not the raw secret).
- `--ledgers-to-extend <count>` — defaults to `535680` (~1 year at 5s/ledger); match the value the
  cron job normally uses unless you have a specific reason to diverge.
- `--dry-run` — logs the contracts that would be extended without calling `stellar contract extend`.

The script reads contract IDs out of `deployments/<network>.json` and calls `stellar contract extend`
for each one, printing an `Extended: N / Failed: N / Total: N` summary at the end and exiting non-zero
if any contract failed.

### 4. Verify success

- **Script output:** confirm the `Extended: N / Failed: 0 / Total: N` summary shows every contract
  succeeded and the process exited `0`.
- **On-chain check:** for each contract ID, query the current TTL and confirm it is at least the
  requested `--ledgers-to-extend` value:

  ```bash
  stellar contract info \
    --network mainnet \
    --id <CONTRACT_ID>
  ```

  The reported `liveUntilLedger` should be well ahead of the current ledger. Repeat for every entry in
  `deployments/mainnet.json`.
- **Workflow re-run:** re-run the failed `Extend Contract TTLs` job (or wait for the next scheduled
  run) and confirm it completes green. If it still fails, capture the run URL and error output in the
  tracking issue before escalating.

### 5. Archival and restore runbook

Soroban archives an entry when its TTL reaches zero. Archived entries are not deleted, but any
transaction that touches them fails until they are restored.

- **Instance entry archived:** every call to the contract fails (admin, counters and registry
  pointers live there). Restore it first.
- **Persistent entry archived:** only calls that read/write that key fail (e.g. one patient's
  record).

To restore:

1. Identify the archived entries. Simulating the failing call reports the keys that need
   restoring (`stellar contract invoke ... --send=no` prints a restore preamble), or use
   `stellar contract info --id <CONTRACT_ID>` for the instance entry.
2. Restore them:

   ```bash
   # Contract instance + code
   stellar contract restore --network mainnet --source mainnet-deployer --id <CONTRACT_ID>

   # A specific persistent key (XDR-encoded ScVal of the DataKey)
   stellar contract restore --network mainnet --source mainnet-deployer \
     --id <CONTRACT_ID> --key-xdr <BASE64_KEY_XDR> --durability persistent
   ```

3. Immediately extend what was restored (restore only grants the network minimum TTL):

   ```bash
   ./scripts/extend-ttls.sh --network mainnet --identity mainnet-deployer
   ```

4. Re-run the failing call and confirm it succeeds; record the incident (affected contract and
   keys, cause, restore tx hashes) on the tracking issue.

### 6. When the scheduled job fails

If the automatic TTL extension job fails (e.g. the run reported in issue #580 on
`2026-07-20T03:23:37.546Z` for `mainnet`), treat it as an incident:

1. **Triage the run.** Open the linked workflow run and read the failing step's logs. Common causes:
   expired/rotated deployer secret, insufficient XLM balance, RPC/network outage, or a contract ID
   missing from `deployments/<network>.json`.
2. **Extend manually.** Follow sections 1–4 above to extend TTLs by hand so no contract expires while
   the automation is being fixed.
3. **Record the outcome.** Comment on the tracking issue with the run URL, the root cause, the manual
   extension summary (`Extended/Failed/Total`), and the follow-up fix (e.g. rotate the secret, top up
   the deployer account).
4. **Confirm recovery.** Once the underlying cause is fixed, verify the next scheduled run succeeds
   and close the issue.
