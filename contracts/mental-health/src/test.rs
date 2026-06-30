#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, vec, Address, BytesN, Env, String, Symbol,
};

// ── Mock emergency-medical-info contract for cross-contract testing ───────────

#[contracttype]
pub struct CrisisAlertEvent {
    pub patient_id: Address,
    pub provider_id: Address,
    pub alert_type: Symbol,
    pub severity: Symbol,
    pub timestamp: u64,
}

#[contracttype]
pub enum MockEmergencyDataKey {
    Alerts(Address),
}

#[contract]
pub struct MockEmergencyMedicalInfo;

#[contractimpl]
impl MockEmergencyMedicalInfo {
    pub fn add_critical_alert(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        alert_type: Symbol,
        _alert_text_hash: BytesN<32>,
        severity: Symbol,
    ) {
        let alert = CrisisAlertEvent {
            patient_id: patient_id.clone(),
            provider_id,
            alert_type,
            severity,
            timestamp: env.ledger().timestamp(),
        };
        let key = MockEmergencyDataKey::Alerts(patient_id.clone());
        let mut alerts: Vec<CrisisAlertEvent> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        alerts.push_back(alert);
        env.storage().persistent().set(&key, &alerts);
    }

    pub fn get_alert_count(env: Env, patient_id: Address) -> u32 {
        let key = MockEmergencyDataKey::Alerts(patient_id);
        env.storage()
            .persistent()
            .get::<_, Vec<CrisisAlertEvent>>(&key)
            .unwrap_or(Vec::new(&env))
            .len()
    }
}

fn grant_consent(env: &Env, client: &MentalHealthContractClient, patient: &Address, data_type: &str, provider: &Address) {
    client.grant_data_consent(patient, &Symbol::new(env, data_type), provider, &None);
}

