# Contract Events Reference

This document describes the events emitted by the Meshmulla healthcare smart contracts. Off-chain clients can subscribe to these events to build real-time notifications, audit logs, and analytics dashboards.

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
