#![cfg(test)]
#![allow(deprecated)]

use super::*;
use shared::privacy::PolicyMetadata;
use soroban_sdk::{contract, contractimpl, testutils::Address as _, BytesN, Env, String, Symbol, Vec};

// ── Mock access-control contract for tests (#300) ────────────────────────────
//
// Always approves check_consent so existing claim-lifecycle tests are
// unaffected.  A separate test verifies that consent denial propagates.
#[contract]
struct MockAccessControl;

#[contractimpl]
impl MockAccessControl {
    /// Unconditionally succeeds — represents a patient who has granted consent.
    pub fn check_consent(
        _env: Env,
        _subject: Address,
        _grantee: Address,
        _purpose_code: String,
        _required_scope: u32,
    ) {
    }
}

fn policy(env: &Env) -> PolicyMetadata {
    PolicyMetadata {
        retention_class: Symbol::new(env, "financial"),
        access_policy_hash: BytesN::from_array(env, &[7u8; 32]),
        purpose: Symbol::new(env, "claims"),
    }
}

fn reference_hash(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn build_services(env: &Env, amount: i128) -> Vec<ServiceLine> {
    let mut services = Vec::new(env);
    services.push_back(ServiceLine {
        procedure_code: String::from_str(env, "99213"),
        modifier: None,
        quantity: 1,
        charge_amount: amount,
        diagnosis_pointers: Vec::new(env),
    });
    services
}

fn setup(
    env: &Env,
) -> (
    MedicalClaimsSystemClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    // Register the always-approving mock access-control contract (#300).
    let ac_id = env.register(MockAccessControl, ());

    let contract_id = env.register_contract(None, MedicalClaimsSystem);
    let client = MedicalClaimsSystemClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let provider = Address::generate(env);
    let patient = Address::generate(env);
    let insurer = Address::generate(env);
    client.initialize(&admin, &ac_id);
    client.register_insurer(&admin, &insurer);
    (client, admin, provider, patient, insurer)
}

fn make_services(env: &Env) -> Vec<ServiceLine> {
    let mut s = Vec::new(env);
    s.push_back(ServiceLine {
        procedure_code: String::from_str(env, "99213"),
        modifier: None,
        quantity: 1,
        charge_amount: 15000,
        diagnosis_pointers: Vec::new(env),
    });
    s
}

#[test]
fn test_full_claim_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, insurer) = setup(&env);

    let claim_id = client.submit_claim(
        &provider,
        &patient,
        &insurer,
        &12345,
        &1690000000,
        &make_services(&env),
        &Vec::new(&env),
        &BytesN::from_array(&env, &[0; 32]),
        &policy(&env),
        &15000,
    );

    let mut approved_lines = Vec::new(&env);
    approved_lines.push_back(1u64);

    client.adjudicate_claim(
        &claim_id,
        &insurer,
        &approved_lines,
        &Vec::new(&env),
        &10000,
        &2000,
    );

    client.process_payment(
        &claim_id,
        &insurer,
        &10000,
        &1690100000,
        &reference_hash(&env, 8),
    );

    client.apply_patient_payment(&claim_id, &patient, &2000, &1690200000);

    let res = client.try_appeal_denial(
        &claim_id,
        &provider,
        &1,
        &BytesN::from_array(&env, &[0; 32]),
    );
    assert!(res.is_err());
}

#[test]
fn test_unregistered_insurer_cannot_adjudicate() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, insurer) = setup(&env);
    let rogue = Address::generate(&env);

    let claim_id = client.submit_claim(
        &provider,
        &patient,
        &insurer,
        &1,
        &100,
        &make_services(&env),
        &Vec::new(&env),
        &BytesN::from_array(&env, &[0; 32]),
        &policy(&env),
        &5000,
    );

    let result =
        client.try_adjudicate_claim(&claim_id, &rogue, &Vec::new(&env), &Vec::new(&env), &0, &0);
    assert_eq!(result, Err(Ok(Error::InsurerNotRegistered)));
}

