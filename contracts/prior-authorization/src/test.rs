#![cfg(test)]

use super::*;
use insurer_registry::{CoveragePlan, InsurerRegistry, InsurerRegistryClient};
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, BytesN, Env, String, Symbol, Vec};

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn dummy_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn setup_insurer_registry(env: &Env, insurer: &Address) -> Address {
    let ir_id = env.register_contract(None, InsurerRegistry);
    let ir_client = InsurerRegistryClient::new(env, &ir_id);
    let issuer = Address::generate(env);
    ir_client.register_insurer(
        insurer,
        &String::from_str(env, "Test Insurer"),
        &String::from_str(env, "LIC-001"),
        &String::from_str(env, "metadata"),
        &dummy_hash(env, 1),
        &issuer,
        &dummy_hash(env, 2),
        &4_100_000_000_u64,
        &dummy_hash(env, 3),
    );

    let mut service_codes = Vec::new(env);
    service_codes.push_back(String::from_str(env, "CPT99213"));
    let mut plans = Vec::new(env);
    plans.push_back(CoveragePlan {
        plan_id: 1,
        plan_name: String::from_str(env, "PPO Gold"),
        service_codes,
        is_active: true,
        effective_from: 0,
        effective_until: None,
    });
    ir_client.set_coverage_plans(insurer, &plans);
    ir_id
}

fn setup() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let insurer = Address::generate(&env);
    (env, provider, patient, insurer)
}

fn setup_client(env: &Env, insurer: &Address) -> PriorAuthorizationContractClient {
    let ir_id = setup_insurer_registry(env, insurer);
    register_contract(env, &ir_id)
}

fn register_contract(env: &Env, insurer_registry_id: &Address) -> PriorAuthorizationContractClient {
    let contract_id = env.register(PriorAuthorizationContract, ());
    let client = PriorAuthorizationContractClient::new(env, &contract_id);
    client.initialize(insurer_registry_id);
    client
}

fn submit(
    env: &Env,
    client: &PriorAuthorizationContractClient,
    provider: &Address,
    patient: &Address,
    insurer: &Address,
) -> u64 {
    let mut service_codes = Vec::new(env);
    service_codes.push_back(String::from_str(env, "CPT99213"));

    let mut diagnosis_codes = Vec::new(env);
    diagnosis_codes.push_back(String::from_str(env, "E11.9"));

    let hash = BytesN::from_array(env, &[1u8; 32]);

    client.submit_prior_authorization(
        provider,
        patient,
        insurer,
        &1001u64,
        &Symbol::new(env, "medication"),
        &String::from_str(env, "Insulin Glargine"),
        &service_codes,
        &diagnosis_codes,
        &hash,
        &Symbol::new(env, "routine"),
    )
}

fn register_test_reviewer(
    env: &Env,
    client: &PriorAuthorizationContractClient,
    insurer: &Address,
    reviewer: &Address,
) {
    let mut specialties = Vec::new(env);
    specialties.push_back(Symbol::new(env, "general"));
    client.register_reviewer(
        insurer,
        reviewer,
        &Symbol::new(env, "reviewer"),
        &specialties,
        &50u32,
        &None,
    );
}

fn approve(
    env: &Env,
    client: &PriorAuthorizationContractClient,
    auth_id: u64,
    reviewer: &Address,
) {
    let insurer = Address::generate(env);
    register_test_reviewer(env, client, &insurer, reviewer);
    client.review_authorization(
        &auth_id,
        reviewer,
        &Symbol::new(env, "approved"),
        &Some(10u32),
        &Some(1_000_000u64),
        &Some(9_000_000u64),
        &String::from_str(env, "Approved for chronic condition"),
    );
}

fn deny(
    env: &Env,
    client: &PriorAuthorizationContractClient,
    auth_id: u64,
    reviewer: &Address,
) {
    let insurer = Address::generate(env);
    register_test_reviewer(env, client, &insurer, reviewer);
    client.review_authorization(
        &auth_id,
        reviewer,
        &Symbol::new(env, "denied"),
        &None,
        &None,
        &None,
        &String::from_str(env, "Not medically necessary"),
    );
}

// -----------------------------------------------------------------------
// submit_prior_authorization
// -----------------------------------------------------------------------

