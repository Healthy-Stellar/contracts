#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec};

fn create_test_env() -> (Env, Address, Address) {
    let env = Env::default();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    (env, patient, therapist)
}

#[test]
fn test_conduct_pt_evaluation() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited ROM")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Lower back pain"),
        &String::from_str(&env, "Chronic pain"),
        &limitations,
        &String::from_str(&env, "Independent"),
        &eval_hash,
    );

    assert_eq!(eval_id, 1);

    let evaluation = client.get_evaluation(&eval_id);
    assert_eq!(evaluation.patient_id, patient);
    assert_eq!(evaluation.therapist_id, therapist);
}

#[test]
fn test_assess_range_of_motion() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited ROM")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Shoulder injury"),
        &String::from_str(&env, "Pain on movement"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    client.assess_range_of_motion(
        &eval_id,
        &String::from_str(&env, "Shoulder"),
        &String::from_str(&env, "Flexion"),
        &120u32,
        &Some(5u32),
        &Some(String::from_str(&env, "Moderate")),
    );

    let rom_assessments = client.get_rom_assessments(&eval_id);
    assert_eq!(rom_assessments.len(), 1);
    assert_eq!(rom_assessments.get(0).unwrap().degrees, 120);
}

#[test]
fn test_assess_strength() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Weakness")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Muscle weakness"),
        &String::from_str(&env, "Reduced strength"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    client.assess_strength(
        &eval_id,
        &String::from_str(&env, "Quadriceps"),
        &String::from_str(&env, "4/5"),
        &Symbol::new(&env, "right"),
    );

    let strength_assessments = client.get_strength_assessments(&eval_id);
    assert_eq!(strength_assessments.len(), 1);
}

#[test]
fn test_assess_balance_mobility() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Balance issues")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Fall risk"),
        &String::from_str(&env, "Unsteady gait"),
        &limitations,
        &String::from_str(&env, "Independent"),
        &eval_hash,
    );

    client.assess_balance_mobility(
        &eval_id,
        &Symbol::new(&env, "berg"),
        &45u32,
        &Symbol::new(&env, "moderate"),
    );

    let balance_assessments = client.get_balance_mobility_assessments(&eval_id);
    assert_eq!(balance_assessments.len(), 1);
    assert_eq!(balance_assessments.get(0).unwrap().score, 45);
}

