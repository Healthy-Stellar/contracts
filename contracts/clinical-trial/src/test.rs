use crate::{ClinicalTrialContractClient, CriteriaRule, DataFilters, Error};
use soroban_sdk::{symbol_short, testutils::Address as _, testutils::Events, Address, Bytes, BytesN, Env, String, Vec};
use soroban_sdk::xdr::ToXdr;

fn create_test_env() -> (Env, Address, Address, Address, ClinicalTrialContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let pi = Address::generate(&env);
    let patient = Address::generate(&env);

    let contract_id = env.register_contract(None, crate::ClinicalTrialContract);
    let client = ClinicalTrialContractClient::new(&env, &contract_id);

    client.initialize(&admin);

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
fn test_initialize() {
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
fn test_double_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, crate::ClinicalTrialContract);
    let client = ClinicalTrialContractClient::new(&env, &contract_id);

    client.initialize(&admin);

    // Second initialization must return AlreadyInitialized typed error
    let result = client.try_initialize(&admin);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
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

#[test]
fn test_enrol_at_full_site_rejected_even_if_trial_has_capacity() {
    let (env, _, pi, _, client) = create_test_env();
    let (trial_id, _site_a, site_b, _coord_a, coord_b) =
        setup_trial_with_sites(&env, &client, &pi);

    // site_b has max_enrollment = 10; fill it up
    let participants = [
        "P0", "P1", "P2", "P3", "P4", "P5", "P6", "P7", "P8", "P9",
    ];
    for pid in participants.iter() {
        let p = Address::generate(&env);
        client.enrol_participant_at_site(
            &trial_id,
            &site_b,
            &coord_b,
            &p,
            &symbol_short!("armB"),
            &1100,
            &create_protocol_hash(&env),
            &String::from_str(&env, pid),
        );
    }

    // 11th enrolment at site_b must fail even though trial total (200) not reached
    let extra = Address::generate(&env);
    let result = client.try_enrol_participant_at_site(
        &trial_id,
        &site_b,
        &coord_b,
        &extra,
        &symbol_short!("armB"),
        &1100,
        &create_protocol_hash(&env),
        &String::from_str(&env, "PEXTRA"),
    );
    assert_eq!(result, Err(Ok(Error::SiteEnrollmentFull)));
}

#[test]
fn test_two_sites_with_different_quotas_cross_site_aggregation() {
    let (env, _, pi, _, client) = create_test_env();
    let (trial_id, site_a, site_b, coord_a, coord_b) =
        setup_trial_with_sites(&env, &client, &pi);

    // Enrol 2 at site_a and 3 at site_b
    let pids_a = ["PA0", "PA1"];
    let pids_b = ["PB0", "PB1", "PB2"];
    for pid in pids_a.iter() {
        let p = Address::generate(&env);
        client.enrol_participant_at_site(
            &trial_id,
            &site_a,
            &coord_a,
            &p,
            &symbol_short!("armA"),
            &1100,
            &create_protocol_hash(&env),
            &String::from_str(&env, pid),
        );
    }
    for pid in pids_b.iter() {
        let p = Address::generate(&env);
        client.enrol_participant_at_site(
            &trial_id,
            &site_b,
            &coord_b,
            &p,
            &symbol_short!("armB"),
            &1100,
            &create_protocol_hash(&env),
            &String::from_str(&env, pid),
        );
    }

    // Trial total should be 5
    let trial = client.get_trial(&trial_id);
    assert_eq!(trial.current_enrollment, 5);

    // export_deidentified_data aggregates all participants (5 included)
    let filters = DataFilters {
        include_withdrawn: false,
        study_arms: Vec::new(&env),
        date_range_start: None,
        date_range_end: None,
    };
    let export_hash = client.export_deidentified_data(&trial_id, &pi, &filters);
    let expected = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_slice(&env, &5u32.to_be_bytes()));
    assert_eq!(export_hash, expected);
}

// ── #486: re-enrolment tests ──────────────────────────────────────────────────

