use crate::{ClinicalTrialClient, DataFilters, AdverseEvent, CriteriaRule};
use soroban_sdk::{symbol_short, testutils::Address as _, vec, Address, BytesN, Env, String, Vec};

fn create_test_env() -> (Env, Address, Address, Address, ClinicalTrialClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let pi = Address::generate(&env);
    let patient = Address::generate(&env);

    let contract_id = env.register_contract(None, crate::ClinicalTrial);
    let client = ClinicalTrialClient::new(&env, &contract_id);

    client.initialize(&admin);

    (env, admin, pi, patient, client)
}

fn create_protocol_hash(env: &Env) -> BytesN<32> {
    let data = String::from_str(env, "protocol_v1");
    env.crypto().sha256(&data.into())
}

fn create_consent_hash(env: &Env) -> BytesN<32> {
    let data = String::from_str(env, "informed_consent");
    env.crypto().sha256(&data.into())
}

fn create_data_hash(env: &Env) -> BytesN<32> {
    let data = String::from_str(env, "patient_data");
    env.crypto().sha256(&data.into())
}

#[test]
fn test_initialize() {
    let (env, admin, _, _, client) = create_test_env();
    
    // Contract should be initialized successfully
    let trial_id = client.register_clinical_trial(
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
    
    assert!(trial_id.is_ok());
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_double_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, crate::ClinicalTrial);
    let client = ClinicalTrialClient::new(&env, &contract_id);

    client.initialize(&admin);
    client.initialize(&admin); // Should panic
}

#[test]
fn test_register_clinical_trial() {
    let (env, _, pi, _, client) = create_test_env();

    let trial_id = client.register_clinical_trial(
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

    assert!(trial_id.is_ok());
    let trial_record_id = trial_id.unwrap();

    let trial = client.get_trial(&trial_record_id);
    assert!(trial.is_ok());
    
    let trial_data = trial.unwrap();
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
        &1000, 