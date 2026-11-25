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

    let evaluation = client.get_evaluation(&therapist, &eval_id);
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

    let rom_assessments = client.get_rom_assessments(&therapist, &eval_id);
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

    let strength_assessments = client.get_strength_assessments(&therapist, &eval_id);
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

    let balance_assessments = client.get_balance_mobility_assessments(&therapist, &eval_id);
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

    let plan = client.get_treatment_plan(&therapist, &plan_id);
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

    let sessions = client.get_therapy_sessions(&therapist, &plan_id);
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

    let pain_measurements = client.get_pain_measurements(&therapist, &plan_id);
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

    let outcomes = client.get_functional_outcomes(&therapist, &plan_id);
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

    let notes = client.get_progress_notes(&therapist, &plan_id);
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

    let discharge = client.get_discharge_record(&therapist, &plan_id);
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
    let evaluation = client.get_evaluation(&therapist, &eval_id);
    assert_eq!(evaluation.patient_id, patient);

    let rom_assessments = client.get_rom_assessments(&therapist, &eval_id);
    assert_eq!(rom_assessments.len(), 1);

    let sessions = client.get_therapy_sessions(&therapist, &plan_id);
    assert_eq!(sessions.len(), 1);

    let pain_measurements = client.get_pain_measurements(&therapist, &plan_id);
    assert_eq!(pain_measurements.len(), 1);

    let outcomes = client.get_functional_outcomes(&therapist, &plan_id);
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
    let goal = client.get_measurable_goal(&therapist, &goal_id);
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

    let progress = client.get_goal_progress(&therapist, &plan_id, &goal_id);
    assert_eq!(progress.len(), 1);
    assert_eq!(progress.get(0).unwrap().current_value, 80);

    // Not yet achieved
    let goal = client.get_measurable_goal(&therapist, &goal_id);
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
    let goal_id =
        client.set_rehabilitation_goal(&plan_id, &Symbol::new(&env, "pain_scale"), &3u32, &2000u64);

    // Pain scale — lower is better but we track as "has reached target"
    client.record_progress(&plan_id, &goal_id, &5u32, &1400u64);
    client.record_progress(&plan_id, &goal_id, &3u32, &1500u64);

    let goal = client.get_measurable_goal(&therapist, &goal_id);
    assert!(goal.achieved);

    let progress = client.get_goal_progress(&therapist, &plan_id, &goal_id);
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
    let goal_id =
        client.set_rehabilitation_goal(&plan_id, &Symbol::new(&env, "fim"), &100u32, &3000u64);

    for i in 0..5u32 {
        client.record_progress(&plan_id, &goal_id, &(50 + i * 10), &(1000 + i as u64 * 100));
    }

    let progress = client.get_goal_progress(&therapist, &plan_id, &goal_id);
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

    // A second, unrelated plan that the same therapist is still bound to (so the
    // authorization check passes), but the goal doesn't belong to it → empty.
    let (_, other_plan_id) = create_plan(&env, &client, &patient, &therapist);
    let progress = client.get_goal_progress(&therapist, &other_plan_id, &goal_id);
    assert_eq!(progress.len(), 0);
}

// ── Paginated therapy sessions (#564) ────────────────────────────────────────

fn add_sessions(
    env: &Env,
    client: &RehabilitationServicesContractClient,
    plan_id: u64,
    count: u32,
) {
    let intervention = TherapyIntervention {
        intervention_type: Symbol::new(env, "exercise"),
        description: String::from_str(env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    };
    for i in 0..count {
        client.document_therapy_session(
            &plan_id,
            &(1000u64 + i as u64),
            &Vec::from_array(env, [intervention.clone()]),
            &30u32,
            &String::from_str(env, "Good"),
            &None::<String>,
        );
    }
}

#[test]
fn test_get_therapy_sessions_paged_first_page() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    add_sessions(&env, &client, plan_id, 7);

    let page = client.get_therapy_sessions_paged(&therapist, &plan_id, &0u32, &3u32);
    assert_eq!(page.items.len(), 3);
    assert!(page.has_more);
}

#[test]
fn test_get_therapy_sessions_paged_last_page() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    add_sessions(&env, &client, plan_id, 7);

    let page = client.get_therapy_sessions_paged(&therapist, &plan_id, &2u32, &3u32);
    assert_eq!(page.items.len(), 1); // 7 items, pages of 3: [0..3), [3..6), [6..7)
    assert!(!page.has_more);
}

