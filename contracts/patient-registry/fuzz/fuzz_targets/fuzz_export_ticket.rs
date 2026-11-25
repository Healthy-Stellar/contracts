//! Fuzz target: `request_data_export` / `validate_export_ticket`.
//!
//! Generates a legitimate export ticket, then constructs forged and mutated
//! variants (random nonces, extended expiry) and replays the genuine ticket.
//!
//! # Invariants checked
//! 1. A freshly-issued ticket is valid once (`validate_export_ticket` → true).
//! 2. A replay of an already-consumed ticket always returns false.
//! 3. A ticket with a nonce that was never issued always returns false.
//! 4. A ticket with a tampered `expires_at` always returns false.
//! 5. A ticket whose timestamp is in the past returns false.
//! 6. No combination of inputs panics.
#![no_main]

use libfuzzer_sys::fuzz_target;
use patient_registry::{ExportTicket, MedicalRegistry, MedicalRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env,
};

fuzz_target!(|data: &[u8]| {
    // Layout (minimum 40 bytes):
    // [0..4]   base_ts u32 le    — ledger timestamp
    // [4..8]   delta u32 le      — added to issued_at to form expires_at (tests expiry)
    // [8..40]  nonce_mask [u8; 32] — XOR mask applied to the nonce bytes
    if data.len() < 40 {
        return;
    }

    let base_ts = u32::from_le_bytes(data[0..4].try_into().unwrap()) as u64;
    let delta = u32::from_le_bytes(data[4..8].try_into().unwrap()) as u64;
    let nonce_mask: [u8; 32] = data[8..40].try_into().unwrap();

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

    // ── Invariant 1: freshly issued ticket must be valid (once) ───────────────
    let ticket = client.request_data_export(&patient);
    let forged_base = ticket.clone();

    // ── Invariant 3: nonce never issued → false ───────────────────────────────
    // XOR-ing the nonce with the fuzz mask yields a nonce the contract never
    // stored (unless the mask is all-zero, which is skipped).
    if !nonce_mask.iter().all(|&b| b == 0) {
        let orig_nonce: [u8; 32] = ticket.nonce.to_array();
        let mut mutated = [0u8; 32];
        for (i, (&orig, &mask)) in orig_nonce.iter().zip(nonce_mask.iter()).enumerate() {
            mutated[i] = orig ^ mask;
        }
        if mutated != orig_nonce {
            let forged = ExportTicket {
                nonce: BytesN::from_array(&env, &mutated),
                ..forged_base.clone()
            };
            assert!(
                !client.validate_export_ticket(&forged),
                "ticket with unissued nonce must not validate"
            );
        }
    }

    // ── Invariant 4: extended expires_at (self-issued TTL forgery) → false ────
    let extended = ExportTicket {
        expires_at: u64::MAX,
        ..forged_base.clone()
    };
    assert!(
        !client.validate_export_ticket(&extended),
        "ticket with tampered expires_at must not validate"
    );

    assert!(
        client.validate_export_ticket(&ticket),
        "freshly issued ticket must validate as true"
    );

    // ── Invariant 2: replay of a consumed ticket → false ──────────────────────
    assert!(
        !client.validate_export_ticket(&ticket),
        "consumed ticket must not validate again"
    );

    // ── Invariant 5: expires_at in the past → false ───────────────────────────
    // Issue a fresh ticket and move the ledger past its expiry.
    let fresh = client.request_data_export(&patient);
    env.ledger().with_mut(|l| {
        l.timestamp = fresh.expires_at.saturating_add(delta.max(1));
    });
    assert!(
        !client.validate_export_ticket(&fresh),
        "expired ticket must not validate"
    );
});
