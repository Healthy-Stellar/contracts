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