#[test]
fn test_get_therapy_sessions_paged_empty() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let page = client.get_therapy_sessions_paged(&therapist, &plan_id, &0u32, &10u32);
    assert_eq!(page.items.len(), 0);
    assert!(!page.has_more);
}

#[test]
fn test_get_therapy_sessions_paged_beyond_range() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    add_sessions(&env, &client, plan_id, 3);

    let page = client.get_therapy_sessions_paged(&therapist, &plan_id, &99u32, &10u32);
    assert_eq!(page.items.len(), 0);
    assert!(!page.has_more);
}

#[test]
fn test_get_therapy_sessions_paged_page_size_clamped() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    add_sessions(&env, &client, plan_id, 5);

    // page_size > MAX_PAGE_SIZE (50) should be clamped — returns at most 50
    let page = client.get_therapy_sessions_paged(&therapist, &plan_id, &0u32, &200u32);
    assert_eq!(page.items.len(), 5); // only 5 sessions, all fit in one page
    assert!(!page.has_more);
}

#[test]
fn test_get_progress_notes_paged() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    for i in 0u64..4 {
        client.document_progress_note(
            &plan_id,
            &(2000u64 + i),
            &String::from_str(&env, "Feels better"),
            &Vec::from_array(&env, [String::from_str(&env, "Improved ROM")]),
            &String::from_str(&env, "Progressing"),
            &Vec::new(&env),
        );
    }

    let page = client.get_progress_notes_paged(&therapist, &plan_id, &0u32, &3u32);
    assert_eq!(page.items.len(), 3);
    assert!(page.has_more);

    let page2 = client.get_progress_notes_paged(&therapist, &plan_id, &1u32, &3u32);
    assert_eq!(page2.items.len(), 1);
    assert!(!page2.has_more);
}

#[test]
fn test_plan_version_is_per_plan_not_global() {
    // Goals created on plan_b must not inflate plan_version on plan_a.
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    let (_, plan_a) = create_plan(&env, &client, &patient, &therapist);
    let (_, plan_b) = create_plan(&env, &client, &patient, &therapist);

    // Add several goals to plan_b first.
    for _ in 0..5 {
        client.set_rehabilitation_goal(&plan_b, &Symbol::new(&env, "strength"), &100u32, &9000u64);
    }

    // First goal on plan_a should have plan_version == 0 regardless of plan_b's goals.
    let g1 = client.set_rehabilitation_goal(
        &plan_a,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &9000u64,
    );
    assert_eq!(client.get_measurable_goal(&therapist, &g1).plan_version, 0);

    // Second goal on plan_a should have plan_version == 1.
    let g2 = client.set_rehabilitation_goal(
        &plan_a,
        &Symbol::new(&env, "range_of_motion"),
        &130u32,
        &9001u64,
    );
    assert_eq!(client.get_measurable_goal(&therapist, &g2).plan_version, 1);
}

