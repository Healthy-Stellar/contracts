#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, BytesN, Env, String, Symbol, Vec};

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    (env, provider, patient)
}

fn create_plan(env: &Env, patient: &Address, provider: &Address) -> u64 {
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(env, &contract_id);

    let mut conditions = Vec::new(env);
    conditions.push_back(String::from_str(env, "Type 2 Diabetes"));

    let mut goals = Vec::new(env);
    goals.push_back(String::from_str(env, "Reduce HbA1c to <7%"));

    client
        .create_care_plan(
            patient,
            provider,
            &Symbol::new(env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
}

fn register_and_create_plan(env: &Env) -> (Address, CarePlanContractClient, u64) {
    let provider = Address::generate(env);
    let patient = Address::generate(env);

    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(env, &contract_id);

    let mut conditions = Vec::new(env);
    conditions.push_back(String::from_str(env, "Hypertension"));

    let mut goals = Vec::new(env);
    goals.push_back(String::from_str(env, "Lower BP to <130/80"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    (provider, client, plan_id)
}

// -----------------------------------------------------------------------
// create_care_plan
// -----------------------------------------------------------------------

#[test]
fn test_create_care_plan_success() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "COPD"));

    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Improve oxygen saturation"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &2_000_000u64,
            &90u32,
        )
        ;

    assert_eq!(plan_id, 1);
}

#[test]
fn test_create_care_plan_increments_ids() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal 1"));

    let id1 = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let id2 = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "preventive"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_create_care_plan_next_review_date_calculated() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal 1"));

    client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let summary = client.get_care_plan_summary(&1, &provider);
    // 1_000_000 + 30 * 86_400 = 3_592_000
    assert_eq!(summary.next_review_date, 1_000_000 + 30 * 86_400);
}

// -----------------------------------------------------------------------
// add_care_goal
// -----------------------------------------------------------------------

#[test]
fn test_add_care_goal_success() {
    let (env, provider, client, plan_id) = {
        let (env, provider, patient) = setup();
        let contract_id = env.register(CarePlanContract, ());
        let client = CarePlanContractClient::new(&env, &contract_id);

        let mut conditions = Vec::new(&env);
        conditions.push_back(String::from_str(&env, "Diabetes"));
        let mut goals = Vec::new(&env);
        goals.push_back(String::from_str(&env, "Initial goal"));

        let plan_id = client
            .create_care_plan(
                &patient,
                &provider,
                &Symbol::new(&env, "chronic_disease"),
                &conditions,
                &goals,
                &1_000_000u64,
                &30u32,
            )
            ;

        (env, provider, client, plan_id)
    };

    let goal_id = client
        .add_care_goal(
            &plan_id,
            &provider,
            &String::from_str(&env, "Reduce HbA1c below 7%"),
            &Some(String::from_str(&env, "6.9")),
            &2_000_000u64,
            &Symbol::new(&env, "high"),
        )
        ;

    assert_eq!(goal_id, 1);
}

