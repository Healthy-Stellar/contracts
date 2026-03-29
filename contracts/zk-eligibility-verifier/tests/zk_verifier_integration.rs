//! Integration-style tests for the ZK eligibility verifier scaffolding.
//!
//! Expand this module when a circuit, VK layout, and proof byte format exist.
//! See `INTERFACE.md` in this crate for the expected end-to-end pipeline.

use soroban_sdk::{testutils::Address as _, Address, Bytes, Env, Vec};
use zk_eligibility_verifier::{
    verify_eligibility_proof, PlaceholderZkProofVerifier, RUST_INTERFACE_VERSION, ZKProofVerifier,
};

#[test]
fn rust_interface_version_is_documented_stub() {
    assert!(
        RUST_INTERFACE_VERSION.contains("stub"),
        "bump RUST_INTERFACE_VERSION when the trait API or encoding contract changes"
    );
}

#[test]
fn placeholder_verifier_returns_false_for_empty_proof() {
    let env = Env::default();
    let patient = Address::generate(&env);
    let proof = Bytes::new(&env);
    let public_inputs = Vec::new(&env);

    let verifier = PlaceholderZkProofVerifier;
    assert!(
        !verifier.verify_eligibility_proof(patient.clone(), proof.clone(), public_inputs.clone()),
        "placeholder must reject until real crypto is implemented"
    );
    assert!(
        !verify_eligibility_proof(&verifier, patient, proof, public_inputs),
        "free function must match PlaceholderZkProofVerifier"
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

// ============================================================================
// Skeleton — enable when ZK artifacts and test vectors are available in CI
// ============================================================================
//
// #[test]
// fn valid_proof_accepts_eligibility() {
//     let env = Env::default();
//     let patient = Address::generate(&env);
//     let (proof, public_inputs) = load_fixture(&env, "fixtures/eligibility_ok_v1.bin");
//     let verifier = ProductionVerifier::from_env(&env, VK_ID_V1);
//     assert!(verify_eligibility_proof(&verifier, patient, proof, public_inputs));
// }
//
// #[test]
// fn tampered_public_input_rejects() {
//     let env = Env::default();
//     let patient = Address::generate(&env);
//     let (mut proof, mut public_inputs) = load_fixture(&env, "fixtures/eligibility_ok_v1.bin");
//     flip_last_byte_of_first_public_input(&mut public_inputs);
//     let verifier = ProductionVerifier::from_env(&env, VK_ID_V1);
//     assert!(!verify_eligibility_proof(&verifier, patient, proof, public_inputs));
// }
//
// #[test]
// fn wrong_patient_binding_rejects() {
//     let env = Env::default();
//     let patient_a = Address::generate(&env);
//     let patient_b = Address::generate(&env);
//     let (proof, public_inputs) = load_fixture_for_patient(&env, patient_a, "...");
//     let verifier = ProductionVerifier::from_env(&env, VK_ID_V1);
//     assert!(!verify_eligibility_proof(&verifier, patient_b, proof, public_inputs));
// }
//
// #[test]
// #[ignore = "requires prover daemon + circuit release artifact"]
// fn soroban_contract_end_to_end() {
//     // deploy wrapper contract that stores VK hash and calls ZKProofVerifier impl
// }