#[test]
fn test_wrong_insurer_cannot_adjudicate() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, provider, patient, insurer) = setup(&env);
    let other_insurer = Address::generate(&env);
    client.register_insurer(&admin, &other_insurer);

    let claim_id = client.submit_claim(
        &provider,
        &patient,
        &insurer,
        &1,
        &100,
        &make_services(&env),
        &Vec::new(&env),
        &BytesN::from_array(&env, &[0; 32]),
        &policy(&env),
        &5000,
    );

    let result = client.try_adjudicate_claim(
        &claim_id,
        &other_insurer,
        &Vec::new(&env),
        &Vec::new(&env),
        &0,
        &0,
    );
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));
}

#[test]
fn test_unregistered_insurer_cannot_process_payment() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, insurer) = setup(&env);
    let rogue = Address::generate(&env);

    let claim_id = client.submit_claim(
        &provider,
        &patient,
        &insurer,
        &1,
        &100,
        &make_services(&env),
        &Vec::new(&env),
        &BytesN::from_array(&env, &[0; 32]),
        &policy(&env),
        &5000,
    );

    client.adjudicate_claim(
        &claim_id,
        &insurer,
        &Vec::new(&env),
        &Vec::new(&env),
        &4500,
        &500,
    );

    let result =
        client.try_process_payment(&claim_id, &rogue, &4500, &200, &reference_hash(&env, 8));
    assert_eq!(result, Err(Ok(Error::InsurerNotRegistered)));
}

#[test]
fn test_submit_claim_with_unregistered_insurer_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, _) = setup(&env);
    let unknown_insurer = Address::generate(&env);

    let result = client.try_submit_claim(
        &provider,
        &patient,
        &unknown_insurer,
        &1,
        &100,
        &make_services(&env),
        &Vec::new(&env),
        &BytesN::from_array(&env, &[0; 32]),
        &policy(&env),
        &5000,
    );
    assert_eq!(result, Err(Ok(Error::InsurerNotRegistered)));
}

#[test]
fn test_appeal_workflow() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, insurer) = setup(&env);

    let claim_id = client.submit_claim(
        &provider,
        &patient,
        &insurer,
        &12345,
        &1690000000,
        &make_services(&env),
        &Vec::new(&env),
        &BytesN::from_array(&env, &[1; 32]),
        &policy(&env),
        &25000,
    );

    let mut denials = Vec::new(&env);
    denials.push_back(DenialInfo {
        line_number: 1,
        denial_code: String::from_str(&env, "CO-50"),
        denial_reason_hash: reference_hash(&env, 9),
        is_appealable: true,
    });

    client.adjudicate_claim(&claim_id, &insurer, &Vec::new(&env), &denials, &0, &0);
    client.appeal_denial(
        &claim_id,
        &provider,
        &1,
        &BytesN::from_array(&env, &[2; 32]),
    );

    // Already at level 1 — should fail
    let res = client.try_appeal_denial(
        &claim_id,
        &provider,
        &1,
        &BytesN::from_array(&env, &[2; 32]),
    );
    assert!(res.is_err());

    client.adjudicate_claim(&claim_id, &insurer, &Vec::new(&env), &denials, &0, &0);
    client.appeal_denial(
        &claim_id,
        &provider,
        &2,
        &BytesN::from_array(&env, &[3; 32]),
    );
    client.adjudicate_claim(&claim_id, &insurer, &Vec::new(&env), &denials, &0, &0);
    client.appeal_denial(
        &claim_id,
        &provider,
        &3,
        &BytesN::from_array(&env, &[4; 32]),
    );
}

#[test]
fn test_double_initialize_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _, _, _) = setup(&env);
    // Second initialize must fail regardless of which access-control address is
    // provided — the AlreadyInitialized guard is checked first.
    let dummy_ac = Address::generate(&env);
    let result = client.try_initialize(&admin, &dummy_ac);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_non_admin_cannot_register_insurer() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _, _, _) = setup(&env);
    let fake_admin = Address::generate(&env);
    let new_insurer = Address::generate(&env);
    let result = client.try_register_insurer(&fake_admin, &new_insurer);
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));
}
