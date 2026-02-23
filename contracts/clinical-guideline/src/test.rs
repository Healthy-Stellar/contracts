#![cfg(test)]
use super::*;
use soroban_sdk::{Address, BytesN, Env, String, Symbol, Vec, testutils::Address as _};

#[test]
fn test_register_and_evaluate_guideline() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guideline_id = String::from_str(&env, "G123");
    let criteria_hash = BytesN::from_array(&env, &[0u8; 32]);

    // Register guideline (Mocking auth)
    env.mock_all_auths();
    client.register_clinical_guideline(
        &admin,
        &guideline_id,
        &String::from_str(&env, "Flu"),
        &criteria_hash,
        &criteria_hash,
        &Symbol::new(&env, "A"),
    );

    // Evaluate: Match
    let result = client.evaluate_guideline(
        &Address::generate(&env),
        &Address::generate(&env),
        &guideline_id,
        &criteria_hash,
    );
    assert!(result.applicable);

    // Evaluate: No Match (different hash)
    let wrong_hash = BytesN::from_array(&env, &[1u8; 32]);
    let result_fail = client.evaluate_guideline(
        &Address::generate(&env),
        &Address::generate(&env),
        &guideline_id,
        &wrong_hash,
    );
    assert!(!result_fail.applicable);
}

#[test]
fn test_drug_dosage_calculation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let weight_grams = 70000; // 70kg
    let result = client.calculate_drug_dosage(
        &Address::generate(&env),
        &String::from_str(&env, "Amoxicillin"),
        &weight_grams,
        &30,
        &Some(50), // Renal impairment < 60
    );

    assert_eq!(result.renal_adjustment, true);
    assert_eq!(result.medication, String::from_str(&env, "Amoxicillin"));
}

#[test]
fn test_preventive_care_logic() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    // Test for older patient
    let alerts = client.check_preventive_care(
        &Address::generate(&env),
        &55,
        &Symbol::new(&env, "M"),
        &Vec::new(&env),
    );

    // Should have at least Cardiac and Blood Pressure checks
    assert!(alerts.len() >= 2);
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_unauthorized_registration() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // We are NOT calling env.mock_all_auths()
    // This will trigger the Host's built-in Auth panic
    client.register_clinical_guideline(
        &admin,
        &String::from_str(&env, "FAIL"),
        &String::from_str(&env, "NA"),
        &BytesN::from_array(&env, &[0u8; 32]),
        &BytesN::from_array(&env, &[0u8; 32]),
        &Symbol::new(&env, "B"),
    );
}