#[test]
fn test_create_rehab_treatment_plan() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited mobility")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Knee injury"),
        &String::from_str(&env, "Pain and stiffness"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(&env, "stg"),
        goal_description: String::from_str(&env, "Reduce pain to 3/10"),
        target_date: 2000u64,
        measurement_method: String::from_str(&env, "VAS scale"),
        achieved: false,
    };

    let ltg_goal = RehabGoal {
        goal_id: 2,
        goal_type: Symbol::new(&env, "ltg"),
        goal_description: String::from_str(&env, "Return to full activity"),
        target_date: 5000u64,
        measurement_method: String::from_str(&env, "Functional assessment"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(&env, "exercise"),
        description: String::from_str(&env, "Quad strengthening"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: Some(String::from_str(&env, "5 lbs")),
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        &therapist,
        &Vec::from_array(&env, [stg_goal]),
        &Vec::from_array(&env, [ltg_goal]),
        &Vec::from_array(&env, [intervention]),
        &String::from_str(&env, "3x/week"),
        &8u32,
        &Symbol::new(&env, "good"),
    );

    assert_eq!(plan_id, 1);

    let plan = client.get_treatment_plan(&plan_id);
    assert_eq!(plan.evaluation_id, eval_id);
    assert_eq!(plan.duration_weeks, 8);
}

#[test]
fn test_document_therapy_session() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Injury"),
        &String::from_str(&env, "Pain"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(&env, "stg"),
        goal_description: String::from_str(&env, "Goal"),
        target_date: 2000u64,
        measurement_method: String::from_str(&env, "Method"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(&env, "exercise"),
        description: String::from_str(&env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        &therapist,
        &Vec::from_array(&env, [stg_goal.clone()]),
        &Vec::from_array(&env, [stg_goal]),
        &Vec::from_array(&env, [intervention.clone()]),
        &String::from_str(&env, "3x/week"),
        &8u32,
        &Symbol::new(&env, "good"),
    );

    client.document_therapy_session(
        &plan_id,
        &1500u64,
        &Vec::from_array(&env, [intervention]),
        &45u32,
        &String::from_str(&env, "Tolerated well"),
        &Some(String::from_str(&env, "Home exercises")),
    );

    let sessions = client.get_therapy_sessions(&plan_id);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions.get(0).unwrap().session_duration_minutes, 45);
}

#[test]
fn test_track_pain_level() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Injury"),
        &String::from_str(&env, "Pain"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(&env, "stg"),
        goal_description: String::from_str(&env, "Goal"),
        target_date: 2000u64,
        measurement_method: String::from_str(&env, "Method"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(&env, "exercise"),
        description: String::from_str(&env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        &therapist,
        &Vec::from_array(&env, [stg_goal.clone()]),
        &Vec::from_array(&env, [stg_goal]),
        &Vec::from_array(&env, [intervention]),
        &String::from_str(&env, "3x/week"),
        &8u32,
        &Symbol::new(&env, "good"),
    );

    let quality = Vec::from_array(&env, [String::from_str(&env, "Sharp")]);

    client.track_pain_level(
        &plan_id,
        &1500u64,
        &Symbol::new(&env, "vas"),
        &6u32,
        &String::from_str(&env, "Lower back"),
        &quality,
    );

    let pain_measurements = client.get_pain_measurements(&plan_id);
    assert_eq!(pain_measurements.len(), 1);
    assert_eq!(pain_measurements.get(0).unwrap().pain_score, 6);
}

#[test]
fn test_measure_functional_outcome() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Injury"),
        &String::from_str(&env, "Pain"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(&env, "stg"),
        goal_description: String::from_str(&env, "Goal"),
        target_date: 2000u64,
        measurement_method: String::from_str(&env, "Method"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(&env, "exercise"),
        description: String::from_str(&env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        &therapist,
        &Vec::from_array(&env, [stg_goal.clone()]),
        &Vec::from_array(&env, [stg_goal]),
        &Vec::from_array(&env, [intervention]),
        &String::from_str(&env, "3x/week"),
        &8u32,
        &Symbol::new(&env, "good"),
    );

    client.measure_functional_outcome(
        &plan_id,
        &1500u64,
        &Symbol::new(&env, "oswestry"),
        &25u32,
        &true,
    );

    let outcomes = client.get_functional_outcomes(&plan_id);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes.get(0).unwrap().score, 25);
}

#[test]
fn test_request_therapy_authorization() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Injury"),
        &String::from_str(&env, "Pain"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(&env, "stg"),
        goal_description: String::from_str(&env, "Goal"),
        target_date: 2000u64,
        measurement_method: String::from_str(&env, "Method"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(&env, "exercise"),
        description: String::from_str(&env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        &therapist,
        &Vec::from_array(&env, [stg_goal.clone()]),
        &Vec::from_array(&env, [stg_goal]),
        &Vec::from_array(&env, [intervention]),
        &String::from_str(&env, "3x/week"),
        &8u32,
        &Symbol::new(&env, "good"),
    );

    let justification_hash = BytesN::from_array(&env, &[2u8; 32]);

    let auth_id = client.request_therapy_authorization(&plan_id, &12u32, &justification_hash);

    assert_eq!(auth_id, 1);
}

#[test]
fn test_document_progress_note() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Injury"),
        &String::from_str(&env, "Pain"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(&env, "stg"),
        goal_description: String::from_str(&env, "Goal"),
        target_date: 2000u64,
        measurement_method: String::from_str(&env, "Method"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(&env, "exercise"),
        description: String::from_str(&env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        &therapist,
        &Vec::from_array(&env, [stg_goal.clone()]),
        &Vec::from_array(&env, [stg_goal]),
        &Vec::from_array(&env, [intervention]),
        &String::from_str(&env, "3x/week"),
        &8u32,
        &Symbol::new(&env, "good"),
    );

    let objective_findings = Vec::from_array(&env, [String::from_str(&env, "ROM improved")]);
    let plan_mods = Vec::from_array(&env, [String::from_str(&env, "Increase resistance")]);

    client.document_progress_note(
        &plan_id,
        &1500u64,
        &String::from_str(&env, "Patient reports less pain"),
        &objective_findings,
        &String::from_str(&env, "Progressing well"),
        &plan_mods,
    );

    let notes = client.get_progress_notes(&plan_id);
    assert_eq!(notes.len(), 1);
}

#[test]
fn test_discharge_from_therapy() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "Injury"),
        &String::from_str(&env, "Pain"),
        &limitations,
        &String::from_str(&env, "Active"),
        &eval_hash,
    );

    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(&env, "stg"),
        goal_description: String::from_str(&env, "Goal"),
        target_date: 2000u64,
        measurement_method: String::from_str(&env, "Method"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(&env, "exercise"),
        description: String::from_str(&env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        &therapist,
        &Vec::from_array(&env, [stg_goal.clone()]),
        &Vec::from_array(&env, [stg_goal]),
        &Vec::from_array(&env, [intervention]),
        &String::from_str(&env, "3x/week"),
        &8u32,
        &Symbol::new(&env, "good"),
    );

    let final_outcomes_hash = BytesN::from_array(&env, &[3u8; 32]);
    let hep_hash = BytesN::from_array(&env, &[4u8; 32]);
    let goals_met = Vec::from_array(&env, [1u64]);

    client.discharge_from_therapy(
        &plan_id,
        &5000u64,
        &Symbol::new(&env, "goals_met"),
        &goals_met,
        &final_outcomes_hash,
        &hep_hash,
    );

    let discharge = client.get_discharge_record(&plan_id);
    assert_eq!(discharge.discharge_date, 5000u64);
}

#[test]
fn test_complete_rehab_workflow() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();

    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    // 1. Conduct evaluation
    let eval_hash = BytesN::from_array(&env, &[1u8; 32]);
    let limitations = Vec::from_array(&env, [String::from_str(&env, "Limited ROM")]);

    let eval_id = client.conduct_pt_evaluation(
        &patient,
        &therapist,
        &1000u64,
        &String::from_str(&env, "ACL tear"),
        &String::from_str(&env, "Knee instability"),
        &limitations,
        &String::from_str(&env, "Athlete"),
        &eval_hash,
    );

    // 2. Assess ROM
    client.assess_range_of_motion(
        &eval_id,
        &String::from_str(&env, "Knee"),
        &String::from_str(&env, "Flexion"),
        &90u32,
        &Some(4u32),
        &Some(String::from_str(&env, "Moderate")),
    );

    // 3. Assess strength
    client.assess_strength(
        &eval_id,
        &String::from_str(&env, "Quadriceps"),
        &String::from_str(&env, "3/5"),
        &Symbol::new(&env, "right"),
    );

    // 4. Create treatment plan
    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(&env, "stg"),
        goal_description: String::from_str(&env, "Increase ROM to 120 degrees"),
        target_date: 2000u64,
        measurement_method: String::from_str(&env, "Goniometry"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(&env, "exercise"),
        description: String::from_str(&env, "Quad sets"),
        sets: Some(3),
        reps: Some(15),
        duration: None,
        resistance: None,
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        &therapist,
        &Vec::from_array(&env, [stg_goal.clone()]),
        &Vec::from_array(&env, [stg_goal]),
        &Vec::from_array(&env, [intervention.clone()]),
        &String::from_str(&env, "3x/week"),
        &12u32,
        &Symbol::new(&env, "excellent"),
    );

    // 5. Document session
    client.document_therapy_session(
        &plan_id,
        &1500u64,
        &Vec::from_array(&env, [intervention]),
        &60u32,
        &String::from_str(&env, "Good effort"),
        &Some(String::from_str(&env, "Daily stretching")),
    );

    // 6. Track pain
    let quality = Vec::from_array(&env, [String::from_str(&env, "Aching")]);
    client.track_pain_level(
        &plan_id,
        &1500u64,
        &Symbol::new(&env, "numeric"),
        &4u32,
        &String::from_str(&env, "Knee"),
        &quality,
    );

    // 7. Measure outcome
    client.measure_functional_outcome(
        &plan_id,
        &1500u64,
        &Symbol::new(&env, "lefs"),
        &55u32,
        &true,
    );

    // Verify all data
    let evaluation = client.get_evaluation(&eval_id);
    assert_eq!(evaluation.patient_id, patient);

    let rom_assessments = client.get_rom_assessments(&eval_id);
    assert_eq!(rom_assessments.len(), 1);

    let sessions = client.get_therapy_sessions(&plan_id);
    assert_eq!(sessions.len(), 1);

    let pain_measurements = client.get_pain_measurements(&plan_id);
    assert_eq!(pain_measurements.len(), 1);

    let outcomes = client.get_functional_outcomes(&plan_id);
    assert_eq!(outcomes.len(), 1);
}

// ── Measurable goal and progress tracking tests (#412) ───────────────────────

fn create_plan(
    env: &Env,
    client: &RehabilitationServicesContractClient,
    patient: &Address,
    therapist: &Address,
) -> (u64, u64) {
    let eval_hash = BytesN::from_array(env, &[1u8; 32]);
    let limitations = Vec::from_array(env, [String::from_str(env, "Limited")]);

    let eval_id = client.conduct_pt_evaluation(
        patient,
        therapist,
        &1000u64,
        &String::from_str(env, "Injury"),
        &String::from_str(env, "Pain"),
        &limitations,
        &String::from_str(env, "Active"),
        &eval_hash,
    );

    let stg_goal = RehabGoal {
        goal_id: 1,
        goal_type: Symbol::new(env, "stg"),
        goal_description: String::from_str(env, "Goal"),
        target_date: 2000u64,
        measurement_method: String::from_str(env, "Method"),
        achieved: false,
    };

    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(env, "exercise"),
        description: String::from_str(env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    };

    let plan_id = client.create_rehab_treatment_plan(
        &eval_id,
        therapist,
        &Vec::from_array(env, [stg_goal.clone()]),
        &Vec::from_array(env, [stg_goal]),
        &Vec::from_array(env, [intervention]),
        &String::from_str(env, "3x/week"),
        &8u32,
        &Symbol::new(env, "good"),
    );

    (eval_id, plan_id)
}

#[test]
fn test_set_rehabilitation_goal_success() {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &2000u64,
    );

    assert_eq!(goal_id, 1);
    let goal = client.get_measurable_goal(&goal_id);
    assert_eq!(goal.plan_id, plan_id);
    assert_eq!(goal.target_value, 120);
    assert!(!goal.achieved);
}

