#![cfg(test)]
#![allow(deprecated)]

use super::*;
use insurer_registry::{InsurerRegistry, InsurerRegistryClient, MAX_CREDENTIAL_LIFETIME};
use shared::privacy::PolicyMetadata;
use soroban_sdk::{contract, contractimpl, testutils::{Address as _, Ledger}, BytesN, Env, String, Symbol, Vec};

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

fn dummy_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn register_active_insurer(env: &Env, insurer: &Address) -> Address {
    env.mock_all_auths();
    let registry_admin = Address::generate(env);
    let ir_id = env.register(InsurerRegistry, (registry_admin.clone(),));
    let ir_client = InsurerRegistryClient::new(env, &ir_id);
    let issuer = Address::generate(env);
    ir_client.set_trusted_issuer(&registry_admin, &issuer, &true);
    ir_client.register_insurer(
        insurer,
        &String::from_str(env, "Claims Insurer"),
        &String::from_str(env, "LIC-CLAIMS"),
        &String::from_str(env, "metadata"),
        &dummy_hash(env, 1),
        &issuer,
        &dummy_hash(env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(env, 3),
    );
    ir_id
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
    // Mock financial records contract (not needed for these tests)
    let fr_id = Address::generate(env);

    let admin = Address::generate(env);
    let provider = Address::generate(env);
    let patient = Address::generate(env);
    let insurer = Address::generate(env);
    let ir_id = register_active_insurer(env, &insurer);

    let contract_id = env.register(
        MedicalClaimsSystem,
        (&admin, &ac_id, &fr_id, &86400u64, &ir_id),
    );
    let client = MedicalClaimsSystemClient::new(env, &contract_id);
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
        &15000,
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
        &15000,
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
        &15000,
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
        &15000,
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
        &15000,
    );

    let mut denials = Vec::new(&env);
    denials.push_back(DenialInfo {
        line_num: 1,
        reason_code: String::from_str(&env, "CO-16"),
        denial_date: 1690000000,
    });

    client.adjudicate_claim(
        &claim_id,
        &insurer,
        &Vec::new(&env),
        &denials,
        &0,
        &0,
    );

    client.appeal_denial(
        &claim_id,
        &provider,
        &1,
        &BytesN::from_array(&env, &[2; 32]),
    );

    // Re-adjudicate after appeal with a valid approval.
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

    let claim = client.get_claim(&claim_id);
    assert_eq!(claim.status, ClaimStatus::Adjudicated);
    assert_eq!(claim.approved_amount, 10000);
    assert_eq!(claim.patient_responsibility, 2000);
}

// ── #1066: re-adjudication must not drop below amounts already paid ──────────

#[test]
fn test_readjudication_cannot_lower_approved_below_insurer_paid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, insurer) = setup(&env);

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

    // Insurer pays 8000 of the 10000 approved.
    client.process_payment(
        &claim_id,
        &insurer,
        &8000,
        &1690100000,
        &reference_hash(&env, 8),
    );

    // Provider appeals; insurer tries to re-adjudicate below what it paid.
    client.appeal_denial(
        &claim_id,
        &provider,
        &1,
        &BytesN::from_array(&env, &[2; 32]),
    );

    let result = client.try_adjudicate_claim(
        &claim_id,
        &insurer,
        &approved_lines,
        &Vec::new(&env),
        &5000,
        &2000,
    );
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    // Re-adjudicating at or above the paid amount succeeds.
    client.adjudicate_claim(
        &claim_id,
        &insurer,
        &approved_lines,
        &Vec::new(&env),
        &8000,
        &2000,
    );
    let claim = client.get_claim(&claim_id);
    assert_eq!(claim.approved_amount, 8000);
    assert!(claim.approved_amount >= claim.insurer_paid_amount);
}

#[test]
fn test_readjudication_cannot_lower_patient_resp_below_patient_paid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, insurer) = setup(&env);

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

    client.apply_patient_payment(&claim_id, &patient, &2000, &1690200000);

    client.appeal_denial(
        &claim_id,
        &provider,
        &1,
        &BytesN::from_array(&env, &[2; 32]),
    );

    // Re-adjudication lowering patient responsibility below what was paid fails.
    let result = client.try_adjudicate_claim(
        &claim_id,
        &insurer,
        &approved_lines,
        &Vec::new(&env),
        &10000,
        &1000,
    );
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_patient_payment_cannot_exceed_responsibility() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, insurer) = setup(&env);

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
        &15000,
    );

    client.adjudicate_claim(
        &claim_id,
        &insurer,
        &Vec::new(&env),
        &Vec::new(&env),
        &10000,
        &2000,
    );

    // Overpaying beyond patient responsibility is rejected.
    let result = client.try_apply_patient_payment(&claim_id, &patient, &2500, &1690200000);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    // Paying exactly the responsibility succeeds and closes the patient side.
    client.apply_patient_payment(&claim_id, &patient, &2000, &1690200000);
    let claim = client.get_claim(&claim_id);
    assert_eq!(claim.patient_paid_amount, 2000);
    assert!(claim.patient_responsibility >= claim.patient_paid_amount);
}

#[test]
fn test_dues_non_negative_after_payment_sequence() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, provider, patient, insurer) = setup(&env);

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
        &4000,
        &1690100000,
        &reference_hash(&env, 8),
    );
    client.apply_patient_payment(&claim_id, &patient, &1000, &1690200000);

    // Appeal and re-adjudicate at exactly the paid amounts.
    client.appeal_denial(
        &claim_id,
        &provider,
        &1,
        &BytesN::from_array(&env, &[2; 32]),
    );
    client.adjudicate_claim(
        &claim_id,
        &insurer,
        &approved_lines,
        &Vec::new(&env),
        &4000,
        &1000,
    );

    let claim = client.get_claim(&claim_id);
    let insurer_due = claim.approved_amount - claim.insurer_paid_amount;
    let patient_due = claim.patient_responsibility - claim.patient_paid_amount;
    assert!(insurer_due >= 0);
    assert!(patient_due >= 0);
}
