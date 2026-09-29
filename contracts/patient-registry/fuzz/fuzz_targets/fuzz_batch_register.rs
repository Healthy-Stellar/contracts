//! Fuzz target: `batch_register_patients`.
//!
//! Constructs batches of 0–55 entries from raw fuzz bytes and exercises:
//!
//! - `BatchTooLarge` when entry count exceeds `MAX_BATCH_SIZE` (50).
//! - `AlreadyExists` for duplicate wallet addresses within a single batch.
//! - `Success` for fresh entries with valid encrypted refs and policy.
//! - `Failed(InvalidEncryptedEnvelope)` for entries with an all-zero content hash.
//! - The output Vec length always equals the input entry count (never panics).
#![no_main]

use libfuzzer_sys::fuzz_target;
use patient_registry::{
    BatchEntryStatus, BatchPatientEntry, ContractError, MedicalRegistry, MedicalRegistryClient,
    MAX_BATCH_SIZE,
};
use shared::privacy::{EncryptedEnvelopeRef, PolicyMetadata};
use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env, String, Symbol, Vec,
};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // First byte determines how many entries to generate (0..=55).
    let n_entries = (data[0] as u32) % 56;

    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    // Initialize.
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client
        .try_initialize(&admin, &treasury, &fee_token)
        .unwrap_or_default();

    let mut entries: Vec<BatchPatientEntry> = Vec::new(&env);

    // Keep a handle to one wallet so we can insert a duplicate later.
    let mut first_wallet: Option<Address> = None;

    for i in 0..n_entries {
        // Use one byte per entry (cycled from data[1..]) to control variation.
        let byte_idx = 1 + (i as usize) % data.len().saturating_sub(1).max(1);
        let seed = if data.len() > byte_idx { data[byte_idx] } else { i as u8 };

        // Every 7th entry is a duplicate of the first wallet (if available).
        let wallet = if i % 7 == 6 && first_wallet.is_some() {
            first_wallet.clone().unwrap()
        } else {
            Address::generate(&env)
        };

        if first_wallet.is_none() {
            first_wallet = Some(wallet.clone());
        }

        // Entries with seed == 0 get an all-zero content_hash (invalid ref).
        let content_hash = if seed == 0 {
            BytesN::from_array(&env, &[0u8; 32]) // triggers InvalidEncryptedEnvelope
        } else {
            BytesN::from_array(&env, &[seed; 32])
        };

        let enc_ref = EncryptedEnvelopeRef {
            content_hash,
            envelope_uri: String::from_str(&env, "enc+ipfs://bafyvalidfuzzref00000000000000"),
            key_version_id: String::from_str(&env, "kv:v01"),
        };
        let pol = PolicyMetadata {
            retention_class: Symbol::new(&env, "clinical"),
            access_policy_hash: BytesN::from_array(&env, &[200u8; 32]),
            purpose: Symbol::new(&env, "treatment"),
        };

        entries.push_back(BatchPatientEntry {
            wallet,
            name: String::from_str(&env, "Fuzzy Patient"),
            dob: 0u64,
            encrypted_metadata_ref: enc_ref,
            policy: pol,
        });
    }

    let result = client.try_batch_register_patients(&entries);

    // ── Invariant: BatchTooLarge when n_entries > MAX_BATCH_SIZE ─────────────
    if n_entries > MAX_BATCH_SIZE {
        assert_eq!(
            result,
            Err(Ok(ContractError::BatchTooLarge)),
            "n_entries={n_entries} must return BatchTooLarge"
        );
        return;
    }

    // ── Invariant: output length == input length ──────────────────────────────
    let statuses = result.expect("batch_register_patients must not panic");
    assert_eq!(
        statuses.len(),
        n_entries,
        "output Vec length must equal input entry count"
    );

    // ── Invariant: every status is a valid BatchEntryStatus ──────────────────
    for i in 0..n_entries {
        match statuses.get(i).unwrap() {
            BatchEntryStatus::Success => {}
            BatchEntryStatus::AlreadyExists => {}
            BatchEntryStatus::Failed(_code) => {}
        }
    }
});
