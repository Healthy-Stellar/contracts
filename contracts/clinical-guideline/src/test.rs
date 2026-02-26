#![cfg(test)]
use super::*;
use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Vec, testutils::Address as _, testutils::Ledger,
};

#[test]
fn test_register_and_evaluate_guideline() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let guideline_id = String::from_str(&env, "G123");
    let condition = String::from_str(&env, "Flu");
    let criteria_hash = BytesN::from_array(&env, &[0u8; 32]);
    let recommendation_hash = BytesN::from_array(&env, &[1u8; 32]);
    let evidence_level = Symbol::new(&env, "Level_A");

    // Register guideline (Mocking auth)
    env.mock_all_auths();
    client.register_clinical_guideline(
        &admin,
        &guideline_id,
        &condition,
        &criteria_hash,
        &recommendation_hash,
        &evidence_level,
    );

    // Evaluate: Match
    let result = client.evaluate_guideline(
        &Address::generate(&env),
        &Address::generate(&env),
        &guideline_id,
        &criteria_hash,
    );
    assert!(result.applicable);
    assert_eq!(result.evidence_level, evidence_level);

    // Evaluate: No Match (different hash)
    let wrong_hash = BytesN::from_array(&env, &[2u8; 32]);
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

    let weight_dg = 700000; // 70kg = 70000g = 700000dg
    let result = client.calculate_drug_dosage(
        &Address::generate(&env),
        &String::from_str(&env, "Amoxicillin"),
        &weight_dg,
        &30,
        &Some(50), // Renal impairment < 60
    );

    assert_eq!(result.renal_adjustment, true);
    assert_eq!(result.medication, String::from_str(&env, "Amoxicillin"));
    // (700000 * 5) / 10000 = 350
    assert_eq!(result.duration, Some(350));
}

#[test]
fn test_risk_score_assessment() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let mut params = Vec::new(&env);
    params.push_back(10);
    params.push_back(20);
    params.push_back(5);

    let result = client.assess_risk_score(
        &Address::generate(&env),
        &Symbol::new(&env, "SCORE_X"),
        &params,
    );

    assert_eq!(result.score, 35);
    assert_eq!(result.calculator, Symbol::new(&env, "SCORE_X"));
}

#[test]
fn test_care_pathway_suggestion() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let condition = String::from_str(&env, "Diabetes");
    let result = client.suggest_care_pathway(&Address::generate(&env), &condition, &Vec::new(&env));

    assert_eq!(result.condition, condition);
    assert!(result.steps.len() >= 3);
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

    assert!(alerts.len() >= 2);
    assert!(alerts.contains(Symbol::new(&env, "Screening_A")));
}

#[test]
fn test_reminders() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let due_date = 1000000;

    env.ledger().with_mut(|li| {
        li.timestamp = 12345;
    });

    let reminder_id = client.create_reminder(
        &patient,
        &Address::generate(&env),
        &Symbol::new(&env, "MEDS"),
        &due_date,
        &Symbol::new(&env, "HIGH"),
    );

    assert_eq!(reminder_id, 12345);
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_unauthorized_registration() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClinicalGuidelineContract);
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.register_clinical_guideline(
        &admin,
        &String::from_str(&env, "FAIL"),
        &String::from_str(&env, "NA"),
        &BytesN::from_array(&env, &[0u8; 32]),
        &BytesN::from_array(&env, &[0u8; 32]),
        &Symbol::new(&env, "B"),
    );
}