#[test]
fn test_persistent_storage_bounded_instance_size() {
    // Regression test for #642: verifies that creating many sessions and progress
    // notes across multiple plans does not balloon instance storage (which would be
    // loaded on every invocation). All data must live in persistent storage keyed
    // by plan/goal id, so reads on an unrelated plan are not burdened by the
    // accumulated data of other plans.
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);

    // Create 3 independent plans.
    let (_, plan_a) = create_plan(&env, &client, &patient, &therapist);
    let (_, plan_b) = create_plan(&env, &client, &patient, &therapist);
    let (_, plan_c) = create_plan(&env, &client, &patient, &therapist);

    // Write 5 therapy sessions and 5 progress notes to each plan.
    for i in 0u32..5 {
        let mut interventions = Vec::new(&env);
        interventions.push_back(TherapyIntervention {
            intervention_type: Symbol::new(&env, "exercise"),
            description: String::from_str(&env, "Stretching"),
            sets: Some(3),
            reps: Some(10),
            duration: None,
            resistance: None,
        });
        for plan in [plan_a, plan_b, plan_c] {
            client.document_therapy_session(
                &plan,
                &(1000u64 + i as u64),
                &interventions,
                &30u32,
                &String::from_str(&env, "Good"),
                &None,
            );
            client.document_progress_note(
                &plan,
                &(1000u64 + i as u64),
                &String::from_str(&env, "Patient reports improvement"),
                &Vec::new(&env),
                &String::from_str(&env, "Progressing"),
                &Vec::new(&env),
            );
        }
    }

    // Reading plan_a must succeed without touching sessions from plan_b or plan_c.
    let sessions_a = client.get_therapy_sessions(&therapist, &plan_a);
    assert_eq!(sessions_a.len(), 5);

    // plan_b and plan_c are independent: their session counts are also 5, not 15.
    let sessions_b = client.get_therapy_sessions(&therapist, &plan_b);
    assert_eq!(sessions_b.len(), 5);

    let sessions_c = client.get_therapy_sessions(&therapist, &plan_c);
    assert_eq!(sessions_c.len(), 5);

    // Goals across plans remain isolated.
    let g_a = client.set_rehabilitation_goal(&plan_a, &Symbol::new(&env, "strength"), &100u32, &9000u64);
    let g_b = client.set_rehabilitation_goal(&plan_b, &Symbol::new(&env, "strength"), &100u32, &9000u64);
    assert_ne!(g_a, g_b);
    assert_eq!(client.get_measurable_goal(&therapist, &g_a).plan_id, plan_a);
    assert_eq!(client.get_measurable_goal(&therapist, &g_b).plan_id, plan_b);
}

// ── Getter access control (#748) ─────────────────────────────────────────────
//
// Rehabilitation-services has no separate care-team roster: each evaluation/plan
// only records a single treating therapist and, via the evaluation, a single
// patient. So "bound to the record" collapses to exactly those two parties —
// there is no third "care-team member" case to exercise here, unlike care-plan's
// multi-provider `is_bound_to_plan`. Each shape of getter below is checked for:
// the treating therapist can read, the patient can read, and an unrelated
// address is rejected.

#[test]
fn test_get_evaluation_access_control() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let stranger = Address::generate(&env);
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

    assert_eq!(
        client.get_evaluation(&therapist, &eval_id).patient_id,
        patient
    );
    assert_eq!(client.get_evaluation(&patient, &eval_id).patient_id, patient);
    assert!(client.try_get_evaluation(&stranger, &eval_id).is_err());
}

#[test]
fn test_get_rom_assessments_access_control() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let stranger = Address::generate(&env);
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

    assert_eq!(
        client.get_rom_assessments(&therapist, &eval_id).len(),
        1
    );
    assert_eq!(client.get_rom_assessments(&patient, &eval_id).len(), 1);
    assert!(client
        .try_get_rom_assessments(&stranger, &eval_id)
        .is_err());
}

#[test]
fn test_get_treatment_plan_access_control() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let stranger = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    assert_eq!(client.get_treatment_plan(&therapist, &plan_id).plan_id, plan_id);
    assert_eq!(client.get_treatment_plan(&patient, &plan_id).plan_id, plan_id);
    assert!(client.try_get_treatment_plan(&stranger, &plan_id).is_err());
}

#[test]
fn test_get_therapy_sessions_access_control() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let stranger = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    add_sessions(&env, &client, plan_id, 2);

    assert_eq!(client.get_therapy_sessions(&therapist, &plan_id).len(), 2);
    assert_eq!(client.get_therapy_sessions(&patient, &plan_id).len(), 2);
    assert!(client
        .try_get_therapy_sessions(&stranger, &plan_id)
        .is_err());
}

#[test]
fn test_get_discharge_record_access_control() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let stranger = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let final_outcomes_hash = BytesN::from_array(&env, &[3u8; 32]);
    let hep_hash = BytesN::from_array(&env, &[4u8; 32]);
    client.discharge_from_therapy(
        &plan_id,
        &5000u64,
        &Symbol::new(&env, "goals_met"),
        &Vec::new(&env),
        &final_outcomes_hash,
        &hep_hash,
    );

    assert_eq!(
        client.get_discharge_record(&therapist, &plan_id).discharge_date,
        5000u64
    );
    assert_eq!(
        client.get_discharge_record(&patient, &plan_id).discharge_date,
        5000u64
    );
    assert!(client
        .try_get_discharge_record(&stranger, &plan_id)
        .is_err());
}