#[test]
fn test_add_care_goal_plan_not_found() {
    let (env, provider, _) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let result = client.try_add_care_goal(
        &999,
        &provider,
        &String::from_str(&env, "Lose weight"),
        &None,
        &2_000_000u64,
        &Symbol::new(&env, "medium"),
    );

    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// add_intervention
// -----------------------------------------------------------------------

#[test]
fn test_add_intervention_success() {
    let (env, provider, client, plan_id) = {
        let (env, provider, patient) = setup();
        let contract_id = env.register(CarePlanContract, ());
        let client = CarePlanContractClient::new(&env, &contract_id);

        let mut conditions = Vec::new(&env);
        conditions.push_back(String::from_str(&env, "Diabetes"));
        let mut goals = Vec::new(&env);
        goals.push_back(String::from_str(&env, "Goal"));

        let plan_id = client
            .create_care_plan(
                &patient,
                &provider,
                &Symbol::new(&env, "chronic_disease"),
                &conditions,
                &goals,
                &1_000_000u64,
                &30u32,
            )
            ;

        (env, provider, client, plan_id)
    };

    let intervention_id = client
        .add_intervention(
            &plan_id,
            &provider,
            &Symbol::new(&env, "medication"),
            &String::from_str(&env, "Metformin 500mg"),
            &String::from_str(&env, "Twice daily"),
            &Symbol::new(&env, "patient"),
        )
        ;

    assert_eq!(intervention_id, 1);
}

#[test]
fn test_add_intervention_plan_not_found() {
    let (env, provider, _) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let result = client.try_add_intervention(
        &999,
        &provider,
        &Symbol::new(&env, "exercise"),
        &String::from_str(&env, "Walk 30 min"),
        &String::from_str(&env, "Daily"),
        &Symbol::new(&env, "patient"),
    );

    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// record_goal_progress
// -----------------------------------------------------------------------

#[test]
fn test_record_goal_progress_success() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let goal_id = client
        .add_care_goal(
            &plan_id,
            &provider,
            &String::from_str(&env, "Reduce HbA1c"),
            &Some(String::from_str(&env, "7.0")),
            &2_000_000u64,
            &Symbol::new(&env, "high"),
        )
        ;

    client
        .record_goal_progress(
            &goal_id,
            &patient,
            &String::from_str(&env, "7.5"),
            &String::from_str(&env, "Progress noted"),
            &1_100_000u64,
        )
        ;
}

#[test]
fn test_record_goal_progress_goal_not_found() {
    let (env, _, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let result = client.try_record_goal_progress(
        &999,
        &patient,
        &String::from_str(&env, "7.2"),
        &String::from_str(&env, "Note"),
        &1_100_000u64,
    );

    assert!(result.is_err());
}

#[test]
fn test_record_goal_progress_on_achieved_goal_fails() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let goal_id = client
        .add_care_goal(
            &plan_id,
            &provider,
            &String::from_str(&env, "Reduce HbA1c"),
            &None,
            &2_000_000u64,
            &Symbol::new(&env, "high"),
        )
        ;

    client
        .mark_goal_achieved(
            &goal_id,
            &provider,
            &1_500_000u64,
            &String::from_str(&env, "Target met"),
        )
        ;

    let result = client.try_record_goal_progress(
        &goal_id,
        &patient,
        &String::from_str(&env, "6.9"),
        &String::from_str(&env, "Update"),
        &1_600_000u64,
    );

    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// mark_goal_achieved
// -----------------------------------------------------------------------

#[test]
fn test_mark_goal_achieved_success() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let goal_id = client
        .add_care_goal(
            &plan_id,
            &provider,
            &String::from_str(&env, "Target HbA1c"),
            &None,
            &2_000_000u64,
            &Symbol::new(&env, "high"),
        )
        ;

    client
        .mark_goal_achieved(
            &goal_id,
            &provider,
            &1_500_000u64,
            &String::from_str(&env, "Patient reached target"),
        )
        ;
}

#[test]
fn test_mark_goal_achieved_twice_fails() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let goal_id = client
        .add_care_goal(
            &plan_id,
            &provider,
            &String::from_str(&env, "Target HbA1c"),
            &None,
            &2_000_000u64,
            &Symbol::new(&env, "high"),
        )
        ;

    client
        .mark_goal_achieved(
            &goal_id,
            &provider,
            &1_500_000u64,
            &String::from_str(&env, "Done"),
        )
        ;

    let result = client.try_mark_goal_achieved(
        &goal_id,
        &provider,
        &1_600_000u64,
        &String::from_str(&env, "Again"),
    );

    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// add_barrier / resolve_barrier
// -----------------------------------------------------------------------

#[test]
fn test_add_barrier_success() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let barrier_id = client
        .add_barrier(
            &plan_id,
            &patient,
            &Symbol::new(&env, "financial"),
            &String::from_str(&env, "Cannot afford medication"),
            &1_050_000u64,
        )
        ;

    assert_eq!(barrier_id, 1);
}

#[test]
fn test_resolve_barrier_success() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let barrier_id = client
        .add_barrier(
            &plan_id,
            &patient,
            &Symbol::new(&env, "financial"),
            &String::from_str(&env, "Cannot afford medication"),
            &1_050_000u64,
        )
        ;

    client
        .resolve_barrier(
            &barrier_id,
            &provider,
            &String::from_str(&env, "Enrolled in assistance program"),
            &1_100_000u64,
        )
        ;
}

#[test]
fn test_resolve_barrier_twice_fails() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let barrier_id = client
        .add_barrier(
            &plan_id,
            &patient,
            &Symbol::new(&env, "financial"),
            &String::from_str(&env, "Cannot afford medication"),
            &1_050_000u64,
        )
        ;

    client
        .resolve_barrier(
            &barrier_id,
            &provider,
            &String::from_str(&env, "Resolved"),
            &1_100_000u64,
        )
        ;

    let result = client.try_resolve_barrier(
        &barrier_id,
        &provider,
        &String::from_str(&env, "Again"),
        &1_200_000u64,
    );

    assert!(result.is_err());
}

