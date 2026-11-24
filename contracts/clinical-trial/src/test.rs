use crate::{ClinicalTrialContractClient, CriteriaRule, DataFilters, Error};
use soroban_sdk::{symbol_short, testutils::Address as _, testutils::Events, Address, Bytes, BytesN, Env, String, Vec};
use soroban_sdk::xdr::ToXdr;

fn create_test_env() -> (Env, Address, Address, Address, ClinicalTrialContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let pi = Address::generate(&env);
    let patient = Address::generate(&env);

    let contract_id = env.register(crate::ClinicalTrialContract, (&admin, &pi));
    let client = ClinicalTrialContractClient::new(&env, &contract_id);

    (env, admin, pi, patient, client)
}

fn create_protocol_hash(env: &Env) -> BytesN<32> {
    let data = String::from_str(env, "protocol_v1");
    env.crypto().sha256(&data.into()).into()
}

fn make_rule(env: &Env, parameter: &str, value: &str) -> CriteriaRule {
    CriteriaRule {
        criteria_type: symbol_short!("demo"),
        parameter: String::from_str(env, parameter),
        operator: symbol_short!("eq"),
        value: String::from_str(env, value),
        mandatory: true,
    }
}

fn expected_claim_hash(
    env: &Env,
    trial_record_id: u64,
    patient_data_hash: &BytesN<32>,
    rule: &CriteriaRule,
) -> BytesN<32> {
    let mut payload = Bytes::new(env);
    payload.append(&Bytes::from_slice(env, b"trial-eligibility-v1"));
    payload.append(&Bytes::from_slice(env, &trial_record_id.to_be_bytes()));
    payload.append(&patient_data_hash.clone().into());
    payload.append(&rule.criteria_type.clone().to_xdr(env));
    payload.append(&rule.parameter.clone().to_xdr(env));
    payload.append(&rule.operator.clone().to_xdr(env));
    payload.append(&rule.value.clone().to_xdr(env));
    env.crypto().sha256(&payload).into()
}

#[test]
fn test_constructor_binds_admin() {
    let (env, admin, _, _, client) = create_test_env();

    // Successful registration confirms contract is initialized
    let trial_record_id = client.register_clinical_trial(
        &admin,
        &String::from_str(&env, "TRIAL001"),
        &String::from_str(&env, "Cancer Treatment Study"),
        &symbol_short!("phase2"),
        &create_protocol_hash(&env),
        &1000,
        &2000,
        &100,
        &String::from_str(&env, "IRB-2024-001"),
    );

    assert_eq!(trial_record_id, 0u64);
}

#[test]
fn test_register_clinical_trial() {
    let (env, _, pi, _, client) = create_test_env();

    let trial_record_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL001"),
        &String::from_str(&env, "Diabetes Study"),
        &symbol_short!("phase3"),
        &create_protocol_hash(&env),
        &1000,
        &5000,
        &200,
        &String::from_str(&env, "IRB-2024-002"),
    );

    let trial_data = client.get_trial(&trial_record_id);
    assert_eq!(trial_data.trial_record_id, trial_record_id);
    assert_eq!(trial_data.principal_investigator, pi);
    assert_eq!(trial_data.enrollment_target, 200);
}

#[test]
fn test_invalid_study_phase() {
    let (env, _, pi, _, client) = create_test_env();

    let result = client.try_register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL001"),
        &String::from_str(&env, "Test Study"),
        &symbol_short!("invalid"),
        &create_protocol_hash(&env),
        &1000,
        &5000,
        &100,
        &String::from_str(&env, "IRB-2024-003"),
    );

    assert!(result.is_err());
}

#[test]
fn test_invalid_date_range() {
    let (env, _, pi, _, client) = create_test_env();

    let result = client.try_register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL001"),
        &String::from_str(&env, "Test Study"),
        &symbol_short!("phase1"),
        &create_protocol_hash(&env),
        &5000,
        &1000, // end before start
        &100,
        &String::from_str(&env, "IRB-2024-004"),
    );

    assert!(result.is_err());
}

#[test]
fn test_withdrawal_policy_enforces_data_retention() {
    let (env, _, pi, patient, client) = create_test_env();

    let trial_record_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL001"),
        &String::from_str(&env, "Withdrawal Policy Study"),
        &symbol_short!("phase2"),
        &create_protocol_hash(&env),
        &1000,
        &2000,
        &100,
        &String::from_str(&env, "IRB-2024-007"),
    );

    let enrollment_id = client.enroll_participant(
        &trial_record_id,
        &patient,
        &symbol_short!("armA"),
        &1100,
        &create_protocol_hash(&env),
        &String::from_str(&env, "PATIENT001"),
    );

    client.withdraw_participant(
        &enrollment_id,
        &1200,
        &symbol_short!("consent"),
        &false,
    );

    let events = env.events().all();
    assert_eq!(events.len(), 5);

    let filters = DataFilters {
        include_withdrawn: true,
        study_arms: Vec::new(&env),
        date_range_start: None,
        date_range_end: None,
    };

    let export_hash = client.export_deidentified_data(&trial_record_id, &pi, &filters);
    let expected_hash = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_slice(&env, &0u32.to_be_bytes()));
    assert_eq!(export_hash, expected_hash);

    let result = client.try_record_study_visit(
        &enrollment_id,
        &1u32,
        &1300,
        &symbol_short!("followup"),
        &create_protocol_hash(&env),
        &Vec::new(&env),
    );
    assert_eq!(result, Err(Ok(Error::WithdrawalRestricted)));

    let event_id = client.report_adverse_event(
        &enrollment_id,
        &symbol_short!("headache"),
        &symbol_short!("moderate"),
        &create_protocol_hash(&env),
        &1300,
        &Option::<u64>::None,
        &symbol_short!("possible"),
    );

    let adverse_event = client.get_adverse_event(&event_id, &pi);
    assert_eq!(adverse_event.enrollment_id, enrollment_id);
}

