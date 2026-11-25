#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec};

// ── Mock provider registry ───────────────────────────────────────────────────
//
// The rehabilitation-services contract verifies therapists against a provider
// registry. For unit tests we register a lightweight mock contract that exposes
// the `is_provider_registered` entry point used by
// `shared::actor_verification::is_provider_registered`.

mod mock_registry {
    use soroban_sdk::{contract, contractimpl, Address, Env};

    #[contract]
    pub struct MockProviderRegistry;

    #[contractimpl]
    impl MockProviderRegistry {
        pub fn register_provider(env: Env, provider: Address) {
            env.storage()
                .persistent()
                .set(&provider, &true);
        }

        pub fn is_provider_registered(env: Env, provider: Address) -> bool {
            env.storage()
                .persistent()
                .get(&provider)
                .unwrap_or(false)
        }
    }
}

use mock_registry::{MockProviderRegistry, MockProviderRegistryClient};

/// Register the rehabilitation contract with its constructor, wiring in a mock
/// provider registry and registering `therapist` as an active provider.
fn setup_with_registry<'a>(
    env: &'a Env,
    therapist: &Address,
) -> (RehabilitationServicesContractClient<'a>, Address) {
    let admin = Address::generate(env);
    let registry_id = env.register(MockProviderRegistry, ());
    let registry = MockProviderRegistryClient::new(env, &registry_id);
    registry.register_provider(therapist);

    let contract_id = env.register(
        RehabilitationServicesContract,
        (&admin, &registry_id),
    );
    let client = RehabilitationServicesContractClient::new(env, &contract_id);
    (client, registry_id)
}

fn create_test_env() -> (Env, Address, Address) {
    let env = Env::default();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    (env, patient, therapist)
}

/// Convenience: set up env + registered therapist + client.
fn setup() -> (
    Env,
    Address,
    Address,
    RehabilitationServicesContractClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let therapist = Address::generate(&env);
    let (client, _) = setup_with_registry(&env, &therapist);
    // SAFETY: the client borrows `env`, which outlives the returned tuple in
    // every test because it is moved together with the client.
    let client: RehabilitationServicesContractClient<'static> =
        unsafe { core::mem::transmute(client) };
    (env, patient, therapist, client)
}

#[test]
fn test_conduct_pt_evaluation() {
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id =
        client.set_rehabilitation_goal(&plan_id, &Symbol::new(&env, "pain_scale"), &3u32, &2000u64);

    // Pain scale — lower is better. Baseline 5 is above target 3, so not achieved.
    client.record_progress(&plan_id, &goal_id, &5u32, &1400u64);
    assert!(!client.get_measurable_goal(&therapist, &goal_id).achieved);

    // Reaching target 3 achieves the goal.
    client.record_progress(&plan_id, &goal_id, &3u32, &1500u64);

    let goal = client.get_measurable_goal(&therapist, &goal_id);
    assert!(goal.achieved);

    let progress = client.get_goal_progress(&therapist, &plan_id, &goal_id);
    assert_eq!(progress.len(), 2);
}

#[test]
fn test_goal_progress_time_series_queryable() {
    let (env, patient, therapist, client) = setup();

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
    let (env, patient, therapist, client) = setup();

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

// ── Lower-is-better goal direction (#1165) ───────────────────────────────────

#[test]
fn test_pain_goal_baseline_not_achieved() {
    // A pain goal of "≤ 2/10" must NOT be achieved by the first (worst) reading.
    let (env, patient, therapist, client) = setup();

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id =
        client.set_rehabilitation_goal(&plan_id, &Symbol::new(&env, "pain_scale"), &2u32, &2000u64);

    // Baseline 8/10 — worse than target, so not achieved.
    client.record_progress(&plan_id, &goal_id, &8u32, &1400u64);
    let goal = client.get_measurable_goal(&therapist, &goal_id);
    assert!(!goal.achieved, "baseline pain reading must not achieve goal");
}

#[test]
fn test_pain_goal_achieved_when_improved() {
    // A pain goal of "≤ 2/10" is achieved once the reading drops to 1/10.
    let (env, patient, therapist, client) = setup();

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id =
        client.set_rehabilitation_goal(&plan_id, &Symbol::new(&env, "pain_scale"), &2u32, &2000u64);

    client.record_progress(&plan_id, &goal_id, &8u32, &1400u64);
    assert!(!client.get_measurable_goal(&therapist, &goal_id).achieved);

    client.record_progress(&plan_id, &goal_id, &1u32, &1500u64);
    let goal = client.get_measurable_goal(&therapist, &goal_id);
    assert!(goal.achieved, "improved pain reading must achieve goal");
}

#[test]
fn test_higher_is_better_goal_still_works() {
    // Sanity: a higher-is-better goal (ROM) is achieved by reaching target.
    let (env, patient, therapist, client) = setup();

    let (_, plan_id) = create_plan(&env, &client, &patient, &therapist);
    let goal_id = client.set_rehabilitation_goal(
        &plan_id,
        &Symbol::new(&env, "range_of_motion"),
        &120u32,
        &2000u64,