#[test]
fn test_get_treatment_plan_history_access_control() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let stranger = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    assert_eq!(
        client.get_treatment_plan_history(&therapist, &plan_id).len(),
        1
    );
    assert_eq!(
        client.get_treatment_plan_history(&patient, &plan_id).len(),
        1
    );
    assert!(client
        .try_get_treatment_plan_history(&stranger, &plan_id)
        .is_err());
}

#[test]
fn test_get_therapy_sessions_paged_access_control() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let stranger = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    add_sessions(&env, &client, plan_id, 2);

    assert_eq!(
        client
            .get_therapy_sessions_paged(&therapist, &plan_id, &0u32, &10u32)
            .items
            .len(),
        2
    );
    assert_eq!(
        client
            .get_therapy_sessions_paged(&patient, &plan_id, &0u32, &10u32)
            .items
            .len(),
        2
    );
    assert!(client
        .try_get_therapy_sessions_paged(&stranger, &plan_id, &0u32, &10u32)
        .is_err());
}

#[test]
fn test_get_measurable_goal_access_control() {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let stranger = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &2000u64,
    );

    assert_eq!(
        client.get_measurable_goal(&therapist, &goal_id).goal_id,
        goal_id
    );
    assert_eq!(
        client.get_measurable_goal(&patient, &goal_id).goal_id,
        goal_id
    );
    assert!(client
        .try_get_measurable_goal(&stranger, &goal_id)
        .is_err());
}

#[test]
fn test_get_goal_progress_access_control() {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let stranger = Address::generate(&env);
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

    assert_eq!(
        client
            .get_goal_progress(&therapist, &plan_id, &goal_id)
            .len(),
        1
    );
    assert_eq!(
        client.get_goal_progress(&patient, &plan_id, &goal_id).len(),
        1
    );
    assert!(client
        .try_get_goal_progress(&stranger, &plan_id, &goal_id)
        .is_err());
}

// ── Discharge closes the plan (#1167) ────────────────────────────────────────

fn sample_intervention(env: &Env) -> TherapyIntervention {
    TherapyIntervention {
        intervention_type: Symbol::new(env, "exercise"),
        description: String::from_str(env, "Exercise"),
        sets: Some(3),
        reps: Some(10),
        duration: None,
        resistance: None,
    }
}

fn discharge_plan(
    env: &Env,
    client: &RehabilitationServicesContractClient,
    plan_id: u64,
    goals_met: Vec<u64>,
) {
    let final_outcomes_hash = BytesN::from_array(env, &[3u8; 32]);
    let hep_hash = BytesN::from_array(env, &[4u8; 32]);
    client.discharge_from_therapy(
        &plan_id,
        &5000u64,
        &Symbol::new(env, "goals_met"),
        &goals_met,
        &final_outcomes_hash,
        &hep_hash,
    );
}

#[test]
fn test_discharge_sets_plan_status() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    assert_eq!(
        client.get_treatment_plan(&therapist, &plan_id).status,
        RehabPlanStatus::Active
    );

    discharge_plan(&env, &client, plan_id, Vec::new(&env));

    assert_eq!(
        client.get_treatment_plan(&therapist, &plan_id).status,
        RehabPlanStatus::Discharged
    );
}

#[test]
fn test_discharge_rejects_clinical_writes() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let intervention = sample_intervention(&env);

    discharge_plan(&env, &client, plan_id, Vec::new(&env));

    // document_therapy_session
    assert!(client
        .try_document_therapy_session(
            &plan_id,
            &6000u64,
            &Vec::from_array(&env, [intervention.clone()]),
            &30u32,
            &String::from_str(&env, "Good"),
            &None,
        )
        .is_err());

    // track_pain_level
    assert!(client
        .try_track_pain_level(
            &plan_id,
            &6000u64,
            &Symbol::new(&env, "vas"),
            &4u32,
            &String::from_str(&env, "Knee"),
            &Vec::new(&env),
        )
        .is_err());

    // measure_functional_outcome
    assert!(client
        .try_measure_functional_outcome(
            &plan_id,
            &6000u64,
            &Symbol::new(&env, "lefs"),
            &60u32,
            &true,
        )
        .is_err());

    // document_progress_note
    assert!(client
        .try_document_progress_note(
            &plan_id,
            &6000u64,
            &String::from_str(&env, "Feels fine"),
            &Vec::new(&env),
            &String::from_str(&env, "Stable"),
            &Vec::new(&env),
        )
        .is_err());

    // set_rehabilitation_goal
    assert!(client
        .try_set_rehabilitation_goal(
            &plan_id,
            &Symbol::new(&env, "strength"),
            &100u32,
            &9000u64,
        )
        .is_err());

    // request_therapy_authorization
    let justification_hash = BytesN::from_array(&env, &[2u8; 32]);
    assert!(client
        .try_request_therapy_authorization(&plan_id, &6u32, &justification_hash)
        .is_err());

    // update_rehab_treatment_plan
    assert!(client
        .try_update_rehab_treatment_plan(
            &plan_id,
            &therapist,
            &Vec::from_array(&env, [intervention]),
            &String::from_str(&env, "2x/week"),
            &4u32,
            &Symbol::new(&env, "fair"),
        )
        .is_err());

    // Nothing was recorded.
    assert_eq!(client.get_therapy_sessions(&therapist, &plan_id).len(), 0);
    assert_eq!(client.get_pain_measurements(&therapist, &plan_id).len(), 0);
    assert_eq!(client.get_functional_outcomes(&therapist, &plan_id).len(), 0);
    assert_eq!(client.get_progress_notes(&therapist, &plan_id).len(), 0);
}

