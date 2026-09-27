//! Fuzz target: `request_data_export` / `validate_export_ticket`.
//!
//! Generates a legitimate export ticket, then constructs mutated variants by
//! flipping bytes in the nonce and signature fields.
//!
//! # Invariants checked
//! 1. A freshly-issued ticket is always valid (`validate_export_ticket` → true).
//! 2. Any ticket with a mutated signature field always returns false.
//! 3. A ticket with `issued_at > expires_at` always returns false.
//! 4. A ticket whose timestamp is in the past returns false.
//! 5. No combination of inputs panics.
#![no_main]

use libfuzzer_sys::fuzz_target;
use patient_registry::{ExportTicket, MedicalRegistry, MedicalRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env,
};

fuzz_target!(|data: &[u8]| {
    // Layout (minimum 34 bytes):
    // [0..4]   base_ts u32 le    — ledger timestamp
    // [4..8]   delta u32 le      — added to issued_at to form expires_at (tests expiry)
    // [8..40]  sig_mutation [u8; 32] — XOR mask applied to the signature bytes
    if data.len() < 40 {
        return;
    }

    let base_ts = u32::from_le_bytes(data[0..4].try_into().unwrap()) as u64;
    let delta = u32::from_le_bytes(data[4..8].try_into().unwrap()) as u64;
    let sig_mask: [u8; 32] = data[8..40].try_into().unwrap();

    let env = Env::default();
    env.mock_all_auths();

    let ts = base_ts.saturating_add(1);
    env.ledger().with_mut(|l| {
        l.timestamp = ts;
    });

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    // Initialize.
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client
        .try_initialize(&admin, &treasury, &fee_token)
        .unwrap_or_default();

    let patient = Address::generate(&env);

    // ── Invariant 1: freshly issued ticket must be valid ─────────────────────
    let ticket = client.request_data_export(&patient);
    assert!(
        client.validate_export_ticket(&ticket),
        "freshly issued ticket must validate as true"
    );

    // ── Invariant 2: mutated signature must be invalid ────────────────────────
    // Build a new signature by XOR-ing every byte with the fuzz-supplied mask.
    // If the mask is all-zero the signature is unchanged — skip to avoid a
    // trivially-passing assertion.
    let all_zero_mask = sig_mask.iter().all(|&b| b == 0);
    if !all_zero_mask {
        let orig_sig: [u8; 32] = ticket.signature.to_array();
        let mut mutated = [0u8; 32];
        for (i, (&orig, &mask)) in orig_sig.iter().zip(sig_mask.iter()).enumerate() {
            mutated[i] = orig ^ mask;
        }
        // Guard: if mutation happened to produce the same bytes, skip.
        if mutated != orig_sig {
            let bad_ticket = ExportTicket {
                patient: ticket.patient.clone(),
                issued_at: ticket.issued_at,
                expires_at: ticket.expires_at,
                nonce: ticket.nonce.clone(),
                signature: BytesN::from_array(&env, &mutated),
            };
            assert!(
                !client.validate_export_ticket(&bad_ticket),
                "mutated-signature ticket must not validate"
            );
        }
    }

    // ── Invariant 3: expires_at in the past → false ───────────────────────────
    // Advance ledger past expires_at.
    let past_ticket = ExportTicket {
        patient: ticket.patient.clone(),
        issued_at: ticket.issued_at,
        // expires_at strictly before current ledger ts
        expires_at: ts.saturating_sub(1),
        nonce: ticket.nonce.clone(),
        signature: ticket.signature.clone(),
    };
    // The signature will also be wrong because expires_at changed, but the
    // expiry check fires first.
    assert!(
        !client.validate_export_ticket(&past_ticket),
        "expired ticket must not validate"
    );

    // ── Invariant 4: arbitrary delta tickle ───────────────────────────────────
    // Advance ledger by `delta` seconds past `expires_at` and re-validate.
    if delta > 3600 {
        env.ledger().with_mut(|l| {
            l.timestamp = ticket.expires_at.saturating_add(delta);
        });
        assert!(
            !client.validate_export_ticket(&ticket),
            "ticket with elapsed TTL must not validate"
        );
    }
});