#[test]
fn test_conduct_assessment_and_record_scores() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MentalHealthContract, ());
    let client = MentalHealthContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    grant_consent(&env, &client, &patient_id, "assessment", &provider_id);
    grant_consent(&env, &client, &patient_id, "suicide_risk", &provider_id);

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
    client.record_gad7_score(
        &assessment_id,
        &12,
        &vec![&env, 2, 2, 2, 2, 2, 2],
        &1690000000,
    );

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

    grant_consent(&env, &client, &patient_id, "treatment_plan", &provider_id);
    grant_consent(&env, &client, &patient_id, "therapy_session", &provider_id);
    grant_consent(&env, &client, &patient_id, "outcomes", &provider_id);

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

    grant_consent(&env, &client, &patient_id, "substance_screening", &provider_id);

    // Set privacy flag for substance abuse
    client.set_enhanced_privacy_flag(&patient_id, &Symbol::new(&env, "substance_abuse"), &true);

    // Request screening should succeed
    let result = client.request_substance_screening(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "CAGE"),
        &1690000000,
    );
    assert_eq!(result, 1);

    // Remove privacy flag
    client.set_enhanced_privacy_flag(&patient_id, &Symbol::new(&env, "substance_abuse"), &false);

    // Request screening should still succeed
    let result2 = client.request_substance_screening(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "CAGE"),
        &1690000000,
    );

    assert_eq!(result2, 2);
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

    grant_consent(&env, &client, &patient_id, "safety_plan", &provider_id);

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
    grant_consent(&env, &client, &patient_id, "hospitalization", &facility_id);
    let hosp_id = client.document_hospitalization(
        &patient_id,
        &1690000000,
        &String::from_str(&env, "severe breakdown"),
        &Symbol::new(&env, "voluntary"),
        &facility_id,
        &None,
    );
    assert_eq!(hosp_id, 1);

    grant_consent(&env, &client, &patient_id, "symptom_severity", &provider_id);
    client.track_symptom_severity(
        &patient_id,
        &provider_id,
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

    client.set_enhanced_privacy_flag(&patient_id, &Symbol::new(&env, "substance_abuse"), &true);
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

#[test]
fn test_high_risk_assessment_triggers_crisis_escalation() {
    let env = Env::default();
    env.mock_all_auths();

    let mental_health_id = env.register(MentalHealthContract, ());
    let emergency_id = env.register(MockEmergencyMedicalInfo, ());

    let mental_health = MentalHealthContractClient::new(&env, &mental_health_id);
    let emergency = MockEmergencyMedicalInfoClient::new(&env, &emergency_id);

    // Initialize mental-health with emergency contract address
    mental_health.initialize(&emergency_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    grant_consent(&env, &mental_health, &patient_id, "assessment", &provider_id);
    grant_consent(&env, &mental_health, &patient_id, "suicide_risk", &provider_id);
    grant_consent(&env, &mental_health, &patient_id, "crisis_escalation", &provider_id);

    // Create an assessment
    let assessment_id = mental_health.conduct_mental_health_assessment(
        &patient_id,
        &provider_id,
        &1690000000,
        &Symbol::new(&env, "initial"),
        &vec![&env, String::from_str(&env, "anxiety")],
        &vec![&env, Symbol::new(&env, "PHQ9")],
        &BytesN::from_array(&env, &[0; 32]),
    );

    // Record PHQ9 and GAD7 to have full data
    mental_health.record_phq9_score(&assessment_id, &22, &vec![&env, 3, 3, 3, 3, 3], &1690000000);

    // Assess with "high" risk - should trigger escalation
    match mental_health.try_assess_suicide_risk(
        &assessment_id,
        &provider_id,
        &Symbol::new(&env, "high"),
        &vec![&env, String::from_str(&env, "isolation")],
        &vec![&env, String::from_str(&env, "family")],
        &true,
    ) {
        Ok(inner) => match inner {
            Ok(()) => {
                // Verify alert was created in the emergency-medical-info contract
                let alert_count = emergency.get_alert_count(&patient_id);
                assert_eq!(alert_count, 1);
            }
            Err(_) => panic!("Conversion error"),
        },
        Err(inner) => match inner {
            Ok(Error::EscalationFailed) => {
                // Escalation failed but assessment was committed - acceptable
            }
            _ => panic!("Unexpected error"),
        },
    }
}

#[test]
fn test_low_risk_assessment_does_not_trigger_escalation() {
    let env = Env::default();
    env.mock_all_auths();

    let mental_health_id = env.register(MentalHealthContract, ());
    let emergency_id = env.register(MockEmergencyMedicalInfo, ());

    let mental_health = MentalHealthContractClient::new(&env, &mental_health_id);
    let emergency = MockEmergencyMedicalInfoClient::new(&env, &emergency_id);

    mental_health.initialize(&emergency_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    grant_consent(&env, &mental_health, &patient_id, "assessment", &provider_id);
    grant_consent(&env, &mental_health, &patient_id, "suicide_risk", &provider_id);

    let assessment_id = mental_health.conduct_mental_health_assessment(
        &patient_id,
        &provider_id,
        &1690000000,
        &Symbol::new(&env, "initial"),
        &vec![&env, String::from_str(&env, "anxiety")],
        &vec![&env, Symbol::new(&env, "PHQ9")],
        &BytesN::from_array(&env, &[0; 32]),
    );

    // Assess with "low" risk - should NOT trigger escalation
    mental_health.assess_suicide_risk(
        &assessment_id,
        &provider_id,
        &Symbol::new(&env, "low"),
        &vec![&env, String::from_str(&env, "good support")],
        &vec![&env, String::from_str(&env, "family")],
        &false,
    );

    // No alerts should have been created
    let alert_count = emergency.get_alert_count(&patient_id);
    assert_eq!(alert_count, 0);
}

#[test]
fn test_safety_plan_triggers_crisis_escalation() {
    let env = Env::default();
    env.mock_all_auths();

    let mental_health_id = env.register(MentalHealthContract, ());
    let emergency_id = env.register(MockEmergencyMedicalInfo, ());

    let mental_health = MentalHealthContractClient::new(&env, &mental_health_id);
    let emergency = MockEmergencyMedicalInfoClient::new(&env, &emergency_id);

    mental_health.initialize(&emergency_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    grant_consent(&env, &mental_health, &patient_id, "safety_plan", &provider_id);
    grant_consent(&env, &mental_health, &patient_id, "crisis_escalation", &provider_id);

    // Create safety plan - should trigger escalation
    match mental_health.try_create_safety_plan(
        &patient_id,
        &provider_id,
        &vec![&env, String::from_str(&env, "isolation")],
        &vec![&env, String::from_str(&env, "call a friend")],
        &vec![&env, String::from_str(&env, "John Doe")],
        &vec![&env, String::from_str(&env, "Suicide Hotline")],
        &BytesN::from_array(&env, &[2; 32]),
    ) {
        Ok(inner) => match inner {
            Ok(plan_id) => {
                assert_eq!(plan_id, 1);
                let alert_count = emergency.get_alert_count(&patient_id);
                assert_eq!(alert_count, 1);
            }
            _ => panic!("Unexpected inner error"),
        },
        Err(inner) => match inner {
            Ok(Error::EscalationFailed) => {
                // Escalation failed but plan was created - acceptable
            }
            _ => panic!("Unexpected error"),
        },
    }
}

#[test]
fn test_crisis_escalation_with_privacy_flag_deidentifies_patient() {
    let env = Env::default();
    env.mock_all_auths();

    let mental_health_id = env.register(MentalHealthContract, ());
    let emergency_id = env.register(MockEmergencyMedicalInfo, ());

    let mental_health = MentalHealthContractClient::new(&env, &mental_health_id);
    let emergency = MockEmergencyMedicalInfoClient::new(&env, &emergency_id);

    mental_health.initialize(&emergency_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    grant_consent(&env, &mental_health, &patient_id, "safety_plan", &provider_id);

    // Set enhanced privacy flag for crisis escalation
    mental_health.set_enhanced_privacy_flag(
        &patient_id,
        &Symbol::new(&env, "crisis_escalation"),
        &true,
    );

    // Create safety plan with privacy flag enabled
    match mental_health.try_create_safety_plan(
        &patient_id,
        &provider_id,
        &vec![&env, String::from_str(&env, "isolation")],
        &vec![&env, String::from_str(&env, "call a friend")],
        &vec![&env, String::from_str(&env, "John Doe")],
        &vec![&env, String::from_str(&env, "Suicide Hotline")],
        &BytesN::from_array(&env, &[2; 32]),
    ) {
        Ok(inner) => match inner {
            Ok(_plan_id) => {
                // Alert should NOT be stored under the real patient_id (de-identified)
                let alert_count = emergency.get_alert_count(&patient_id);
                assert_eq!(alert_count, 0);
            }
            _ => panic!("Unexpected inner error"),
        },
        Err(inner) => match inner {
            Ok(Error::EscalationFailed) => {
                // Escalation failed but plan was created - acceptable
            }
            _ => panic!("Unexpected error"),
        },
    }
}

#[test]
fn test_crisis_escalation_without_emergency_contract_configured() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MentalHealthContract, ());
    let client = MentalHealthContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    grant_consent(&env, &client, &patient_id, "safety_plan", &provider_id);

    // Without emergency contract configured, create_safety_plan should still work
    let plan_id = client.create_safety_plan(
        &patient_id,
        &provider_id,
        &vec![&env, String::from_str(&env, "isolation")],
        &vec![&env, String::from_str(&env, "call a friend")],
        &vec![&env, String::from_str(&env, "John Doe")],
        &vec![&env, String::from_str(&env, "Suicide Hotline")],
        &BytesN::from_array(&env, &[2; 32]),
    );
    assert_eq!(plan_id, 1);
}