#[test]
fn test_submit_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    assert_eq!(id, 1);
}

#[test]
fn test_submit_increments_ids() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id1 = submit(&env, &client, &provider, &patient, &insurer);
    let id2 = submit(&env, &client, &provider, &patient, &insurer);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_submit_initial_status_is_submitted() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::Submitted));
    assert_eq!(info.units_used, 0);
    assert!(info.decision.is_none());
}

// -----------------------------------------------------------------------
// attach_supporting_documentation
// -----------------------------------------------------------------------

#[test]
fn test_attach_document_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let hash = BytesN::from_array(&env, &[2u8; 32]);
    client
        .attach_supporting_documentation(
            &id,
            &provider,
            &hash,
            &Symbol::new(&env, "clinical_notes"),
        )
;
}

#[test]
fn test_attach_document_wrong_provider_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let other = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[3u8; 32]);

    let result = client.try_attach_supporting_documentation(
        &id,
        &other,
        &hash,
        &Symbol::new(&env, "lab_results"),
    );
    assert!(result.is_err());
}

#[test]
fn test_attach_document_not_found_fails() {
    let (env, provider, _, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let hash = BytesN::from_array(&env, &[4u8; 32]);

    let result = client.try_attach_supporting_documentation(
        &999,
        &provider,
        &hash,
        &Symbol::new(&env, "lab_results"),
    );
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// review_authorization
// -----------------------------------------------------------------------

#[test]
fn test_review_approve_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer);

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::Approved));
    assert_eq!(info.approved_units, Some(10));
    assert!(info.valid_from.is_some());
    assert!(info.valid_until.is_some());
    assert!(info.decision_date.is_some());
}

#[test]
fn test_review_deny_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    deny(&env, &client, id, &reviewer);

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::Denied));
}

#[test]
fn test_review_more_info_needed() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    let insurer = Address::generate(&env);
    register_test_reviewer(&env, &client, &insurer, &reviewer);

    client.review_authorization(
        &id,
        &reviewer,
        &Symbol::new(&env, "more_info_needed"),
        &None,
        &None,
        &None,
        &String::from_str(&env, "Need additional clinical notes"),
    );

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::MoreInfoNeeded));
}

#[test]
fn test_review_invalid_decision_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);

    let result = client.try_review_authorization(
        &id,
        &reviewer,
        &Symbol::new(&env, "maybe"),
        &None,
        &None,
        &None,
        &String::from_str(&env, "notes"),
    );
    assert!(result.is_err());
}

#[test]
fn test_review_already_approved_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer);

    let result = client.try_review_authorization(
        &id,
        &reviewer,
        &Symbol::new(&env, "approved"),
        &Some(5u32),
        &None,
        &None,
        &String::from_str(&env, "Again"),
    );
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// request_peer_to_peer / schedule_peer_to_peer
// -----------------------------------------------------------------------

#[test]
fn test_request_p2p_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let mut times = Vec::new(&env);
    times.push_back(String::from_str(&env, "Mon 9am"));

    client
        .request_peer_to_peer(&id, &provider, &2_000_000u64, &times)
;

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::UnderReview));
}

#[test]
fn test_request_p2p_twice_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let mut times = Vec::new(&env);
    times.push_back(String::from_str(&env, "Mon 9am"));

    client
        .request_peer_to_peer(&id, &provider, &2_000_000u64, &times)
;

    let result = client.try_request_peer_to_peer(&id, &provider, &2_000_000u64, &times);
    assert!(result.is_err());
}

#[test]
fn test_schedule_p2p_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let mut times = Vec::new(&env);
    times.push_back(String::from_str(&env, "Tue 2pm"));

    client
        .request_peer_to_peer(&id, &provider, &2_000_000u64, &times)
;

    let insurance_admin = Address::generate(&env);
    let medical_director = Address::generate(&env);

    client
        .schedule_peer_to_peer(&id, &insurance_admin, &3_000_000u64, &medical_director)
;

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::PeerToPeerScheduled));
}

#[test]
fn test_p2p_wrong_provider_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let other = Address::generate(&env);
    let mut times = Vec::new(&env);
    times.push_back(String::from_str(&env, "Wed 10am"));

    let result = client.try_request_peer_to_peer(&id, &other, &2_000_000u64, &times);
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// appeal_denial
// -----------------------------------------------------------------------

