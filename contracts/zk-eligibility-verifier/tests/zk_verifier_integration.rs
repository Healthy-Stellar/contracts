//! Integration-style tests for the ZK eligibility verifier scaffolding.
//!
//! These tests document the **intended** end-to-end flow once a real proof system
//! exists. Expand this file when:
//! - A circuit artifact and byte layout for `proof` / `public_inputs` are frozen.
//! - A test vector generator produces valid proofs in CI.
//! - Optional: a thin Soroban contract wraps [`ZKProofVerifier`] behind `contractimpl`.

use soroban_sdk::{testutils::Address as _, Address, Bytes, Env, Vec};
use zk_eligibility_verifier::{PlaceholderZkProofVerifier, ZKProofVerifier};

#[test]
fn placeholder_verifier_returns_false_for_empty_proof() {
    let env = Env::default();
    let patient = Address::generate(&env);
    let proof = Bytes::new(&env);
    let public_inputs = Vec::new(&env);

    let verifier = PlaceholderZkProofVerifier;
    assert!(
        !verifier.verify_eligibility_proof(patient, proof, public_inputs),
        "placeholder must reject until real crypto is implemented"
    );
}

#[test]
fn placeholder_verifier_returns_false_with_nonempty_inputs() {
    let env = Env::default();
    let patient = Address::generate(&env);
    let proof = Bytes::from_slice(&env, &[0xde, 0xad, 0xbe, 0xef]);
    let mut public_inputs = Vec::new(&env);
    public_inputs.push_back(Bytes::from_slice(&env, &[1, 2, 3]));

    let verifier = PlaceholderZkProofVerifier;
    assert!(!verifier.verify_eligibility_proof(patient, proof, public_inputs));
}

// ---------------------------------------------------------------------------
// Skeleton: replace with real integration tests when the ZK stack is available
// ---------------------------------------------------------------------------
//
// #[test]
// fn valid_proof_accepts_eligibility() {
//     let env = Env::default();
//     let patient = Address::generate(&env);
//     let (proof, public_inputs) = load_official_test_vector(&env, "v1/eligibility_ok.bin");
//     let verifier = ProductionGroth16Verifier::new(&env, VK_ID_V1);
//     assert!(verifier.verify_eligibility_proof(patient, proof, public_inputs));
// }
//
// #[test]
// fn tampered_public_input_rejects() { ... }
//
// #[test]
// fn wrong_patient_binding_rejects() { ... }
