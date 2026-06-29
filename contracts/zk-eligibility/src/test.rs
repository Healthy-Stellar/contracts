#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, Vec};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, Address, ZkEligibilityClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(ZkEligibility, ());
    let client = ZkEligibilityClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, admin, client)
}

/// VK whose first byte is `tag`.
fn vk(env: &Env, tag: u8) -> Bytes {
    Bytes::from_slice(env, &[tag, 0xAB, 0xCD])
}

/// Proof whose first byte is `tag` (matches VK with same tag → valid).
fn proof(env: &Env, tag: u8) -> Bytes {
    Bytes::from_slice(env, &[tag, 0x01, 0x02, 0x03])
}

fn empty_inputs(env: &Env) -> Vec<BytesN<32>> {
    Vec::new(env)
}

fn bundle(env: &Env, tag: u8, schema_version: u32) -> ProofBundle {
    ProofBundle {
        proof: proof(env, tag),
        public_inputs: empty_inputs(env),
        schema_version,
    }
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn test_double_initialize_returns_error() {
    let (_, admin, client) = setup();
    let err = client.try_initialize(&admin).unwrap_err().unwrap();
    assert_eq!(err, Error::AlreadyInitialized);
}

#[test]
fn test_call_before_init_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(ZkEligibility, ());
    let client = ZkEligibilityClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let err = client
        .try_register_verifier_key(&admin, &1u32, &vk(&env, 0xAA))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotInitialized);
}

// ── verifier key management ───────────────────────────────────────────────────

#[test]
fn test_register_and_get_verifier_key() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    let entry = client.get_verifier_key(&1u32);
    assert_eq!(entry.schema_version, 1);
    assert!(entry.active);
}

#[test]
fn test_duplicate_schema_returns_error() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    let err = client
        .try_register_verifier_key(&admin, &1u32, &vk(&env, 0xBB))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::SchemaAlreadyExists);
}

#[test]
fn test_non_admin_register_returns_error() {
    let (env, _, client) = setup();
    let stranger = Address::generate(&env);
    let err = client
        .try_register_verifier_key(&stranger, &1u32, &vk(&env, 0xAA))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn test_deprecate_verifier_key() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    client.deprecate_verifier_key(&admin, &1u32);
    let entry = client.get_verifier_key(&1u32);
    assert!(!entry.active);
}

#[test]
fn test_deprecate_unknown_schema_returns_error() {
    let (_, admin, client) = setup();
    let err = client
        .try_deprecate_verifier_key(&admin, &99u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::SchemaNotFound);
}

// ── verify_eligibility: happy path ────────────────────────────────────────────

#[test]
fn test_valid_proof_accepted() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    let subject = Address::generate(&env);
    client.verify_eligibility(&subject, &bundle(&env, 0xAA, 1));
}

// ── verify_eligibility: failure paths ────────────────────────────────────────

#[test]
fn test_invalid_proof_returns_error() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    let subject = Address::generate(&env);
    // proof tag 0xBB ≠ vk tag 0xAA → verification fails
    let bad_bundle = ProofBundle {
        proof: proof(&env, 0xBB),
        public_inputs: empty_inputs(&env),
        schema_version: 1,
    };
    let err = client
        .try_verify_eligibility(&subject, &bad_bundle)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::VerificationFailed);
}

#[test]
fn test_unknown_schema_returns_error() {
    let (env, _, client) = setup();
    let subject = Address::generate(&env);
    let err = client
        .try_verify_eligibility(&subject, &bundle(&env, 0xAA, 99))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::SchemaNotFound);
}

#[test]
fn test_deprecated_schema_returns_error() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    client.deprecate_verifier_key(&admin, &1u32);
    let subject = Address::generate(&env);
    let err = client
        .try_verify_eligibility(&subject, &bundle(&env, 0xAA, 1))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::SchemaNotFound);
}

#[test]
fn test_proof_replay_returns_error() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    let subject = Address::generate(&env);
    client.verify_eligibility(&subject, &bundle(&env, 0xAA, 1));
    // Same proof submitted again
    let err = client
        .try_verify_eligibility(&subject, &bundle(&env, 0xAA, 1))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ProofAlreadyUsed);
}

#[test]
fn test_proof_too_large_returns_error() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    let subject = Address::generate(&env);
    let big_proof = Bytes::from_slice(&env, &[0xAA; 513]);
    let bad_bundle = ProofBundle {
        proof: big_proof,
        public_inputs: empty_inputs(&env),
        schema_version: 1,
    };
    let err = client
        .try_verify_eligibility(&subject, &bad_bundle)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ProofTooLarge);
}

#[test]
fn test_too_many_public_inputs_returns_error() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    let subject = Address::generate(&env);
    let mut inputs: Vec<BytesN<32>> = Vec::new(&env);
    for _ in 0..=MAX_PUBLIC_INPUTS {
        inputs.push_back(BytesN::from_array(&env, &[0u8; 32]));
    }
    let bad_bundle = ProofBundle {
        proof: proof(&env, 0xAA),
        public_inputs: inputs,
        schema_version: 1,
    };
    let err = client
        .try_verify_eligibility(&subject, &bad_bundle)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::TooManyPublicInputs);
}

// ── nullifier ─────────────────────────────────────────────────────────────────

#[test]
fn test_nullifier_recorded_after_valid_proof() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    let subject = Address::generate(&env);
    let p = proof(&env, 0xAA);
    let proof_hash: BytesN<32> = env.crypto().sha256(&p).into();
    assert!(!client.is_nullified(&proof_hash));
    client.verify_eligibility(&subject, &bundle(&env, 0xAA, 1));
    assert!(client.is_nullified(&proof_hash));
}

// ── schema versioning ─────────────────────────────────────────────────────────

#[test]
fn test_multiple_schema_versions_coexist() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    client.register_verifier_key(&admin, &2u32, &vk(&env, 0xBB));

    let subject = Address::generate(&env);
    // v1 proof
    client.verify_eligibility(&subject, &bundle(&env, 0xAA, 1));
    // v2 proof (different proof bytes → different nullifier)
    client.verify_eligibility(&subject, &bundle(&env, 0xBB, 2));
}