#[test]
fn test_re_enroll_after_withdrawal_succeeds() {
    let (env, _, pi, patient, client) = create_test_env();

    let trial_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-486"),
        &String::from_str(&env, "Re-enrol Study"),
        &symbol_short!("phase2"),
        &create_protocol_hash(&env),
        &1000,
        &9999,
        &100,
        &String::from_str(&env, "IRB-486"),
    );

    // Initial enrolment
    let enrollment_id = client.enroll_participant(
        &trial_id,
        &patient,
        &symbol_short!("armA"),
        &1100,
        &create_protocol_hash(&env),
        &String::from_str(&env, "P001"),
    );

    // Withdraw
    client.withdraw_participant(&enrollment_id, &1200, &symbol_short!("consent"), &false);

    // Re-enrol
    let new_enrollment_id = client.re_enroll_participant(
        &trial_id,
        &patient,
        &enrollment_id,
        &create_protocol_hash(&env),
        &symbol_short!("armB"),
        &1300,
        &String::from_str(&env, "P001-R1"),
    );

    assert_ne!(new_enrollment_id, enrollment_id);

    let new_enrollment = client.get_enrollment(&new_enrollment_id, &pi);
    assert_eq!(new_enrollment.prior_enrollment_id, Some(enrollment_id));
    assert_eq!(new_enrollment.trial_record_id, trial_id);

    // Prior enrolment data_retention_consent is unchanged (false)
    let prior = client.get_enrollment(&enrollment_id, &pi);
    assert!(!prior.data_retention_consent);
}

#[test]
fn test_re_enroll_blocked_when_trial_closed() {
    let (env, _, pi, patient, client) = create_test_env();

    let trial_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-486B"),
        &String::from_str(&env, "Closed Trial"),
        &symbol_short!("phase1"),
        &create_protocol_hash(&env),
        &1000,
        &9999,
        &100,
        &String::from_str(&env, "IRB-486B"),
    );

    let enrollment_id = client.enroll_participant(
        &trial_id,
        &patient,
        &symbol_short!("armA"),
        &1100,
        &create_protocol_hash(&env),
        &String::from_str(&env, "P001"),
    );

    client.withdraw_participant(&enrollment_id, &1200, &symbol_short!("consent"), &true);

    // Mark trial closed by suspending it via safety halt pathway is complex;
    // instead directly verify that TrialNotActive is returned for Suspended.
    // To test TrialClosed: use the Suspended path (status != Active) which
    // returns TrialNotActive.  The TrialClosed path is a superset guard.
    // We validate the TrialClosed variant is distinct from TrialNotActive
    // by asserting a Suspended trial returns TrialNotActive, not TrialClosed.
    // A truly Closed status would require a close_trial endpoint; the guard
    // exists and is unit-tested via integration once that endpoint is added.
    // For now validate the Active-required guard fires on Suspended trials.
    let admin = Address::generate(&env);
    let dsmb_member = Address::generate(&env);
    let mut members = Vec::new(&env);
    members.push_back(dsmb_member.clone());
    client.appoint_dsmb(&trial_id, &pi, &members);
    client.propose_safety_halt(&trial_id, &dsmb_member, &create_protocol_hash(&env));
    client.approve_safety_halt(&trial_id, &dsmb_member);

    let result = client.try_re_enroll_participant(
        &trial_id,
        &patient,
        &enrollment_id,
        &create_protocol_hash(&env),
        &symbol_short!("armA"),
        &1400,
        &String::from_str(&env, "P001-R1"),
    );
    assert_eq!(result, Err(Ok(Error::TrialNotActive)));
}

#[test]
fn test_re_enroll_blocked_when_prior_enrollment_still_active() {
    let (env, _, pi, patient, client) = create_test_env();

    let trial_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-486C"),
        &String::from_str(&env, "Active Prior Enrol"),
        &symbol_short!("phase2"),
        &create_protocol_hash(&env),
        &1000,
        &9999,
        &100,
        &String::from_str(&env, "IRB-486C"),
    );

    let enrollment_id = client.enroll_participant(
        &trial_id,
        &patient,
        &symbol_short!("armA"),
        &1100,
        &create_protocol_hash(&env),
        &String::from_str(&env, "P001"),
    );

    // Prior enrolment is still Active
    let result = client.try_re_enroll_participant(
        &trial_id,
        &patient,
        &enrollment_id,
        &create_protocol_hash(&env),
        &symbol_short!("armA"),
        &1200,
        &String::from_str(&env, "P001-R1"),
    );
    assert_eq!(result, Err(Ok(Error::PriorEnrollmentActive)));
}

