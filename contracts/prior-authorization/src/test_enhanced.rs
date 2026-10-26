#![cfg(test)]

use super::*;
use insurer_registry::{
    CoveragePlan, InsurerRegistry, InsurerRegistryClient, MAX_CREDENTIAL_LIFETIME,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Symbol, Vec,
};

fn dummy_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn setup_insurer_registry(env: &Env, insurer: &Address) -> (Address, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let ir_id = env.register(InsurerRegistry, (admin.clone(),));
    let ir_client = InsurerRegistryClient::new(env, &ir_id);
    let issuer = Address::generate(env);
    ir_client.set_trusted_issuer(&admin, &issuer, &true);
    ir_client.register_insurer(
        insurer,
        &String::from_str(env, "Test Insurer"),
        &String::from_str(env, "LIC-001"),
        &String::from_str(env, "metadata"),
        &dummy_hash(env, 1),
        &issuer,
        &dummy_hash(env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
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
    (ir_id, admin)
}

fn setup() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let insurer = Address::generate(&env);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    (env, insurer, provider, patient)
}

fn make_contract<'a>(env: &'a Env, insurer: &Address) -> PriorAuthorizationContractClient<'a> {
    let (ir_id, _) = setup_insurer_registry(env, insurer);
    let contract_id = env.register(PriorAuthorizationContract, ());
    let client = PriorAuthorizationContractClient::new(env, &contract_id);
    client.initialize(&ir_id);
    client
}