#[test]
fn test_appeal_level_1_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    deny(&env, &client, id, &reviewer);

    let hash = BytesN::from_array(&env, &[5u8; 32]);
    let appeal_id = client
        .appeal_denial(&id, &provider, &1u32, &hash, &None)
;

    assert_eq!(appeal_id, 1);

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::Appealed));
}

// -----------------------------------------------------------------------
// insurer-registry integration (#526)
// -----------------------------------------------------------------------

#[test]
fn test_integration_covered_service_proceeds() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    assert_eq!(id, 1);
}

#[test]
fn test_integration_uncovered_service_returns_service_not_covered() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);

    let mut service_codes = Vec::new(&env);
    service_codes.push_back(String::from_str(&env, "CPT99999"));
    let mut diagnosis_codes = Vec::new(&env);
    diagnosis_codes.push_back(String::from_str(&env, "E11.9"));
    let hash = BytesN::from_array(&env, &[1u8; 32]);

    let result = client.try_submit_prior_authorization(
        &provider,
        &patient,
        &insurer,
        &1001u64,
        &Symbol::new(&env, "medication"),
        &String::from_str(&env, "Experimental Therapy"),
        &service_codes,
        &diagnosis_codes,
        &hash,
        &Symbol::new(&env, "routine"),
    );
    assert_eq!(result, Err(Ok(Error::ServiceNotCovered)));
}

#[test]
fn test_appeal_level_2_and_3() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    deny(&env, &client, id, &reviewer);

    let h1 = BytesN::from_array(&env, &[5u8; 32]);
    client.appeal_denial(&id, &provider, &1u32, &h1, &None);

    let h2 = BytesN::from_array(&env, &[6u8; 32]);
    client.appeal_denial(&id, &provider, &2u32, &h2, &None);

    let h3 = BytesN::from_array(&env, &[7u8; 32]);
    let appeal_id = client.appeal_denial(&id, &provider, &3u32, &h3, &None);

    assert_eq!(appeal_id, 3);
}

#[test]
fn test_appeal_exceeds_max_level_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    deny(&env, &client, id, &reviewer);

    let hash = BytesN::from_array(&env, &[8u8; 32]);
    let result = client.try_appeal_denial(&id, &provider, &4u32, &hash, &None);
    assert!(result.is_err());
}

#[test]
fn test_appeal_not_denied_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let hash = BytesN::from_array(&env, &[9u8; 32]);
    let result = client.try_appeal_denial(&id, &provider, &1u32, &hash, &None);
    assert!(result.is_err());
}

#[test]
fn test_appeal_wrong_provider_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    deny(&env, &client, id, &reviewer);

    let other = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[10u8; 32]);
    let result = client.try_appeal_denial(&id, &other, &1u32, &hash, &None);
    assert!(result.is_err());
}

#[test]
fn test_appeal_with_additional_evidence() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    deny(&env, &client, id, &reviewer);

    let reason_hash = BytesN::from_array(&env, &[11u8; 32]);
    let evidence_hash = BytesN::from_array(&env, &[12u8; 32]);

    client
        .appeal_denial(&id, &provider, &1u32, &reason_hash, &Some(evidence_hash))
;
}

#[test]
fn test_review_history_and_appeal_chain_is_tamper_evident() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    let insurer = Address::generate(&env);
    register_test_reviewer(&env, &client, &insurer, &reviewer);

    client.review_authorization(
        &id,
        &reviewer,
        &Symbol::new(&env, "denied"),
        &None,
        &None,
        &None,
        &String::from_str(&env, "Initial denial based on policy criteria"),
    );

    let review_history = client.get_review_history(&id, &provider);
    assert_eq!(review_history.len(), 1);
    let first_review = review_history.get(0).unwrap();
    assert_eq!(first_review.reviewer_id, reviewer);
    assert!(first_review.prior_review_hash.is_none());
    assert_ne!(first_review.review_entry_hash, BytesN::from_array(&env, &[0u8; 32]));

    let reason_hash = BytesN::from_array(&env, &[13u8; 32]);
    client
        .appeal_denial(&id, &provider, &1u32, &reason_hash, &None)
