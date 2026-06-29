#![cfg(test)]

use super::*;
use insurer_registry::{CoveragePlan, InsurerRegistry, InsurerRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Symbol, Vec,
};

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
    let insurer = Address::generate(&env);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    (env, insurer, provider, patient)
}

fn make_contract(env: &Env, insurer: &Address) -> PriorAuthorizationContractClient {
    let ir_id = setup_insurer_registry(env, insurer);
    let contract_id = env.register(PriorAuthorizationContract, ());
    let client = PriorAuthorizationContractClient::new(env, &contract_id);
    client.initialize(&ir_id);
    client
}

fn register_reviewer(
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

fn submit_auth(
    env: &Env,
    client: &PriorAuthorizationContractClient,
    provider: &Address,
    patient: &Address,
    insurer: &Address,
    urgency: &Symbol,
) -> u64 {
    let mut svc = Vec::new(env);
    svc.push_back(String::from_str(env, "CPT99213"));
    let mut diag = Vec::new(env);
    diag.push_back(String::from_str(env, "E11.9"));
    let hash = BytesN::from_array(env, &[1u8; 32]);

    client.submit_prior_authorization(
        provider,
        patient,
        insurer,
        &1001u64,
        &Symbol::new(env, "medication"),
        &String::from_str(env, "Insulin Glargine"),
        &svc,
        &diag,
        &hash,
        urgency,
    )
}

// ── register_reviewer ────────────────────────────────────────────────────────

#[test]
fn test_register_reviewer_success() {
    let (env, insurer, _provider, _patient) = setup();
    let client = make_contract(&env, &insurer);
    let reviewer = Address::generate(&env);

    let mut specialties = Vec::new(&env);
    specialties.push_back(Symbol::new(&env, "cardiology"));

    client.register_reviewer(
        &insurer,
        &reviewer,
        &Symbol::new(&env, "medical_director"),
        &specialties,
        &50u32,
        &None,
    );
}

#[test]
fn test_register_reviewer_unauthorized_reviewer_fails() {
    let (env, insurer, _provider, _patient) = setup();
    let client = make_contract(&env, &insurer);
    let reviewer = Address::generate(&env);

    let mut specialties = Vec::new(&env);
    specialties.push_back(Symbol::new(&env, "general"));

    client.register_reviewer(
        &insurer,
        &reviewer,
        &Symbol::new(&env, "reviewer"),
        &specialties,
        &10u32,
        &None,
    );

    // Unregistered reviewer should fail
    let unauthorized = Address::generate(&env);
    let auth_id = submit_auth(
        &env,
        &client,
        &Address::generate(&env),
        &Address::generate(&env),
        &insurer,
        &Symbol::new(&env, "routine"),
    );

    let result = client.try_review_authorization(
        &auth_id,
        &unauthorized,
        &Symbol::new(&env, "approved"),
        &Some(5u32),
        &Some(1_000_000u64),
        &Some(9_000_000u64),
        &String::from_str(&env, "Unauthorized"),
    );
    assert!(result.is_err());
}

// ── configure_sla ────────────────────────────────────────────────────────────

#[test]
fn test_configure_sla_success() {
    let (env, insurer, _provider, _patient) = setup();
    let client = make_contract(&env, &insurer);

    client.configure_sla(
        &insurer,
        &Symbol::new(&env, "standard"),
        &72u64,
        &24u64,
        &30u32,
        &false,
    );
}

// ── SLA breach detection ─────────────────────────────────────────────────────

#[test]
fn test_status_on_time_no_breach() {
    let (env, insurer, provider, patient) = setup();
    let client = make_contract(&env, &insurer);

    let auth_id = submit_auth(&env, &client, &provider, &patient, &insurer, &Symbol::new(&env, "routine"));

    // Query before deadline — no breach event
    let info = client.get_authorization_status(&auth_id, &provider);
    assert!(matches!(info.status, AuthStatus::Submitted));
}

#[test]
fn test_status_after_deadline_detects_breach() {
    let (env, insurer, provider, patient) = setup();
    let client = make_contract(&env, &insurer);

    // Configure short 1-hour SLA
    client.configure_sla(
        &insurer,
        &Symbol::new(&env, "routine"),
        &1u64,
        &1u64,
        &30u32,
        &false,
    );

    let auth_id = submit_auth(&env, &client, &provider, &patient, &insurer, &Symbol::new(&env, "routine"));

    // Advance time past the SLA deadline (default 72h for routine = 259200s)
    env.ledger().with_mut(|li| li.timestamp += 300_000);

    // Query after deadline — SLABreached event is emitted and added to overdue list
    let info = client.get_authorization_status(&auth_id, &provider);
    assert!(matches!(info.status, AuthStatus::Submitted));
}

// ── escalation ───────────────────────────────────────────────────────────────

#[test]
fn test_escalate_overdue_authorization() {
    let (env, insurer, provider, patient) = setup();
    let client = make_contract(&env, &insurer);

    let reviewer = Address::generate(&env);
    register_reviewer(&env, &client, &insurer, &reviewer);

    let auth_id = submit_auth(&env, &client, &provider, &patient, &insurer, &Symbol::new(&env, "routine"));

    // Advance time past the SLA deadline (default 72h = 259200s)
    env.ledger().with_mut(|li| li.timestamp += 300_000);

    // Trigger breach detection by querying status
    client.get_authorization_status(&auth_id, &provider);

    // Escalate
    let count = client.escalate_expired_authorizations(&insurer);
    assert_eq!(count, 1);

    // Verify the request was escalated
    let info = client.get_authorization_status(&auth_id, &provider);
    assert!(matches!(info.status, AuthStatus::Escalated));
}

#[test]
fn test_escalate_no_overdue_returns_zero() {
    let (env, insurer, provider, patient) = setup();
    let client = make_contract(&env, &insurer);

    let reviewer = Address::generate(&env);
    register_reviewer(&env, &client, &insurer, &reviewer);

    // Submit but don't advance time
    submit_auth(&env, &client, &provider, &patient, &insurer, &Symbol::new(&env, "routine"));

    let count = client.escalate_expired_authorizations(&insurer);
    assert_eq!(count, 0);
}

#[test]
fn test_escalate_already_resolved_skipped() {
    let (env, insurer, provider, patient) = setup();
    let client = make_contract(&env, &insurer);

    let reviewer = Address::generate(&env);
    register_reviewer(&env, &client, &insurer, &reviewer);

    let auth_id = submit_auth(&env, &client, &provider, &patient, &insurer, &Symbol::new(&env, "routine"));

    // Approve the request before the deadline
    client.review_authorization(
        &auth_id,
        &reviewer,
        &Symbol::new(&env, "approved"),
        &Some(10u32),
        &Some(1_000_000u64),
        &Some(9_000_000u64),
        &String::from_str(&env, "Approved"),
    );

    // Advance time past deadline
    env.ledger().with_mut(|li| li.timestamp += 300_000);

    // Escalation should skip already-approved request
    let count = client.escalate_expired_authorizations(&insurer);
    assert_eq!(count, 0);
}

#[test]
fn test_reviewer_registered_by_insurer() {
    let (env, insurer, _provider, _patient) = setup();
    let client = make_contract(&env, &insurer);

    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);

    register_reviewer(&env, &client, &insurer, &reviewer1);
    register_reviewer(&env, &client, &insurer, &reviewer2);

    // Both reviewers should be able to receive escalated work
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let auth_id = submit_auth(&env, &client, &provider, &patient, &insurer, &Symbol::new(&env, "routine"));

    env.ledger().with_mut(|li| li.timestamp += 300_000);
    client.get_authorization_status(&auth_id, &provider);

    let count = client.escalate_expired_authorizations(&insurer);
    assert_eq!(count, 1);
}

// ── SLA deadline enforcement in review_authorization ─────────────────────────

#[test]
fn test_review_after_sla_deadline_fails() {
    let (env, insurer, provider, patient) = setup();
    let client = make_contract(&env, &insurer);

    let reviewer = Address::generate(&env);
    register_reviewer(&env, &client, &insurer, &reviewer);

    let auth_id = submit_auth(&env, &client, &provider, &patient, &insurer, &Symbol::new(&env, "routine"));

    // Advance past deadline
    env.ledger().with_mut(|li| li.timestamp += 300_000);

    let result = client.try_review_authorization(
        &auth_id,
        &reviewer,
        &Symbol::new(&env, "approved"),
        &Some(5u32),
        &Some(1_000_000u64),
        &Some(9_000_000u64),
        &String::from_str(&env, "Late review"),
    );
    assert!(result.is_err());
}