#[test]
fn test_record_progress_rejected_after_discharge() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &2000u64,
    );

    discharge_plan(&env, &client, plan_id, Vec::new(&env));

    assert!(client
        .try_record_progress(&plan_id, &goal_id, &100u32, &6000u64)
        .is_err());
}

#[test]
fn test_discharge_is_one_time() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    discharge_plan(&env, &client, plan_id, Vec::new(&env));

    let final_outcomes_hash = BytesN::from_array(&env, &[5u8; 32]);
    let hep_hash = BytesN::from_array(&env, &[6u8; 32]);
    assert!(client
        .try_discharge_from_therapy(
            &plan_id,
            &7000u64,
            &Symbol::new(&env, "goals_met"),
            &Vec::new(&env),
            &final_outcomes_hash,
            &hep_hash,
        )
        .is_err());

    // Original record is preserved.
    let discharge = client.get_discharge_record(&therapist, &plan_id);
    assert_eq!(discharge.discharge_date, 5000u64);
}

#[test]
fn test_discharge_rejects_nonexistent_goal() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    assert!(client
        .try_discharge_from_therapy(
            &plan_id,
            &5000u64,
            &Symbol::new(&env, "goals_met"),
            &Vec::from_array(&env, [999u64]),
            &BytesN::from_array(&env, &[3u8; 32]),
            &BytesN::from_array(&env, &[4u8; 32]),
        )
        .is_err());

    assert_eq!(
        client.get_treatment_plan(&therapist, &plan_id).status,
        RehabPlanStatus::Active
    );
}

#[test]
fn test_discharge_rejects_goal_from_other_plan() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let (_, other_plan_id) = create_plan(&env, &client, &patient, &therapist);

    let other_goal = client.set_rehabilitation_goal(
        &other_plan_id,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &2000u64,
    );

    assert!(client
        .try_discharge_from_therapy(
            &plan_id,
            &5000u64,
            &Symbol::new(&env, "goals_met"),
            &Vec::from_array(&env, [other_goal]),
            &BytesN::from_array(&env, &[3u8; 32]),
            &BytesN::from_array(&env, &[4u8; 32]),
        )
        .is_err());
}

#[test]
fn test_discharge_rejects_unachieved_goal() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
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

    assert!(client
        .try_discharge_from_therapy(
            &plan_id,
            &5000u64,
            &Symbol::new(&env, "goals_met"),
            &Vec::from_array(&env, [goal_id]),
            &BytesN::from_array(&env, &[3u8; 32]),
            &BytesN::from_array(&env, &[4u8; 32]),
        )
        .is_err());
}

#[test]
fn test_discharge_accepts_achieved_goal() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &2000u64,
    );
    client.record_progress(&plan_id, &goal_id, &120u32, &1500u64);

    discharge_plan(&env, &client, plan_id, Vec::from_array(&env, [goal_id]));

    assert_eq!(
        client.get_treatment_plan(&therapist, &plan_id).status,
        RehabPlanStatus::Discharged
    );
}