#[test]
fn test_add_barrier_plan_not_found() {
    let (env, _, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let result = client.try_add_barrier(
        &999,
        &patient,
        &Symbol::new(&env, "social"),
        &String::from_str(&env, "No transport"),
        &1_000_000u64,
    );

    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// schedule_care_plan_review / conduct_care_plan_review
// -----------------------------------------------------------------------

#[test]
fn test_schedule_review_success() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let review_id = client
        .schedule_care_plan_review(
            &plan_id,
            &provider,
            &3_600_000u64,
            &Symbol::new(&env, "routine"),
        )
        ;

    assert_eq!(review_id, 1);
}

#[test]
fn test_conduct_review_success() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let review_id = client
        .schedule_care_plan_review(
            &plan_id,
            &provider,
            &3_600_000u64,
            &Symbol::new(&env, "routine"),
        )
        ;

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let mut mods = Vec::new(&env);
    mods.push_back(String::from_str(&env, "Increase exercise frequency"));

    client
        .conduct_care_plan_review(&review_id, &provider, &hash, &mods, &true)
        ;
}

#[test]
fn test_conduct_review_twice_fails() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let review_id = client
        .schedule_care_plan_review(
            &plan_id,
            &provider,
            &3_600_000u64,
            &Symbol::new(&env, "routine"),
        )
        ;

    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let mods = Vec::new(&env);

    client
        .conduct_care_plan_review(&review_id, &provider, &hash, &mods, &true)
        ;

    let result =
        client.try_conduct_care_plan_review(&review_id, &provider, &hash, &mods, &true);

    assert!(result.is_err());
}

#[test]
fn test_conduct_review_updates_plan_dates() {
    let (env, provider, patient) = setup();
    env.ledger().set_timestamp(5_000_000);

    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let review_id = client
        .schedule_care_plan_review(
            &plan_id,
            &provider,
            &5_000_000u64,
            &Symbol::new(&env, "routine"),
        )
        ;

    let hash = BytesN::from_array(&env, &[2u8; 32]);
    let mods = Vec::new(&env);

    client
        .conduct_care_plan_review(&review_id, &provider, &hash, &mods, &true)
        ;

    let summary = client.get_care_plan_summary(&plan_id, &provider);
    assert_eq!(summary.last_review_date, Some(5_000_000));
    // next = 5_000_000 + 30 * 86_400
    assert_eq!(summary.next_review_date, 5_000_000 + 30 * 86_400);
}

// -----------------------------------------------------------------------
// assign_care_team_member
// -----------------------------------------------------------------------

#[test]
fn test_assign_care_team_member_success() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let specialist = Address::generate(&env);
    let mut responsibilities = Vec::new(&env);
    responsibilities.push_back(String::from_str(&env, "Monitor blood sugar"));

    client
        .assign_care_team_member(
            &plan_id,
            &provider,
            &specialist,
            &Symbol::new(&env, "specialist"),
            &responsibilities,
        )
        ;

    let summary = client.get_care_plan_summary(&plan_id, &provider);
    assert_eq!(summary.care_team.len(), 1);
    assert_eq!(summary.care_team.get(0).unwrap().team_member, specialist);
}

#[test]
fn test_assign_multiple_team_members() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let nurse = Address::generate(&env);
    let dietitian = Address::generate(&env);

    let mut r1 = Vec::new(&env);
    r1.push_back(String::from_str(&env, "Wound care"));
    let mut r2 = Vec::new(&env);
    r2.push_back(String::from_str(&env, "Nutrition plan"));

    client
        .assign_care_team_member(
            &plan_id,
            &provider,
            &nurse,
            &Symbol::new(&env, "nurse"),
            &r1,
        )
        ;

    client
        .assign_care_team_member(
            &plan_id,
            &provider,
            &dietitian,
            &Symbol::new(&env, "dietitian"),
            &r2,
        )
        ;

    let summary = client.get_care_plan_summary(&plan_id, &provider);
    assert_eq!(summary.care_team.len(), 2);
}

// -----------------------------------------------------------------------
// get_care_plan_summary
// -----------------------------------------------------------------------

#[test]
fn test_get_care_plan_summary_not_found() {
    let (env, provider, _) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let result = client.try_get_care_plan_summary(&999, &provider);
    assert!(result.is_err());
}

