#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _, testutils::Ledger as _, Address, Bytes, BytesN, Env, Vec,
};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Default ledger timestamp used across all tests (well in the past so we can
/// set expiry relative to it). We start at a non-zero value to avoid
/// ambiguous zero comparisons.
const BASE_TIMESTAMP: u64 = 1_000_000;

/// A future expiry that is always valid relative to BASE_TIMESTAMP.
const FUTURE_EXPIRY: u64 = BASE_TIMESTAMP + 86_400;

fn setup() -> (Env, Address, ZkEligibilityClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    // Pin the ledger timestamp so our expiry values are deterministic.
    env.ledger().with_mut(|l| {
        l.timestamp = BASE_TIMESTAMP;
    });
    let contract_id = env.register(ZkEligibility, ());
    let client = ZkEligibilityClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, admin, client)
}

/// VK bytes: 32 bytes with the first byte set to `tag`.
fn vk(env: &Env, tag: u8) -> Bytes {
    let mut raw = [0u8; 32];
    raw[0] = tag;
    raw[1] = 0xAB;
    raw[2] = 0xCD;
    Bytes::from_slice(env, &raw)
}

/// Encode `ts` as a big-endian u64 in the first 8 bytes of a 32-byte scalar.
fn expiry_scalar(env: &Env, ts: u64) -> BytesN<32> {
    let mut raw = [0u8; 32];
    raw[0] = (ts >> 56) as u8;
    raw[1] = (ts >> 48) as u8;
    raw[2] = (ts >> 40) as u8;
    raw[3] = (ts >> 32) as u8;
    raw[4] = (ts >> 24) as u8;
    raw[5] = (ts >> 16) as u8;
    raw[6] = (ts >>  8) as u8;
    raw[7] =  ts        as u8;
    BytesN::from_array(env, &raw)
}

/// Build a public inputs vector with `expiry` at index 0.
fn inputs_with_expiry(env: &Env, expiry: u64) -> Vec<BytesN<32>> {
    let mut v: Vec<BytesN<32>> = Vec::new(env);
    v.push_back(expiry_scalar(env, expiry));
    v
}

/// Compute the 4-byte stub commitment tag for a given `(vk_bytes, inputs)` pair.
///
/// commitment = SHA-256( vk_bytes || input_0 || … || input_n )
/// tag        = commitment[0..4]
///
/// This mirrors the logic inside `run_verification` exactly so tests can build
/// well-formed proofs without copy-pasting the algorithm.
fn commitment_tag(env: &Env, vk_bytes: &Bytes, inputs: &Vec<BytesN<32>>) -> [u8; 4] {
    let mut buf = Bytes::new(env);
    buf.append(vk_bytes);
    for i in 0..inputs.len() {
        let scalar: BytesN<32> = inputs.get(i).unwrap();
        buf.append(&Bytes::from_slice(env, &scalar.to_array()));
    }
    let digest: BytesN<32> = env.crypto().sha256(&buf).into();
    [
        digest.get(0).unwrap(),
        digest.get(1).unwrap(),
        digest.get(2).unwrap(),
        digest.get(3).unwrap(),
    ]
}

/// Build a well-formed proof for `(vk_bytes, inputs)`.
///
/// Layout: [ 0xDE, 0xAD, 0xBE, 0xEF, 0x00 (5-byte payload) ][ commitment_tag (4 bytes) ]
/// Total: 9 bytes — well under MAX_PROOF_BYTES and satisfies the ≥5-byte minimum.
fn make_proof(env: &Env, vk_bytes: &Bytes, inputs: &Vec<BytesN<32>>) -> Bytes {
    let tag = commitment_tag(env, vk_bytes, inputs);
    let raw: [u8; 9] = [
        0xDE, 0xAD, 0xBE, 0xEF, 0x00,  // arbitrary payload
        tag[0], tag[1], tag[2], tag[3], // 4-byte commitment tag
    ];
    Bytes::from_slice(env, &raw)
}

/// Build a proof with a deliberately wrong commitment tag (last 4 bytes are
/// all 0xFF, which will not match any real commitment).
fn make_invalid_proof(env: &Env) -> Bytes {
    Bytes::from_slice(env, &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF])
}