#[test]
fn test_discharge_rejects_date_before_ledger() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    // Ledger timestamp is 0 in tests; a discharge date in the past is invalid.
    assert!(client
        .try_discharge_from_therapy(
            &plan_id,
            &0u64,
            &Symbol::new(&env, "goals_met"),
            &Vec::new(&env),
            &BytesN::from_array(&env, &[3u8; 32]),
            &BytesN::from_array(&env, &[4u8; 32]),
        )
        .is_err());
}

#[test]
fn test_session_rejects_date_before_ledger() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let intervention = sample_intervention(&env);

    assert!(client
        .try_document_therapy_session(
            &plan_id,
            &0u64,
            &Vec::from_array(&env, [intervention]),
            &30u32,
            &String::from_str(&env, "Good"),
            &None,
        )
        .is_err());
}

// ── Therapy authorization lifecycle (#1167) ──────────────────────────────────

#[test]
fn test_therapy_authorization_lifecycle_approve() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let payer = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let justification_hash = BytesN::from_array(&env, &[2u8; 32]);
    let auth_id = client.request_therapy_authorization(&plan_id, &12u32, &justification_hash);

    let auth = client.get_therapy_authorization(&therapist, &auth_id);
    assert_eq!(auth.auth_id, auth_id);
    assert_eq!(auth.plan_id, plan_id);
    assert_eq!(auth.requested_visits, 12);
    assert_eq!(auth.status, TherapyAuthorizationStatus::Pending);

    client.decide_therapy_authorization(
        &payer,
        &auth_id,
        &10u32,
        &TherapyAuthorizationStatus::Approved,
    );

    let auth = client.get_therapy_authorization(&therapist, &auth_id);
    assert_eq!(auth.status, TherapyAuthorizationStatus::Approved);
    assert_eq!(auth.approved_visits, 10);
}

#[test]
fn test_therapy_authorization_lifecycle_deny() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let payer = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let justification_hash = BytesN::from_array(&env, &[2u8; 32]);
    let auth_id = client.request_therapy_authorization(&plan_id, &12u32, &justification_hash);

    client.decide_therapy_authorization(
        &payer,
        &auth_id,
        &0u32,
        &TherapyAuthorizationStatus::Denied,
    );

    let auth = client.get_therapy_authorization(&therapist, &auth_id);
    assert_eq!(auth.status, TherapyAuthorizationStatus::Denied);
}

#[test]
fn test_therapy_authorization_decision_is_one_time() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let payer = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let justification_hash = BytesN::from_array(&env, &[2u8; 32]);
    let auth_id = client.request_therapy_authorization(&plan_id, &12u32, &justification_hash);

    client.decide_therapy_authorization(
        &payer,
        &auth_id,
        &10u32,
        &TherapyAuthorizationStatus::Approved,
    );

    assert!(client
        .try_decide_therapy_authorization(
            &payer,
            &auth_id,
            &0u32,
            &TherapyAuthorizationStatus::Denied,
        )
        .is_err());

    let auth = client.get_therapy_authorization(&therapist, &auth_id);
    assert_eq!(auth.status, TherapyAuthorizationStatus::Approved);
}

#[test]
fn test_therapy_authorization_rejects_pending_decision() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let payer = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let justification_hash = BytesN::from_array(&env, &[2u8; 32]);
    let auth_id = client.request_therapy_authorization(&plan_id, &12u32, &justification_hash);

    assert!(client
        .try_decide_therapy_authorization(
            &payer,
            &auth_id,
            &0u32,
            &TherapyAuthorizationStatus::Pending,
        )
        .is_err());
}

#[test]
fn test_therapy_authorization_getter_access_control() {
    let (env, patient, therapist) = create_test_env();
    env.mock_all_auths();
    let stranger = Address::generate(&env);
    let contract_id = env.register(RehabilitationServicesContract, ());
    let client = RehabilitationServicesContractClient::new(&env, &contract_id);
    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);

    let justification_hash = BytesN::from_array(&env, &[2u8; 32]);
    let auth_id = client.request_therapy_authorization(&plan_id, &12u32, &justification_hash);

    assert_eq!(
        client.get_therapy_authorization(&therapist, &auth_id).auth_id,
        auth_id
    );
    assert_eq!(
        client.get_therapy_authorization(&patient, &auth_id).auth_id,
        auth_id
    );
    assert!(client
        .try_get_therapy_authorization(&stranger, &auth_id)
        .is_err());
}
