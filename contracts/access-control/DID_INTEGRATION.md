# DID Integration Guide

This contract now includes an integration layer that links Soroban `Address` values to W3C Decentralized Identifiers (DIDs).

## Purpose

- Anchor identity bindings on-chain for both patients and providers.
- Enable future cross-chain identity checks by storing a canonical DID per address.
- Emit an auditable DID-change event without exposing the old/new DID values directly.

## API

### `register_did(address: Address, did: Bytes) -> Result<(), ContractError>`

- Self-registration only: `address` must authorize the call (`address.require_auth()`).
- Validates DID format: must begin with ASCII prefix `did:`.
- Stores/updates DID under persistent storage key `Did(address)`.
- Emits audit event:
  - Topic: `("did_aud", address)`
  - Data: `(old_did_hash: Option<BytesN<32>>, new_did_hash: BytesN<32>)`

### `get_did(address: Address) -> Option<Bytes>`

- Returns DID bytes if registered; otherwise `None`.

## Validation Rules

- Accepted: `did:example:123`, `did:stellar:provider:abc`, `did:key:z6M...`
- Rejected: values not starting with `did:`

## Integration Pattern for DID Resolution

1. Read on-chain DID:
   - Call `get_did(address)`.
2. Resolve DID off-chain:
   - Send DID to your DID resolver (method-specific, e.g. `did:key`, `did:web`).
3. Verify controller binding:
   - Compare resolved DID document controller/public key against expected signer.
4. Optional audit check:
   - Track `did_aud` events to detect DID rotations and require re-verification.

## Security Notes

- One address can only self-assert or rotate its DID (no third-party writes).
- Audit events emit hashes, not plain DID history, reducing metadata leakage.
- Downstream services should re-verify proofs whenever a DID hash changes.