#[test]
fn test_record_progress_below_target() {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &2000u64,
    );

    client.record_progress(&plan_id, &goal_id, &80u32, &1500u64);

    let progress = client.get_goal_progress(&plan_id, &goal_id);
    assert_eq!(progress.len(), 1);
    assert_eq!(progress.get(0).unwrap().current_value, 80);

    // Not yet achieved
    let goal = client.get_measurable_goal(&goal_id);
    assert!(!goal.achieved);
}

#[test]
fn test_record_progress_achieves_goal() {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "pain_scale"),
        &3u32,
        &2000u64,
    );

    // Pain scale — lower is better but we track as "has reached target"
    client.record_progress(&plan_id, &goal_id, &5u32, &1400u64);
    client.record_progress(&plan_id, &goal_id, &3u32, &1500u64);

    let goal = client.get_measurable_goal(&goal_id);
    assert!(goal.achieved);

    let progress = client.get_goal_progress(&plan_id, &goal_id);
    assert_eq!(progress.len(), 2);
}

#[test]
fn test_goal_progress_time_series_queryable() {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "fim"),
        &100u32,
        &3000u64,
    );

    for i in 0..5u32 {
        client.record_progress(&plan_id, &goal_id, &(50 + i * 10), &(1000 + i as u64 * 100));
    }

    let progress = client.get_goal_progress(&plan_id, &goal_id);
    assert_eq!(progress.len(), 5);
    assert_eq!(progress.get(0).unwrap().current_value, 50);
    assert_eq!(progress.get(4).unwrap().current_value, 90);
}

