# ZK eligibility verifier — expected system interface

This document describes how the Rust trait [`ZKProofVerifier`](src/interface.rs) is
expected to connect to a **real** zero-knowledge stack for **medical benefit
eligibility** without disclosing full records on-chain.

## Goals

1. **Privacy** — Sensitive clinical content stays in the witness *w*; the ledger sees
   only `proof`, agreed `public_inputs`, and identifiers such as `patient` (or
   commitments derived from them).
2. **Soundness** — Under standard cryptographic assumptions, an adversary cannot
   construct a valid `proof` for a false statement (e.g. ineligible patient claims
   eligibility).
3. **Composability** — Benefit contracts depend on a **single trait** so the proof
   system can be swapped (Groth16 → Plonk, etc.) behind the same call boundary.

## Actors

| Actor | Role |
|--------|------|
| **Patient / wallet** | Holds or authorizes use of witness data; may call a prover service. |
| **Prover service** | Off-chain: builds proof from *w* + `public_inputs` + VK parameters. |
| **Verifier** | On-chain: [`ZKProofVerifier::verify_eligibility_proof`]; uses VK (or VK hash + stored data). |
| **Benefit contract** | Calls verifier; if `true`, updates entitlements, nullifiers, etc. |

## Data flowing on-chain

| Field | Description |
|--------|-------------|
| `patient` | [`Address`](https://docs.rs/soroban-sdk/latest/soroban_sdk/struct.Address.html). Must be bound inside the circuit if proofs must not replay across accounts. |
| `proof` | Opaque bytes; layout = proof system + version specific. |
| `public_inputs` | [`Vec<Bytes>`](https://docs.rs/soroban-sdk/latest/soroban_sdk/struct.Vec.html); each element is one **public** circuit input with fixed order and serialization. |

### Suggested `public_inputs` layout (example only)

Document the real layout next to your circuit artifact. Example ordering:

0. Policy / benefit id (32-byte hash)  
1. Circuit / VK version (u32 BE)  
2. Epoch or ledger snapshot id (anti-replay)  
3. Merkle root of allowed ICD / CPT codes (or similar commitment)  
4. Optional: nullifier seed commitment for “one claim per period”

**Do not** put raw PHI in `public_inputs` unless explicitly intended.

## Lifecycle (target)

1. **Setup** — Trusted setup or universal SRS per proof system; publish VK id on-chain.
2. **Deposit / registration** — Optional: patient registers a commitment to a secret used in proofs.
3. **Prove** — Prover computes `proof` for statement “eligible under policy P.”
4. **Verify** — Contract calls `verify_eligibility_proof`; on `true`, executes business logic.
5. **Upgrade** — New VK id + version in `public_inputs`; old proofs may be rejected after cutoff.

## Soroban implementation notes

- **Host crypto** — Real verifiers may use `Env::crypto()` (hashes, curve ops where
  available) or precomputed checks; confirm limits for proof size and CPU budget.
- **Storage** — Store VK hash + version; optionally store compact VK if size allows.
- **Determinism** — No floats; canonical encodings for field elements.
- **Failure** — Return `false` for invalid proofs; use `panic!` only for programmer errors,
  not for adversarial inputs.

## Current stub

[`PlaceholderZkProofVerifier`](src/interface.rs) implements the trait but **always
returns `false`**. The free function [`verify_eligibility_proof`](src/interface.rs)
delegates to the trait and is behaviorally identical for the placeholder.

**Rust API version:** `RUST_INTERFACE_VERSION` in `interface.rs` — bump when signatures
or type meanings change (not the same as circuit version).

## Integration testing (future)

When a circuit exists:

- Add deterministic **test vectors** (proof + public inputs + expected bool).
- Run them in `tests/zk_verifier_integration.rs` and optionally in a thin Soroban
  contract test that invokes the real verifier behind `contractimpl`.

See commented skeletons in `tests/zk_verifier_integration.rs`.