;

    let appeals = client.get_appeal_history(&id, &provider);
    assert_eq!(appeals.len(), 1);
    let first_appeal = appeals.get(0).unwrap();
    assert!(first_appeal.previous_appeal_id.is_none());
    assert!(first_appeal.previous_appeal_hash.is_none());
    assert_eq!(first_appeal.ruling_dependency_hash, first_review.review_entry_hash);
    assert_ne!(first_appeal.appeal_chain_hash, BytesN::from_array(&env, &[0u8; 32]));
}

// -----------------------------------------------------------------------
// expedite_authorization
// -----------------------------------------------------------------------

#[test]
fn test_expedite_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    client
        .expedite_authorization(
            &id,
            &provider,
            &String::from_str(&env, "Patient surgery in 48 hours"),
            &1_100_000u64,
        )
;
}

#[test]
fn test_expedite_approved_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer);

    let result = client.try_expedite_authorization(
        &id,
        &provider,
        &String::from_str(&env, "Too late"),
        &1_100_000u64,
    );
    assert!(result.is_err());
}

#[test]
fn test_expedite_wrong_provider_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let other = Address::generate(&env);
    let result = client.try_expedite_authorization(
        &id,
        &other,
        &String::from_str(&env, "Urgent"),
        &1_100_000u64,
    );
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// extend_authorization
// -----------------------------------------------------------------------

#[test]
fn test_extend_approved_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer);

    client
        .extend_authorization(
            &id,
            &provider,
            &String::from_str(&env, "Ongoing chronic condition"),
            &5u32,
        )
;
}

#[test]
fn test_extend_not_approved_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let result = client.try_extend_authorization(
        &id,
        &provider,
        &String::from_str(&env, "Reason"),
        &5u32,
    );
    assert!(result.is_err());
}

#[test]
fn test_extend_wrong_provider_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer);

    let other = Address::generate(&env);
    let result = client.try_extend_authorization(
        &id,
        &other,
        &String::from_str(&env, "Reason"),
        &5u32,
    );
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// track_authorization_usage
// -----------------------------------------------------------------------

#[test]
fn test_track_usage_success() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer);

    client
        .track_authorization_usage(&id, &provider, &3u32, &1_500_000u64)
;

    let info = client.get_authorization_status(&id, &provider);
    assert_eq!(info.units_used, 3);
}

#[test]
fn test_track_usage_accumulates() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer);

    client
        .track_authorization_usage(&id, &provider, &3u32, &1_500_000u64)
;
    client
        .track_authorization_usage(&id, &provider, &4u32, &1_600_000u64)
;

    let info = client.get_authorization_status(&id, &provider);
    assert_eq!(info.units_used, 7);
}

#[test]
fn test_track_usage_exceeds_approved_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer); // approved_units = 10

    let result = client.try_track_authorization_usage(&id, &provider, &11u32, &1_500_000u64);
    assert!(result.is_err());
}

#[test]
fn test_track_usage_not_approved_fails() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);

    let result = client.try_track_authorization_usage(&id, &provider, &1u32, &1_500_000u64);
    assert!(result.is_err());
}