#[test]
fn test_goal_progress_wrong_plan_returns_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &2000u64,
    );

    // Query with wrong plan_id → empty
    let progress = client.get_goal_progress(&(plan_id + 99), &goal_id);
    assert_eq!(progress.len(), 0);
}

// ── Treatment plan versioning tests (#560) ────────────────────────────────────

#[test]
fn test_initial_version_created_on_plan_creation() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    // Verify version history exists with version 1
    let history = client.get_treatment_plan_history(&plan_id);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().version, 1);
}

#[test]
fn test_update_rehab_treatment_plan_by_authorized_therapist() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let goals = Vec::new(&env);
    let interventions = Vec::new(&env);
    let ipfs_hash = String::from_str(&env, "QmNewHash123");

    // Update the plan with new fields
    let new_version = client.update_rehab_treatment_plan(
        &plan_id,
        &therapist,
        &goals,
        &goals,
        &interventions,
        &String::from_str(&env, "bi-weekly"),
        &12u32,
        &Symbol::new(&env, "good"),
        &ipfs_hash,
    );
    assert_eq!(new_version, 2);

    // Verify history now has 2 entries
    let history = client.get_treatment_plan_history(&plan_id);
    assert_eq!(history.len(), 2);
    assert_eq!(history.get(0).unwrap().version, 1);
    assert_eq!(history.get(1).unwrap().version, 2);
    assert_eq!(history.get(1).unwrap().ipfs_hash, ipfs_hash);
    assert_eq!(history.get(1).unwrap().updated_by, therapist);

    // Verify plan was actually updated
    let plan = client.get_treatment_plan(&plan_id);
    assert_eq!(plan.frequency, String::from_str(&env, "bi-weekly"));
    assert_eq!(plan.duration_weeks, 12);
}