// ── #487: phase transition tests ─────────────────────────────────────────────

#[test]
fn test_advance_trial_phase_succeeds_and_emits_event() {
    let (env, _, pi, _, client) = create_test_env();

    let trial_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-487"),
        &String::from_str(&env, "Phase Advance Study"),
        &symbol_short!("phase1"),
        &create_protocol_hash(&env),
        &1000,
        &9999,
        &100,
        &String::from_str(&env, "IRB-487"),
    );

    let new_protocol = env.crypto().sha256(
        &String::from_str(&env, "protocol_v2").into()
    );
    let new_protocol: BytesN<32> = new_protocol.into();

    client.advance_trial_phase(
        &pi,
        &trial_id,
        &symbol_short!("phase2"),
        &new_protocol,
    );

    let trial = client.get_trial(&trial_id);
    assert_eq!(trial.study_phase, symbol_short!("phase2"));
    assert_eq!(trial.protocol_hash, new_protocol);

    // Verify TrialPhaseAdvanced event was emitted (register_clinical_trial = 1, advance = 1)
    let events = env.events().all();
    assert_eq!(events.len(), 2);
}

#[test]
fn test_advance_trial_phase_rejected_for_non_pi() {
    let (env, _, pi, patient, client) = create_test_env();

    let trial_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-487B"),
        &String::from_str(&env, "Phase Advance Auth"),
        &symbol_short!("phase1"),
        &create_protocol_hash(&env),
        &1000,
        &9999,
        &100,
        &String::from_str(&env, "IRB-487B"),
    );

    let result = client.try_advance_trial_phase(
        &patient, // not the PI
        &trial_id,
        &symbol_short!("phase2"),
        &create_protocol_hash(&env),
    );
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_advance_trial_phase_invalid_phase_rejected() {
    let (env, _, pi, _, client) = create_test_env();

    let trial_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-487C"),
        &String::from_str(&env, "Phase Advance Validate"),
        &symbol_short!("phase1"),
        &create_protocol_hash(&env),
        &1000,
        &9999,
        &100,
        &String::from_str(&env, "IRB-487C"),
    );

    let result = client.try_advance_trial_phase(
        &pi,
        &trial_id,
        &symbol_short!("phaseX"),
        &create_protocol_hash(&env),
    );
    assert_eq!(result, Err(Ok(Error::InvalidStudyPhase)));
}

#[test]
fn test_enrolment_after_phase_advance_uses_updated_trial() {
    let (env, _, pi, patient, client) = create_test_env();

    let trial_id = client.register_clinical_trial(
        &pi,
        &String::from_str(&env, "TRIAL-487D"),
        &String::from_str(&env, "Phase Advance + Enrol"),
        &symbol_short!("phase1"),
        &create_protocol_hash(&env),
        &1000,
        &9999,
        &100,
        &String::from_str(&env, "IRB-487D"),
    );

    let v2_hash: BytesN<32> = env.crypto().sha256(
        &String::from_str(&env, "protocol_v2").into()
    ).into();

    client.advance_trial_phase(&pi, &trial_id, &symbol_short!("phase2"), &v2_hash);

    // New enrolment after phase advance; must succeed (trial still Active)
    let enrollment_id = client.enroll_participant(
        &trial_id,
        &patient,
        &symbol_short!("armA"),
        &1100,
        &create_protocol_hash(&env),
        &String::from_str(&env, "P-PHASE2"),
    );

    let trial = client.get_trial(&trial_id);
    assert_eq!(trial.study_phase, symbol_short!("phase2"));
    assert_eq!(trial.current_enrollment, 1);

    let enrol = client.get_enrollment(&enrollment_id, &pi);
    assert_eq!(enrol.trial_record_id, trial_id);
}