/// Convenience: a complete, valid ProofBundle for `(vk_tag, schema_version)`.
fn valid_bundle(env: &Env, vk_bytes: &Bytes, schema_version: u32) -> ProofBundle {
    let inputs = inputs_with_expiry(env, FUTURE_EXPIRY);
    let proof = make_proof(env, vk_bytes, &inputs);
    ProofBundle { proof, public_inputs: inputs, schema_version }
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
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);
    client.verify_eligibility(&subject, &valid_bundle(&env, &vk_bytes, 1));
}

#[test]
fn test_is_eligible_true_after_valid_proof() {
    let (env, admin, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);
    assert!(!client.is_eligible(&subject));
    client.verify_eligibility(&subject, &valid_bundle(&env, &vk_bytes, 1));
    assert!(client.is_eligible(&subject));
}

// ── verify_eligibility: failure paths ────────────────────────────────────────

#[test]
fn test_invalid_proof_returns_error() {
    let (env, admin, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);
    // Proof whose commitment tag is wrong → VerificationFailed.
    let inputs = inputs_with_expiry(&env, FUTURE_EXPIRY);
    let bad_bundle = ProofBundle {
        proof: make_invalid_proof(&env),
        public_inputs: inputs,
        schema_version: 1,
    };
    let err = client
        .try_verify_eligibility(&subject, &bad_bundle)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::VerificationFailed);
}

#[test]
fn test_tampered_public_inputs_returns_error() {
    // Proof was computed for input set A; resubmitted with input set B.
    // The commitment will not match → VerificationFailed.
    let (env, admin, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);

    // Build a valid proof bound to FUTURE_EXPIRY inputs.
    let original_inputs = inputs_with_expiry(&env, FUTURE_EXPIRY);
    let proof = make_proof(&env, &vk_bytes, &original_inputs);

    // Submit with a *different* expiry scalar — public inputs no longer match.
    let tampered_inputs = inputs_with_expiry(&env, FUTURE_EXPIRY + 1);
    let bad_bundle = ProofBundle {
        proof,
        public_inputs: tampered_inputs,
        schema_version: 1,
    };
    let err = client
        .try_verify_eligibility(&subject, &bad_bundle)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::VerificationFailed);
}

#[test]
fn test_expired_proof_returns_error() {
    let (env, admin, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);

    // expiry = BASE_TIMESTAMP − 1: already in the past.
    let past_expiry = BASE_TIMESTAMP - 1;
    let inputs = inputs_with_expiry(&env, past_expiry);
    let proof = make_proof(&env, &vk_bytes, &inputs);
    let bad_bundle = ProofBundle { proof, public_inputs: inputs, schema_version: 1 };

    let err = client
        .try_verify_eligibility(&subject, &bad_bundle)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ProofExpired);
}

#[test]
fn test_missing_expiry_input_returns_error() {
    let (env, admin, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);

    // Empty public_inputs: no expiry scalar at index 0.
    let empty: Vec<BytesN<32>> = Vec::new(&env);
    // Proof length ≥ 5 so it passes the structural check, but expiry check fires first.
    let proof = Bytes::from_slice(&env, &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09]);
    let bad_bundle = ProofBundle { proof, public_inputs: empty, schema_version: 1 };

    let err = client
        .try_verify_eligibility(&subject, &bad_bundle)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ProofExpired);
}

#[test]
fn test_unknown_schema_returns_error() {
    let (env, _, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    let subject = Address::generate(&env);
    let err = client
        .try_verify_eligibility(&subject, &valid_bundle(&env, &vk_bytes, 99))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::SchemaNotFound);
}

#[test]
fn test_deprecated_schema_returns_error() {
    let (env, admin, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    client.deprecate_verifier_key(&admin, &1u32);
    let subject = Address::generate(&env);
    let err = client
        .try_verify_eligibility(&subject, &valid_bundle(&env, &vk_bytes, 1))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::SchemaNotFound);
}

#[test]
fn test_proof_replay_returns_error() {
    let (env, admin, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);
    let bundle = valid_bundle(&env, &vk_bytes, 1);
    client.verify_eligibility(&subject, &bundle);
    // Same proof bytes → same nullifier hash → ProofAlreadyUsed.
    let err = client
        .try_verify_eligibility(&subject, &bundle)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ProofAlreadyUsed);
}