#[test]
fn test_update_rehab_treatment_plan_unauthorized_rejected() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let unauthorized = Address::generate(&env);
    let goals = Vec::new(&env);
    let interventions = Vec::new(&env);

    let result = client.try_update_rehab_treatment_plan(
        &plan_id,
        &unauthorized,
        &goals,
        &goals,
        &interventions,
        &String::from_str(&env, "daily"),
        &6u32,
        &Symbol::new(&env, "fair"),
        &String::from_str(&env, "QmHash"),
    );
    assert!(result.is_err());
    let inner = result.unwrap_err();
    match inner {
        Ok(Error::Unauthorized) => {},
        Ok(_) => {},
        Err(_) => {},
    }
}

#[test]
fn test_get_treatment_plan_history_returns_full_history() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let goals = Vec::new(&env);
    let interventions = Vec::new(&env);

    // Make two updates
    client.update_rehab_treatment_plan(
        &plan_id,
        &therapist,
        &goals,
        &goals,
        &interventions,
        &String::from_str(&env, "weekly"),
        &8u32,
        &Symbol::new(&env, "good"),
        &String::from_str(&env, "QmHash1"),
    );

    client.update_rehab_treatment_plan(
        &plan_id,
        &therapist,
        &goals,
        &goals,
        &interventions,
        &String::from_str(&env, "bi-weekly"),
        &10u32,
        &Symbol::new(&env, "excellent"),
        &String::from_str(&env, "QmHash2"),
    );

    // Full history: 3 entries (initial + 2 updates)
    let history = client.get_treatment_plan_history(&plan_id);
    assert_eq!(history.len(), 3);
    assert_eq!(history.get(0).unwrap().version, 1);
    assert_eq!(history.get(1).unwrap().version, 2);
    assert_eq!(history.get(2).unwrap().version, 3);
    assert_eq!(history.get(2).unwrap().ipfs_hash, String::from_str(&env, "QmHash2"));
}
