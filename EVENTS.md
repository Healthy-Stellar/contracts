# Contract Events Reference

This document describes the events emitted by the Meshmulla healthcare smart contracts. Off-chain clients can subscribe to these events to build real-time notifications, audit logs, and analytics dashboards.

---

## Privacy Rule for Event Payloads and Topics

Contract events are public, indexed by every RPC and indexer, and cannot be deleted. Anyone can rebuild a patient timeline from them. Therefore **all contracts MUST follow this workspace rule**:

> Event payloads and topics contain **record ids, content hashes and actor roles** — never a patient address alongside clinical values or condition-revealing event names.

Concretely:

- **No `patient_id` / patient `Address` in the same event as a clinical value** (vital values, weights, vaccine names, device models, medication names, diagnosis-revealing event names, free-text reasons, etc.).
- **No condition-revealing event names** that are themselves a diagnosis (e.g. `session_recorded`, `symptom_tracked`, `outcomes_tracked`, `consent_granted(data_type)` for mental-health / substance-use treatment).
- **Where a patient correlator is needed**, use a **per-contract salted pseudonym** derived from a secret, rotatable key — never the raw patient address. The salt must not be emitted or recoverable from public on-chain data.
- **Prefer record ids and content hashes** as the public correlator; clinical content stays in encrypted envelopes / hashed state.

### Shared helper: `shared::events::emit_redacted(...)`

Affected contracts emit through the shared helper instead of calling `env.events().publish(...)` directly. The helper enforces the rule above: it accepts only record ids, content hashes, actor roles and (optionally) a per-contract salted pseudonym — it does not accept a raw patient address together with a clinical value.

```rust
// shared::events::emit_redacted(env, topic, record_id, content_hash, actor_role, pseudonym)
// - topic:        Symbol discriminator (must not be a diagnosis)
// - record_id:    u64 record identifier
// - content_hash: BytesN<32> commitment to the clinical content
// - actor_role:   Symbol role (e.g. "provider", "patient", "system")
// - pseudonym:    Option<BytesN<32>> per-contract salted patient correlator
```

Each affected contract has a test asserting that **no emitted event includes `patient_id` together with a clinical value**.

---

## Patient Registry (`patient-registry`)

### `new_record` — Provider-to-Patient Record Notification

Emitted when a provider (doctor) successfully adds a medical record for a patient via `add_medical_record`. Designed to let off-chain clients alert patients in real time.

| Field | Location | Type | Description |
|-------|----------|------|-------------|
| `"new_record"` | Topic[0] | `Symbol` | Event discriminator |
| `patient_address` | Topic[1] | `Address` | The patient the record belongs to |
| `provider_address` | Topic[2] | `Address` | The doctor who created the record |
| `record_id` | Data[0] | `u64` | Auto-incremented record identifier (per patient) |
| `record_type` | Data[1] | `Symbol` | Type of record (e.g., `LAB`, `IMAGING`, `PRESCRIPTION`) |
| `timestamp` | Data[2] | `u64` | Ledger timestamp when the record was created |

**Constant:** `NEW_RECORD_TOPIC = "new_record"` (defined in `lib.rs`)

#### Example: Subscribing Off-Chain

```jsonc
// Filter for events with topic[0] == "new_record"
// and topic[1] == <patient_address> to notify a specific patient
{
  "topic": ["new_record", "<PATIENT_ADDRESS>", "*"],
  "data": { "record_id": 1, "record_type": "LAB", "timestamp": 1700000000 }
}
```

---

### Other Events

| Event | Topics | Data | Description |
|-------|--------|------|-------------|
| `reg_pat` | `("reg_pat", wallet)` | `"success"` | Patient registered |
| `upd_pat` | `("upd_pat", wallet)` | `"success"` | Patient metadata updated |
| `reg_doc` | `("reg_doc", wallet)` | `"success"` | Doctor registered |
| `ver_doc` | `("ver_doc", wallet)` | `"verified"` | Doctor verified by institution |
| `consent_v` | `("consent_v", admin)` | `BytesN<32>` | New consent version published |
| `consent_a` | `("consent_a", patient)` | `BytesN<32>` | Patient acknowledged consent |
| `grd_asgn` | `("grd_asgn", patient)` | `Address` | Guardian assigned to patient |
| `grd_rev` | `("grd_rev", patient)` | `"revoked"` | Guardian revoked |
| `hold_set` | `("hold_set", patient)` | `(reason_hash, expires_at, placed_at)` | Regulatory hold placed |
| `hold_lift` | `("hold_lift", patient)` | `(reason_hash, expires_at, placed_at, lifted_at)` | Regulatory hold lifted |
| `snap_meta` | `("snap_meta", ledger_seq)` | `(patient_count, doctor_count, consent_ver)` | State snapshot metadata |
| `snap_pats` | `("snap_pats", ledger_seq)` | `Vec<Address>` | Snapshot: all patient addresses |
| `snap_docs` | `("snap_docs", ledger_seq)` | `Vec<Address>` | Snapshot: all doctor addresses |

---

## PACS Integration (`pacs-integration`)

### `anon` — Study Anonymization Recorded

Emitted when a study is anonymized for research. The contract records only a **commitment** to the anonymized UID; the pseudonym itself is derived off-chain by the PACS/anonymization service using a secret, rotatable HMAC key. The on-chain salt is a per-study random value from `env.prng()` that is never emitted or returned, so pseudonyms cannot be recomputed from public on-chain data.

| Field | Location | Type | Description |
|-------|----------|------|-------------|
| `"anon"` | Topic[0] | `Symbol` | Event discriminator |
| `commitment` | Data[0] | `BytesN<32>` | Commitment to the anonymized UID (does not reveal the UID) |
| `anonymization_level` | Data[1] | `Symbol` | Level of anonymization applied |
| `purpose` | Data[2] | `Symbol` | Research purpose |
| `rotation_epoch` | Data[3] | `u32` | Key-rotation epoch used by the off-chain service |

**Privacy note:** `study_id` and `requesting_researcher` are intentionally **not** emitted together with the anonymized record, so researchers cannot be linked to specific patients' studies. The `study_reg` event still maps `study_id → patient_id` for registry purposes; pseudonyms must therefore never be derived from on-chain-recoverable inputs.