#[test]
fn test_proof_too_large_returns_error() {
    let (env, admin, client) = setup();
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);
    let big_proof = Bytes::from_slice(&env, &[0xAA; 513]);
    let bad_bundle = ProofBundle {
        proof: big_proof,
        public_inputs: inputs_with_expiry(&env, FUTURE_EXPIRY),
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
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);
    let mut inputs: Vec<BytesN<32>> = Vec::new(&env);
    // MAX_PUBLIC_INPUTS + 1 entries.
    for _ in 0..=MAX_PUBLIC_INPUTS {
        inputs.push_back(BytesN::from_array(&env, &[0u8; 32]));
    }
    let bad_bundle = ProofBundle {
        proof: Bytes::from_slice(&env, &[0xAA; 9]),
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
    let vk_bytes = vk(&env, 0xAA);
    client.register_verifier_key(&admin, &1u32, &vk_bytes);
    let subject = Address::generate(&env);
    let bundle = valid_bundle(&env, &vk_bytes, 1);
    let proof_hash: BytesN<32> = env.crypto().sha256(&bundle.proof).into();
    assert!(!client.is_nullified(&proof_hash));
    client.verify_eligibility(&subject, &bundle);
    assert!(client.is_nullified(&proof_hash));
}

// ── schema versioning ─────────────────────────────────────────────────────────

#[test]
fn test_multiple_schema_versions_coexist() {
    let (env, admin, client) = setup();
    let vk1 = vk(&env, 0xAA);
    let vk2 = vk(&env, 0xBB);
    client.register_verifier_key(&admin, &1u32, &vk1);
    client.register_verifier_key(&admin, &2u32, &vk2);

    let subject = Address::generate(&env);
    // v1 proof — bound to vk1 + its public inputs.
    client.verify_eligibility(&subject, &valid_bundle(&env, &vk1, 1));
    // v2 proof — different VK → different commitment → different proof bytes → different nullifier.
    client.verify_eligibility(&subject, &valid_bundle(&env, &vk2, 2));
}

#[test]
fn test_migrate_schema_marks_old_schema_as_migrated() {
    let (env, admin, client) = setup();
    client.register_verifier_key(&admin, &1u32, &vk(&env, 0xAA));
    client.register_verifier_key(&admin, &2u32, &vk(&env, 0xBB));
    client.deprecate_verifier_key(&admin, &1u32);

    client.migrate_schema(&admin, &1u32, &2u32, &proof(&env, 0xBB));

    let old_entry = client.get_verifier_key(&1u32);
    assert_eq!(old_entry.migrated_to, 2);
}

// ── admin rotation ────────────────────────────────────────────────────────────

#[test]
fn test_admin_rotation_success() {
    let (env, admin, client) = setup();
    let new_admin = Address::generate(&env);

    client.propose_admin_rotation(&admin, &new_admin);
    client.accept_admin_rotation(&new_admin);

    // New admin can now perform admin-only actions.
    client.register_verifier_key(&new_admin, &1u32, &vk(&env, 0xAA));

    // Old admin is no longer authorized.
    let err = client
        .try_register_verifier_key(&admin, &2u32, &vk(&env, 0xBB))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn test_double_propose_returns_error() {
    let (env, admin, client) = setup();
    let first = Address::generate(&env);
    let second = Address::generate(&env);

    client.propose_admin_rotation(&admin, &first);
    let err = client
        .try_propose_admin_rotation(&admin, &second)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::RotationPending);
}

#[test]
fn test_wrong_address_accept_returns_error() {
    let (env, admin, client) = setup();
    let pending = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);
    let err = client
        .try_accept_admin_rotation(&stranger)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotPendingAdmin);
}

#[test]
fn test_accept_without_proposal_returns_error() {
    let (env, _, client) = setup();
    let stranger = Address::generate(&env);
    let err = client
        .try_accept_admin_rotation(&stranger)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NoRotationPending);
}

#[test]
fn test_accept_after_expiry_returns_error() {
    let (env, admin, client) = setup();
    let pending = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);

    // Advance past the 24 h rotation window.
    env.ledger().with_mut(|l| {
        l.timestamp += ROTATION_TTL + 1;
    });

    let err = client
        .try_accept_admin_rotation(&pending)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::RotationExpired);
}

#[test]
fn test_accept_at_exact_expiry_boundary() {
    let (env, admin, client) = setup();
    let pending = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);

    // Exactly at the expiry boundary the rotation is still valid.
    env.ledger().with_mut(|l| {
        l.timestamp += ROTATION_TTL;
    });

    client.accept_admin_rotation(&pending);
    client.register_verifier_key(&pending, &1u32, &vk(&env, 0xAA));
}