#[test]
fn test_track_usage_expired_fails() {
    let (env, provider, patient, insurer) = setup();
    env.ledger().with_mut(|li| li.timestamp = 1_000_000);

    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    let reviewer = Address::generate(&env);
    let insurer = Address::generate(&env);
    register_test_reviewer(&env, &client, &insurer, &reviewer);

    // Approve with valid_until in the past relative to usage tracking time
    client.review_authorization(
        &id,
        &reviewer,
        &Symbol::new(&env, "approved"),
        &Some(10u32),
        &Some(1_000_000u64),
        &Some(1_500_000u64), // expires at 1.5M
        &String::from_str(&env, "Approved"),
    );

    // Advance time past expiry
    env.ledger().with_mut(|li| li.timestamp = 2_000_000);

    let result = client.try_track_authorization_usage(&id, &provider, &1u32, &2_000_000u64);
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// get_authorization_status
// -----------------------------------------------------------------------

#[test]
fn test_get_status_not_found_fails() {
    let (env, provider, _, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let result = client.try_get_authorization_status(&999, &provider);
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// Full multi-step workflow
// -----------------------------------------------------------------------

#[test]
fn test_full_workflow_approve_and_use() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);

    // 1. Submit
    let id = submit(&env, &client, &provider, &patient, &insurer);

    // 2. Attach document
    let doc_hash = BytesN::from_array(&env, &[20u8; 32]);
    client
        .attach_supporting_documentation(
            &id,
            &provider,
            &doc_hash,
            &Symbol::new(&env, "lab_results"),
        )
;

    // 3. Expedite
    client
        .expedite_authorization(
            &id,
            &provider,
            &String::from_str(&env, "Urgent surgery"),
            &1_100_000u64,
        )
;

    // 4. Review -> approve
    let reviewer = Address::generate(&env);
    approve(&env, &client, id, &reviewer);

    // 5. Track usage (3 of 10)
    client
        .track_authorization_usage(&id, &provider, &3u32, &1_500_000u64)
;

    // 6. Extend
    client
        .extend_authorization(
            &id,
            &provider,
            &String::from_str(&env, "Continued treatment needed"),
            &5u32,
        )
;

    // 7. Track more usage (5 of 10)
    client
        .track_authorization_usage(&id, &provider, &5u32, &1_600_000u64)
;

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::Approved));
    assert_eq!(info.units_used, 8);
}

#[test]
fn test_full_workflow_deny_appeal_three_levels() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);

    let id = submit(&env, &client, &provider, &patient, &insurer);

    // Request peer-to-peer
    let mut times = Vec::new(&env);
    times.push_back(String::from_str(&env, "Thu 11am"));
    client
        .request_peer_to_peer(&id, &provider, &2_000_000u64, &times)
;

    // Schedule peer-to-peer
    let insurance_admin = Address::generate(&env);
    let medical_director = Address::generate(&env);
    client
        .schedule_peer_to_peer(&id, &insurance_admin, &3_000_000u64, &medical_director)
;

    // Deny after P2P
    let reviewer = Address::generate(&env);
    deny(&env, &client, id, &reviewer);

    // Level 1 appeal
    let h1 = BytesN::from_array(&env, &[30u8; 32]);
    client.appeal_denial(&id, &provider, &1u32, &h1, &None);

    // Level 2 appeal
    let h2 = BytesN::from_array(&env, &[31u8; 32]);
    let ev2 = BytesN::from_array(&env, &[32u8; 32]);
    client
        .appeal_denial(&id, &provider, &2u32, &h2, &Some(ev2))
;

    // Level 3 appeal (final)
    let h3 = BytesN::from_array(&env, &[33u8; 32]);
    let appeal_id = client.appeal_denial(&id, &provider, &3u32, &h3, &None);
    assert_eq!(appeal_id, 3);

    // 4th level should fail
    let h4 = BytesN::from_array(&env, &[34u8; 32]);
    let result = client.try_appeal_denial(&id, &provider, &4u32, &h4, &None);
    assert!(result.is_err());

    let info = client.get_authorization_status(&id, &provider);
    assert!(matches!(info.status, AuthStatus::Appealed));
}

// -----------------------------------------------------------------------
// insurer-registry integration (#526)
// -----------------------------------------------------------------------

#[test]
fn test_integration_covered_service_proceeds() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);
    let id = submit(&env, &client, &provider, &patient, &insurer);
    assert_eq!(id, 1);
}

#[test]
fn test_integration_uncovered_service_returns_service_not_covered() {
    let (env, provider, patient, insurer) = setup();
    let client = setup_client(&env, &insurer);

    let mut service_codes = Vec::new(&env);
    service_codes.push_back(String::from_str(&env, "CPT99999"));
    let mut diagnosis_codes = Vec::new(&env);
    diagnosis_codes.push_back(String::from_str(&env, "E11.9"));
    let hash = BytesN::from_array(&env, &[1u8; 32]);

    let result = client.try_submit_prior_authorization(
        &provider,
        &patient,
        &insurer,
        &1001u64,
        &Symbol::new(&env, "medication"),
        &String::from_str(&env, "Experimental Therapy"),
        &service_codes,
        &diagnosis_codes,
        &hash,
        &Symbol::new(&env, "routine"),
    );
    assert_eq!(result, Err(Ok(Error::ServiceNotCovered)));
}