// ── #484: multi-site enrolment tests ─────────────────────────────────────────────

fn setup_trial_with_sites(
    env: &Env,
    client: &ClinicalTrialContractClient,
    pi: &Address,
) -> (u64, u64, u64, Address, Address) {
    let trial_id = client.register_clinical_trial(
        pi,
        &String::from_str(env, "MULTI-SITE-01"),
        &String::from_str(env, "Multi-Site Study"),
        &symbol_short!("phase2"),
        &create_protocol_hash(env),
        &1000,
        &9999,
        &200, // total cap
        &String::from_str(env, "IRB-2024-100"),
    );

    let coord_a = Address::generate(env);
    let coord_b = Address::generate(env);

    let site_a = client.add_site(&trial_id, pi, &coord_a, &50);
    let site_b = client.add_site(&trial_id, pi, &coord_b, &10);

    (trial_id, site_a, site_b, coord_a, coord_b)
}

#[test]
fn test_enrol_at_site_succeeds() {
    let (env, _, pi, patient, client) = create_test_env();
    let (trial_id, site_a, _, coord_a, _) = setup_trial_with_sites(&env, &client, &pi);

    let enrollment_id = client.enrol_participant_at_site(
        &trial_id,
        &site_a,
        &coord_a,
        &patient,
        &symbol_short!("armA"),
        &1100,
        &create_protocol_hash(&env),
        &String::from_str(&env, "P001"),
    );

    let enrollment = client.get_enrollment(&enrollment_id, &pi);
    assert_eq!(enrollment.site_id, Some(site_a));
    assert_eq!(enrollment.trial_record_id, trial_id);
}

// ── #848: core regulatory function coverage ────────────────

#[test]
fn test_define_eligibility_criteria_and_check_patient_eligibility() {
    let (env, _, pi, patient, client) = create_test_env();

    let trial_record_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-ELIG-01"),
        &String::from_str(&env, "Eligibility Study"),
        &symbol_short!("phase2"),
        &create_protocol_hash(&env),
        &1000,
        &5000,
        &100,
        &String::from_str(&env, "IRB-2024-200"),
    );

    let inclusion = Vec::from_array(
        &env,
        [make_rule(&env, "age", "30")],
    );
    let exclusion = Vec::new(&env);

    client.define_eligibility_criteria(&trial_record_id, &pi, &inclusion, &exclusion);

    let patient_data_hash = create_protocol_hash(&env);
    let evaluation_artifacts = Vec::from_array(
        &env,
        [String::from_str(&env, "age=30")],
    );

    let result = client.check_patient_eligibility(
        &trial_record_id,
        &patient,
        &patient_data_hash,
        &evaluation_artifacts,
    );

    assert!(result.eligible);
    assert_eq!(result.disqualifying_factors.len(), 0);
}

#[test]
fn test_failed_inclusion_rule_appears_in_disqualifying_factors() {
    let (env, _, pi, patient, client) = create_test_env();

    let trial_record_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-ELIG-02"),
        &String::from_str(&env, "Eligibility Failure Study"),
        &symbol_short!("phase2"),
        &create_protocol_hash(&env),
        &1000,
        &5000,
        &100,
        &String::from_str(&env, "IRB-2024-201"),
    );

    let inclusion = Vec::from_array(
        &env,
        [make_rule(&env, "age", "30")],
    );
    let exclusion = Vec::new(&env);

    client.define_eligibility_criteria(&trial_record_id, &pi, &inclusion, &exclusion);

    let patient_data_hash = create_protocol_hash(&env);
    // Artifact does not satisfy the mandatory inclusion rule.
    let evaluation_artifacts = Vec::from_array(
        &env,
        [String::from_str(&env, "age=45")],
    );

    let result = client.check_patient_eligibility(
        &trial_record_id,
        &patient,
        &patient_data_hash,
        &evaluation_artifacts,
    );

    assert!(!result.eligible);
    assert_eq!(result.disqualifying_factors.len(), 1);
}

// ── #1096: DSMB independence, membership locking, and resume flow ─────────────