#[test]
fn test_get_care_plan_summary_excludes_achieved_goals() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes"));
    let mut goals = Vec::new(&env);
    goals.push_back(String::from_str(&env, "Goal"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    let goal_id = client
        .add_care_goal(
            &plan_id,
            &provider,
            &String::from_str(&env, "Active goal"),
            &None,
            &2_000_000u64,
            &Symbol::new(&env, "high"),
        )
        ;

    let achieved_goal_id = client
        .add_care_goal(
            &plan_id,
            &provider,
            &String::from_str(&env, "Achieved goal"),
            &None,
            &2_000_000u64,
            &Symbol::new(&env, "low"),
        )
        ;

    client
        .mark_goal_achieved(
            &achieved_goal_id,
            &provider,
            &1_500_000u64,
            &String::from_str(&env, "Done"),
        )
        ;

    let summary = client.get_care_plan_summary(&plan_id, &provider);
    assert_eq!(summary.active_goals.len(), 1);
    assert_eq!(summary.active_goals.get(0).unwrap().goal_id, goal_id);
}

// -----------------------------------------------------------------------
// Full workflow integration test
// -----------------------------------------------------------------------

#[test]
fn test_full_care_plan_workflow() {
    let (env, provider, patient) = setup();
    let contract_id = env.register(CarePlanContract, ());
    let client = CarePlanContractClient::new(&env, &contract_id);

    // 1. Create care plan
    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Type 2 Diabetes"));
    conditions.push_back(String::from_str(&env, "Hypertension"));

    let mut initial_goals = Vec::new(&env);
    initial_goals.push_back(String::from_str(&env, "Reduce HbA1c"));

    let plan_id = client
        .create_care_plan(
            &patient,
            &provider,
            &Symbol::new(&env, "chronic_disease"),
            &conditions,
            &initial_goals,
            &1_000_000u64,
            &30u32,
        )
        ;

    // 2. Add goals
    let goal_id = client
        .add_care_goal(
            &plan_id,
            &provider,
            &String::from_str(&env, "HbA1c below 7%"),
            &Some(String::from_str(&env, "6.9")),
            &3_000_000u64,
            &Symbol::new(&env, "high"),
        )
        ;

    // 3. Add intervention
    client
        .add_intervention(
            &plan_id,
            &provider,
            &Symbol::new(&env, "medication"),
            &String::from_str(&env, "Metformin"),
            &String::from_str(&env, "Twice daily"),
            &Symbol::new(&env, "patient"),
        )
        ;

    // 4. Add barrier
    let barrier_id = client
        .add_barrier(
            &plan_id,
            &patient,
            &Symbol::new(&env, "financial"),
            &String::from_str(&env, "Cannot afford test strips"),
            &1_100_000u64,
        )
        ;

    // 5. Record progress
    client
        .record_goal_progress(
            &goal_id,
            &patient,
            &String::from_str(&env, "7.8"),
            &String::from_str(&env, "Improving"),
            &1_200_000u64,
        )
        ;

    // 6. Assign team member
    let nurse = Address::generate(&env);
    let mut responsibilities = Vec::new(&env);
    responsibilities.push_back(String::from_str(&env, "Check vitals"));

    client
        .assign_care_team_member(
            &plan_id,
            &provider,
            &nurse,
            &Symbol::new(&env, "nurse"),
            &responsibilities,
        )
        ;

    // 7. Resolve barrier
    client
        .resolve_barrier(
            &barrier_id,
            &provider,
            &String::from_str(&env, "Patient enrolled in assistance program"),
            &1_300_000u64,
        )
        ;

    // 8. Schedule review
    let review_id = client
        .schedule_care_plan_review(
            &plan_id,
            &provider,
            &3_592_000u64,
            &Symbol::new(&env, "routine"),
        )
        ;

    // 9. Conduct review
    let hash = BytesN::from_array(&env, &[9u8; 32]);
    let mut mods = Vec::new(&env);
    mods.push_back(String::from_str(&env, "Increase Metformin to 1000mg"));

    client
        .conduct_care_plan_review(&review_id, &provider, &hash, &mods, &true)
        ;

    // 10. Mark goal achieved
    client
        .mark_goal_achieved(
            &goal_id,
            &provider,
            &2_500_000u64,
            &String::from_str(&env, "Patient reached HbA1c target"),
        )
        ;

    // 11. Verify summary
    let summary = client.get_care_plan_summary(&plan_id, &provider);
    assert_eq!(summary.care_plan_id, plan_id);
    assert_eq!(summary.active_goals.len(), 0); // achieved goal excluded
    assert_eq!(summary.interventions.len(), 1);
    assert_eq!(summary.care_team.len(), 1);
    // resolved barrier still shows in summary (resolved = true flag set)
    assert_eq!(summary.barriers.len(), 1);
    assert!(summary.barriers.get(0).unwrap().resolved);
    assert!(summary.last_review_date.is_some());
}
