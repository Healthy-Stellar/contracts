#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, BytesN, Env, String, Symbol};

#[test]
fn test_conduct_assessment_and_record_scores() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MentalHealthContract, ());
    let client = MentalHealthContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let concerns = vec![&env, String::from_str(&env, "anxiety")];
    let tools = vec![&env, Symbol::new(&env, "PHQ9")];
    let hash = BytesN::from_array(&env, &[0; 32]);

    let assessment_id = client.conduct_mental_health_assessment(
        &patient_id,
        &provider_id,
        &1690000000,
        &Symbol::new(&env, "initial"),
        &concerns,
        &tools,
        &hash,
    );

    assert_eq!(assessment_id, 1);

    // Record PHQ9
    client.record_phq9_score(&assessment_id, &15, &vec![&env, 3, 3, 3, 3, 3], &1690000000);
    // Record GAD7
    client.record_gad7_score(&assessment_id, &12, &vec![&env, 2, 2, 2, 2, 2, 2], &1690000000);

    let risk_factors = vec![&env, String::from_str(&env, "isolation")];
    let protective_factors = vec![&env, String::from_str(&env, "family")];
    
    client.assess_suicide_risk(
        &assessment_id,
        &provider_id,
        &Symbol::new(&env, "moderate"),
        &risk_factors,
        &protective_factors,
        &true,
    );
}

#[test]
fn test_treatment_plan_and_outcomes() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MentalHealthContract, ());
    let client = MentalHealthContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let diagnoses = vec![&env, String::from_str(&env, "F32.1")];
    let goals = vec![
        &env,
        TreatmentGoal {
            goal_description: String::from_str(&env, "Reduce PHQ9 < 10"),
            target_date: 1700000000,
            measurement_method: String::from_str(&env, "PHQ-9"),
            status: Symbol::new(&env, "active"),
        },
    ];
    let interventions = vec![&env, String::from_str(&env, "CBT")];

    let plan_id = client.create_treatment_plan(
        &patient_id,
        &provider_id,
        &diagnoses,
        &goals,
        &interventions,
        &String::from_str(&env, "weekly"),
        &1700000000,
    );
    assert_eq!(plan_id, 1);

    let hash = BytesN::from_array(&env, &[1; 32]);
    client.record_therapy_session(
        &plan_id,
        &1690000000,
        &Symbol::new(&env, "individual"),
        &45,
        &vec![&env, String::from_str(&env, "Cognitive Restructuring")],
        &hash,
        &None,
    );

    let outcomes = vec![
        &env,
        OutcomeMeasure {
            measure_name: String::from_str(&env, "PHQ-9"),
            baseline_score: 15,
            current_score: 8,
            improvement_percentage: 46,
        },
    ];

    client.track_treatment_outcomes(&plan_id, &1690500000, &outcomes, &true);
}

#[test]
fn test_privacy_and_screening() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MentalHealthContract, ());
    let client = MentalHealthContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    // Set privacy flag for substance abuse
    client.set_enhanced_privacy_flag(
        &patient_id,
        &Symbol::new(&env, "substance_abuse"),
        &true,
    );

    // Request screening should fail due to privacy flag
    let result = client.try_request_substance_screening(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "CAGE"),
        &1690000000,
    );

    assert_eq!(result.is_err(), true);

    // Remove privacy flag
    client.set_enhanced_privacy_flag(
        &patient_id,
        &Symbol::new(&env, "substance_abuse"),
        &false,
    );

    // Request screening should succeed
    let result2 = client.request_substance_screening(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "CAGE"),
        &1690000000,
    );

    assert_eq!(result2, 1);
}

#[test]
fn test_safety_plan_and_hospitalization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MentalHealthContract, ());
    let client = MentalHealthContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[2; 32]);

    let plan_id = client.create_safety_plan(
        &patient_id,
        &provider_id,
        &vec![&env, String::from_str(&env, "isolation")],
        &vec![&env, String::from_str(&env, "call a friend")],
        &vec![&env, String::from_str(&env, "John Doe")],
        &vec![&env, String::from_str(&env, "Suicide Hotline")],
        &hash,
    );
    assert_eq!(plan_id, 1);

    let facility_id = Address::generate(&env);
    let hosp_id = client.document_hospitalization(
        &patient_id,
        &1690000000,
        &String::from_str(&env, "severe breakdown"),
        &Symbol::new(&env, "voluntary"),
        &facility_id,
        &None,
    );
    assert_eq!(hosp_id, 1);

    client.track_symptom_severity(
        &patient_id,
        &Symbol::new(&env, "panic"),
        &8,
        &1690000000,
        &Symbol::new(&env, "self_report"),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_invalid_privacy_flag_auth() {
    let env = Env::default();
    // env.mock_all_auths() is NOT called, so auth will fail
    let contract_id = env.register(MentalHealthContract, ());
    let client = MentalHealthContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    
    client.set_enhanced_privacy_flag(
        &patient_id,
        &Symbol::new(&env, "substance_abuse"),
        &true,
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_assess_suicide_risk_unauth() {
    let env = Env::default();
    // env.mock_all_auths() is NOT called
    let contract_id = env.register(MentalHealthContract, ());
    let client = MentalHealthContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[0; 32]);
    let concerns = vec![&env, String::from_str(&env, "anxiety")];
    let tools = vec![&env, Symbol::new(&env, "PHQ9")];

    // auth should fail on this method directly
    client.conduct_mental_health_assessment(
        &patient_id,
        &provider_id,
        &1690000000,
        &Symbol::new(&env, "initial"),
        &concerns,
        &tools,
        &hash,
    );
}
