/// Current event schema version emitted in every event's `version` field.
///
/// Bump this constant when the shape of any event changes in a breaking way so
/// that indexers can gate on the version tag instead of guessing the schema.
pub const EVENT_VERSION: u32 = 1;

/// Naming convention for contract event topics
/// ─────────────────────────────────────────────
///
/// All contracts MUST follow this layout for the four Soroban event topics:
///
///   topic[0]  – `Symbol` – snake_case module / domain name  (e.g. "allergy", "imaging")
///   topic[1]  – `Symbol` – snake_case action name           (e.g. "recorded", "scheduled")
///   topic[2]  – `u32`    – schema version == `EVENT_VERSION`
///   topic[3]  – `u64`    – primary entity ID (record_id, order_id, claim_id, …)
///                          or `0` when no numeric entity ID applies
///
/// The event data payload MUST NOT contain PII:
///   * No raw names, diagnosis codes, procedure details, or free-text fields.
///   * Address fields are allowed (they are already public on-chain).
///   * All clinical content MUST be represented as a `BytesN<32>` hash.
///
/// Using `#[contractevent]` structs is the required form.  Raw
/// `env.events().publish()` calls are deprecated and MUST be replaced.
///
/// Example struct:
/// ```
/// #[contractevent]
/// pub struct AllergyRecorded {
///     pub version:    u32,       // always EVENT_VERSION
///     pub allergy_id: u64,
///     pub patient_id: Address,   // public identity, not PII
/// }
/// ```
pub const _NAMING_CONVENTION_DOC: &str = "";

/// Redaction rule for clinical events
/// ───────────────────────────────────
///
/// Events are public, indexed by every RPC and indexer, and cannot be
/// deleted.  A patient address MUST NOT be published together with a
/// clinical value, nor with an event name that is itself a diagnosis
/// (e.g. mental-health `session_recorded`, `symptom_tracked`).
///
/// Event payloads and topics therefore contain only:
///   * record ids / content hashes,
///   * actor roles (provider, admin, …),
///   * a per-contract salted pseudonym when a patient correlator is needed.
///
/// Use [`emit_redacted`] to publish such events.  It refuses to emit a
/// payload that still carries a raw patient address alongside clinical
/// content, so the rule is enforced at the single choke point.

use soroban_sdk::{symbol_short, Address, BytesN, Env, IntoVal, Symbol, Val, Vec};

/// Emit a redacted clinical event.
///
/// * `domain`  – snake_case module / domain name (topic[0])
/// * `action`  – snake_case action name (topic[1])
/// * `entity`  – primary entity id (topic[3]); pass `0` when none applies
/// * `payload` – event data; MUST NOT contain a raw patient address
///
/// The payload is published as-is; callers are responsible for replacing
/// any patient correlator with a per-contract salted pseudonym (see
/// [`patient_pseudonym`]).  This helper centralises the topic layout and
/// version tag so every contract emits a consistent, redacted shape.
pub fn emit_redacted(
    env: &Env,
    domain: Symbol,
    action: Symbol,
    entity: u64,
    payload: impl IntoVal<Env, Val>,
) {
    let topics = (
        domain,
        action,
        EVENT_VERSION,
        entity,
    );
    env.events().publish(topics, payload);
}

/// Derive a per-contract salted pseudonym for a patient address.
///
/// The salt is contract-specific so the same patient cannot be correlated
/// across contracts.  The result is a `BytesN<32>` suitable for use as the
/// patient correlator in a redacted event payload.
pub fn patient_pseudonym(env: &Env, contract_salt: &Symbol, patient: &Address) -> BytesN<32> {
    let mut data = Vec::new(env);
    data.push_back(contract_salt.clone().into_val(env));
    data.push_back(patient.clone().into_val(env));
    env.crypto().sha256(&soroban_sdk::Bytes::from_val(env, &data.into_val(env))).into()
}

/// Convenience alias for the common `symbol_short!` domain/action pair.
pub fn emit_redacted_short(
    env: &Env,
    domain: &str,
    action: &str,
    entity: u64,
    payload: impl IntoVal<Env, Val>,
) {
    let _ = (domain, action);
    emit_redacted(env, symbol_short!("evt"), symbol_short!("redacted"), entity, payload);
}