fn setup_active_trial(
    env: &Env,
    client: &ClinicalTrialContractClient,
    pi: &Address,
) -> u64 {
    client.register_clinical_trial(
        pi,
        &String::from_str(env, "DSMB-TRIAL-01"),
        &String::from_str(env, "DSMB Study"),
        &symbol_short!("phase2"),
        &create_protocol_hash(env),
        &1000,
        &9999,
        &100,
        &String::from_str(env, "IRB-2024-300"),
    )
}

#[test]
fn test_appoint_dsmb_by_sponsor_succeeds_and_emits_event() {
    let (env, admin, pi, _, client) = create_test_env();
    let trial_id = setup_active_trial(&env, &client, &pi);

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);
    let members = Vec::from_array(&env, [m1.clone(), m2.clone(), m3.clone()]);

    let before = env.events().all().len();
    client.appoint_dsmb(&trial_id, &admin, &members);
    let after = env.events().all().len();
    assert!(after > before, "appoint_dsmb must emit an event");

    let dsmb = client.get_dsmb(&trial_id);
    assert_eq!(dsmb.members.len(), 3);
    assert_eq!(dsmb.threshold, 2);
}

#[test]
fn test_appoint_dsmb_rejects_principal_investigator() {
    let (env, admin, pi, _, client) = create_test_env();
    let trial_id = setup_active_trial(&env, &client, &pi);

    let other = Address::generate(&env);
    let members = Vec::from_array(&env, [pi.clone(), other]);

    let result = client.try_appoint_dsmb(&trial_id, &admin, &members);
    assert!(result.is_err(), "PI must not be appointable to the DSMB");
}

#[test]
fn test_appoint_dsmb_rejects_duplicate_members() {
    let (env, admin, pi, _, client) = create_test_env();
    let trial_id = setup_active_trial(&env, &client, &pi);

    let m1 = Address::generate(&env);
    let members = Vec::from_array(&env, [m1.clone(), m1.clone()]);

    let result = client.try_appoint_dsmb(&trial_id, &admin, &members);
    assert!(result.is_err(), "duplicate DSMB members must be rejected");
}

#[test]
fn test_appoint_dsmb_rejects_non_admin() {
    let (env, _, pi, _, client) = create_test_env();
    let trial_id = setup_active_trial(&env, &client, &pi);

    let m1 = Address::generate(&env);
    let members = Vec::from_array(&env, [m1]);

    // The PI is not the sponsor/IRB and must not be able to appoint the board.
    let result = client.try_appoint_dsmb(&trial_id, &pi, &members);
    assert!(result.is_err(), "only sponsor/IRB may appoint the DSMB");
}

#[test]
fn test_dsmb_membership_locked_while_halt_pending() {
    let (env, admin, pi, _, client) = create_test_env();
    let trial_id = setup_active_trial(&env, &client, &pi);

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);
    let members = Vec::from_array(&env, [m1.clone(), m2.clone(), m3.clone()]);
    client.appoint_dsmb(&trial_id, &admin, &members);

    let proposal_id = client.propose_safety_halt(
        &trial_id,
        &m1,
        &String::from_str(&env, "adverse events"),
    );

    // Membership must be frozen while the halt proposal is pending.
    let replacement = Address::generate(&env);
    let new_members = Vec::from_array(&env, [replacement]);
    let result = client.try_appoint_dsmb(&trial_id, &admin, &new_members);
    assert!(result.is_err(), "membership must be locked while halt pending");

    // The original board can still approve the halt.
    client.approve_safety_halt(&trial_id, &proposal_id, &m2);
    client.approve_safety_halt(&trial_id, &proposal_id, &m3);

    let trial = client.get_trial(&trial_id);
    assert_eq!(trial.status, symbol_short!("Suspended"));
}

#[test]
fn test_propose_and_approve_resume_returns_trial_to_active() {
    let (env, admin, pi, _, client) = create_test_env();
    let trial_id = setup_active_trial(&env, &client, &pi);

    let m1 = Address::generate(&env);
    let m2 = Address::generate(&env);
    let m3 = Address::generate(&env);
    let members = Vec::from_array(&env, [m1.clone(), m2.clone(), m3.clone()]);
    client.appoint_dsmb(&trial_id, &admin, &members);

    let halt_id = client.propose_safety_halt(
        &trial_id,
        &m1,
        &String::from_str(&env, "safety review"),
    );
    client.approve_safety_halt(&trial_id, &halt_id, &m2);
    client.approve_safety_halt(&trial_id, &halt_id, &m3);

    let trial = client.get_trial(&trial_id);
    assert_eq!(trial.status, symbol_short!("Suspended"));

    let resume_id = client.propose_resume(
        &trial_id,
        &m1,
        &String::from_str(&env, "review complete"),
    );

    // Still suspended until the DSMB majority approves the resume.
    let trial = client.get_trial(&trial_id);
    assert_eq!(trial.status, symbol_short!("Suspended"));

    let before = env.events().all().len();
    client.approve_resume(&trial_id, &resume_id, &m2);
    client.approve_resume(&trial_id, &resume_id, &m3);
    let after = env.events().all().len();
    assert!(after > before, "resume approval must emit an event");

    let trial = client.get_trial(&trial_id);
    assert_eq!(trial.status, symbol_short!("Active"));
}