fn make_contract_with_registry<'a>(
    env: &'a Env,
    insurer: &Address,
) -> (PriorAuthorizationContractClient<'a>, Address, Address) {
    let (ir_id, admin) = setup_insurer_registry(env, insurer);
    let contract_id = env.register(PriorAuthorizationContract, ());
    let client = PriorAuthorizationContractClient::new(env, &contract_id);
    client.initialize(&ir_id);
    (client, ir_id, admin)
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

#[test]
fn test_configure_sla_rejects_non_insurer() {
    let (env, insurer, _provider, _patient) = setup();
    let client = make_contract(&env, &insurer);

    // An address that was never registered in the insurer-registry must be
    // rejected even though `mock_all_auths` makes the `require_auth()` check
    // pass on its own (#723).
    let non_insurer = Address::generate(&env);

    let result = client.try_configure_sla(
        &non_insurer,
        &Symbol::new(&env, "standard"),
        &72u64,
        &24u64,
        &30u32,
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn test_configure_sla_is_scoped_per_insurer() {
    let (env, insurer_a, provider, patient) = setup();
    let (ir_id, registry_admin) = setup_insurer_registry(&env, &insurer_a);

    // Register a second, independent insurer on the same insurer-registry
    // instance so it is also active.
    let insurer_b = Address::generate(&env);
    let ir_client = InsurerRegistryClient::new(&env, &ir_id);
    let issuer = Address::generate(&env);
    ir_client.set_trusted_issuer(&registry_admin, &issuer, &true);
    ir_client.register_insurer(
        &insurer_b,
        &String::from_str(&env, "Second Insurer"),
        &String::from_str(&env, "LIC-002"),
        &String::from_str(&env, "metadata"),
        &dummy_hash(&env, 4),
        &issuer,
        &dummy_hash(&env, 5),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 6),
    );

    let contract_id = env.register(PriorAuthorizationContract, ());
    let client = PriorAuthorizationContractClient::new(&env, &contract_id);
    client.initialize(&ir_id);

    let urgency = Symbol::new(&env, "routine");

    // Insurer A requires medical-director sign-off for "routine" requests.
    client.configure_sla(&insurer_a, &urgency, &72u64, &24u64, &30u32, &true);

    // Insurer B configures the SAME urgency for its own requests with a
    // laxer policy. Before the fix this overwrote the single shared
    // SLAConfig(urgency) entry and silently disabled insurer A's
    // medical-director requirement.
    client.configure_sla(&insurer_b, &urgency, &72u64, &24u64, &30u32, &false);

    let auth_id = submit_auth(&env, &client, &provider, &patient, &insurer_a, &urgency);

    // A non-medical-director reviewer on insurer A's own roster must still
    // be rejected, proving insurer A's stricter policy was not clobbered by
    // insurer B's later call at the same urgency (#723).
    let reviewer = Address::generate(&env);
    register_reviewer(&env, &client, &insurer_a, &reviewer);

    let result = client.try_review_authorization(
        &auth_id,
        &reviewer,
        &Symbol::new(&env, "approved"),
        &Some(5u32),
        &Some(1_000_000u64),
        &Some(9_000_000u64),
        &String::from_str(&env, "Attempting approval without medical director"),
    );
    assert!(result.is_err());
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

// ── Cross-insurer escalation isolation (#734) ─────────────────────────────────

#[test]
fn test_escalate_does_not_hijack_other_insurers_requests() {
    let env = Env::default();
    env.mock_all_auths();

    let insurer_a = Address::generate(&env);
    let insurer_b = Address::generate(&env);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // One shared insurer registry with BOTH insurers registered as active.
    let registry_admin = Address::generate(&env);
    let ir_id = env.register(InsurerRegistry, (registry_admin.clone(),));
    let ir_client = InsurerRegistryClient::new(&env, &ir_id);
    let issuer = Address::generate(&env);
    ir_client.set_trusted_issuer(&registry_admin, &issuer, &true);

    ir_client.register_insurer(
        &insurer_a,
        &String::from_str(&env, "Insurer A"),
        &String::from_str(&env, "LIC-A"),
        &String::from_str(&env, "metadata"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 3),
    );
    let mut svc_a = Vec::new(&env);
    svc_a.push_back(String::from_str(&env, "CPT99213"));
    let mut plans_a = Vec::new(&env);
    plans_a.push_back(CoveragePlan {
        plan_id: 1,
        plan_name: String::from_str(&env, "Plan A"),
        service_codes: svc_a,
        is_active: true,
        effective_from: 0,
        effective_until: None,
    });
    ir_client.set_coverage_plans(&insurer_a, &plans_a);

    ir_client.register_insurer(
        &insurer_b,
        &String::from_str(&env, "Insurer B"),
        &String::from_str(&env, "LIC-B"),
        &String::from_str(&env, "metadata"),
        &dummy_hash(&env, 4),
        &issuer,
        &dummy_hash(&env, 5),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 6),
    );
    let mut svc_b = Vec::new(&env);
    svc_b.push_back(String::from_str(&env, "CPT99213"));
    let mut plans_b = Vec::new(&env);
    plans_b.push_back(CoveragePlan {
        plan_id: 2,
        plan_name: String::from_str(&env, "Plan B"),
        service_codes: svc_b,
        is_active: true,
        effective_from: 0,
        effective_until: None,
    });
    ir_client.set_coverage_plans(&insurer_b, &plans_b);

    // One shared PriorAuthorizationContract instance backed by that registry.
    let contract_id = env.register(PriorAuthorizationContract, ());
    let client = PriorAuthorizationContractClient::new(&env, &contract_id);
    client.initialize(&ir_id);

    // Each insurer has its own reviewer pool so insurer_b's call doesn't
    // short-circuit on an empty reviewer pool.
    let reviewer_a = Address::generate(&env);
    let reviewer_b = Address::generate(&env);
    register_reviewer(&env, &client, &insurer_a, &reviewer_a);
    register_reviewer(&env, &client, &insurer_b, &reviewer_b);

    // Submit a request under insurer_a and let it breach its SLA.
    let auth_id = submit_auth(
        &env,
        &client,
        &provider,
        &patient,
        &insurer_a,
        &Symbol::new(&env, "routine"),
    );
    env.ledger().with_mut(|li| li.timestamp += 300_000);

    // Trigger breach detection, which pushes it onto the shared overdue list.
    let info = client.get_authorization_status(&auth_id, &provider);
    assert!(matches!(info.status, AuthStatus::Submitted));

    // insurer_b calls escalate_expired_authorizations — it must NOT touch
    // insurer_a's overdue request.
    let count_b = client.escalate_expired_authorizations(&insurer_b);
    assert_eq!(count_b, 0);

    // insurer_a's request is completely unaffected: still Submitted, not
    // reassigned to a reviewer and not removed from the overdue list.
    let info_after_b = client.get_authorization_status(&auth_id, &provider);
    assert!(matches!(info_after_b.status, AuthStatus::Submitted));

    // insurer_a's own escalation run still successfully escalates it,
    // proving insurer_b's call did not silently remove it from the overdue
    // list.
    let count_a = client.escalate_expired_authorizations(&insurer_a);
    assert_eq!(count_a, 1);

    let info_after_a = client.get_authorization_status(&auth_id, &provider);
    assert!(matches!(info_after_a.status, AuthStatus::Escalated));
}

// ── Insurer binding (#684) ────────────────────────────────────────────────────

#[test]
fn test_reviewer_from_wrong_insurer_rejected() {
    let (env, insurer, provider, patient) = setup();
    let (client, ir_id, registry_admin) = make_contract_with_registry(&env, &insurer);

    // Register a reviewer under the correct insurer
    let reviewer_a = Address::generate(&env);
    register_reviewer(&env, &client, &insurer, &reviewer_a);

    // Create a second insurer that is also registered in the registry
    let insurer_b = Address::generate(&env);
    let ir_client = InsurerRegistryClient::new(&env, &ir_id);
    let issuer_b = Address::generate(&env);
    ir_client.set_trusted_issuer(&registry_admin, &issuer_b, &true);
    ir_client.register_insurer(
        &insurer_b,
        &String::from_str(&env, "Insurer B"),
        &String::from_str(&env, "LIC-002"),
        &String::from_str(&env, "metadata"),
        &dummy_hash(&env, 10),
        &issuer_b,
        &dummy_hash(&env, 11),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 12),
    );
    let mut svc_b = Vec::new(&env);
    svc_b.push_back(String::from_str(&env, "CPT99213"));
    let mut plans_b = Vec::new(&env);
    plans_b.push_back(CoveragePlan {
        plan_id: 10,
        plan_name: String::from_str(&env, "Plan B"),
        service_codes: svc_b,
        is_active: true,
        effective_from: 0,
        effective_until: None,
    });
    ir_client.set_coverage_plans(&insurer_b, &plans_b);

    // Register reviewer_b under insurer_b
    let reviewer_b = Address::generate(&env);
    let mut specialties = Vec::new(&env);
    specialties.push_back(Symbol::new(&env, "general"));
    client.register_reviewer(
        &insurer_b,
        &reviewer_b,
        &Symbol::new(&env, "reviewer"),
        &specialties,
        &50u32,
        &None,
    );

    // Submit a request against insurer
    let auth_id = submit_auth(&env, &client, &provider, &patient, &insurer, &Symbol::new(&env, "routine"));

    // Reviewer_b tries to review a request submitted against insurer — should be rejected
    let result = client.try_review_authorization(
        &auth_id,
        &reviewer_b,
        &Symbol::new(&env, "approved"),
        &Some(10u32),
        &Some(1_000_000u64),
        &Some(9_000_000u64),
        &String::from_str(&env, "Cross-insurer review attempt"),
    );
    assert!(result.is_err());

    // Reviewer_a (correct insurer) should succeed
    client.review_authorization(
        &auth_id,
        &reviewer_a,
        &Symbol::new(&env, "approved"),
        &Some(10u32),
        &Some(1_000_000u64),
        &Some(9_000_000u64),
        &String::from_str(&env, "Legitimate review"),
    );
    let info = client.get_authorization_status(&auth_id, &provider);
    assert!(matches!(info.status, AuthStatus::Approved));
}
