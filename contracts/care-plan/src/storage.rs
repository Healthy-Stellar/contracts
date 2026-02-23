#![no_std]

use soroban_sdk::{Address, Env, Vec};

use crate::types::{
    Barrier, CarePlan, CareReview, CareTeamMember, CareGoal, DataKey, Intervention,
};

// -----------------------------------------------------------------------
// Counter helpers
// -----------------------------------------------------------------------

pub fn next_care_plan_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::CarePlanCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::CarePlanCounter, &next);
    next
}

pub fn next_goal_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::GoalCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::GoalCounter, &next);
    next
}

pub fn next_intervention_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::InterventionCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::InterventionCounter, &next);
    next
}

pub fn next_barrier_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::BarrierCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::BarrierCounter, &next);
    next
}

pub fn next_review_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::ReviewCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::ReviewCounter, &next);
    next
}

// -----------------------------------------------------------------------
// CarePlan
// -----------------------------------------------------------------------

pub fn save_care_plan(env: &Env, plan: &CarePlan) {
    env.storage()
        .persistent()
        .set(&DataKey::CarePlan(plan.care_plan_id), plan);
}

pub fn load_care_plan(env: &Env, care_plan_id: u64) -> Option<CarePlan> {
    env.storage()
        .persistent()
        .get(&DataKey::CarePlan(care_plan_id))
}

pub fn add_patient_plan(env: &Env, patient_id: &Address, care_plan_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PatientPlans(patient_id.clone()))
        .unwrap_or(Vec::new(env));
    ids.push_back(care_plan_id);
    env.storage()
        .persistent()
        .set(&DataKey::PatientPlans(patient_id.clone()), &ids);
}

// -----------------------------------------------------------------------
// CareGoal
// -----------------------------------------------------------------------

pub fn save_goal(env: &Env, goal: &CareGoal) {
    env.storage()
        .persistent()
        .set(&DataKey::Goal(goal.goal_id), goal);
}

pub fn load_goal(env: &Env, goal_id: u64) -> Option<CareGoal> {
    env.storage().persistent().get(&DataKey::Goal(goal_id))
}

pub fn add_plan_goal(env: &Env, care_plan_id: u64, goal_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PlanGoals(care_plan_id))
        .unwrap_or(Vec::new(env));
    ids.push_back(goal_id);
    env.storage()
        .persistent()
        .set(&DataKey::PlanGoals(care_plan_id), &ids);
}

pub fn load_plan_goals(env: &Env, care_plan_id: u64) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PlanGoals(care_plan_id))
        .unwrap_or(Vec::new(env))
}

// -----------------------------------------------------------------------
// Intervention
// -----------------------------------------------------------------------

pub fn save_intervention(env: &Env, intervention: &Intervention) {
    env.storage()
        .persistent()
        .set(&DataKey::Intervention(intervention.intervention_id), intervention);
}

pub fn load_intervention(env: &Env, intervention_id: u64) -> Option<Intervention> {
    env.storage()
        .persistent()
        .get(&DataKey::Intervention(intervention_id))
}

pub fn add_plan_intervention(env: &Env, care_plan_id: u64, intervention_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PlanInterventions(care_plan_id))
        .unwrap_or(Vec::new(env));
    ids.push_back(intervention_id);
    env.storage()
        .persistent()
        .set(&DataKey::PlanInterventions(care_plan_id), &ids);
}

pub fn load_plan_interventions(env: &Env, care_plan_id: u64) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PlanInterventions(care_plan_id))
        .unwrap_or(Vec::new(env))
}

// -----------------------------------------------------------------------
// Barrier
// -----------------------------------------------------------------------

pub fn save_barrier(env: &Env, barrier: &Barrier) {
    env.storage()
        .persistent()
        .set(&DataKey::Barrier(barrier.barrier_id), barrier);
}

pub fn load_barrier(env: &Env, barrier_id: u64) -> Option<Barrier> {
    env.storage()
        .persistent()
        .get(&DataKey::Barrier(barrier_id))
}

pub fn add_plan_barrier(env: &Env, care_plan_id: u64, barrier_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PlanBarriers(care_plan_id))
        .unwrap_or(Vec::new(env));
    ids.push_back(barrier_id);
    env.storage()
        .persistent()
        .set(&DataKey::PlanBarriers(care_plan_id), &ids);
}

pub fn load_plan_barriers(env: &Env, care_plan_id: u64) -> Vec<Barrier> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PlanBarriers(care_plan_id))
        .unwrap_or(Vec::new(env));
    let mut barriers = Vec::new(env);
    for id in ids.iter() {
        if let Some(b) = load_barrier(env, id) {
            barriers.push_back(b);
        }
    }
    barriers
}

// -----------------------------------------------------------------------
// CareReview
// -----------------------------------------------------------------------

pub fn save_review(env: &Env, review: &CareReview) {
    env.storage()
        .persistent()
        .set(&DataKey::Review(review.review_id), review);
}

pub fn load_review(env: &Env, review_id: u64) -> Option<CareReview> {
    env.storage()
        .persistent()
        .get(&DataKey::Review(review_id))
}

pub fn add_plan_review(env: &Env, care_plan_id: u64, review_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PlanReviews(care_plan_id))
        .unwrap_or(Vec::new(env));
    ids.push_back(review_id);
    env.storage()
        .persistent()
        .set(&DataKey::PlanReviews(care_plan_id), &ids);
}

// -----------------------------------------------------------------------
// Care team
// -----------------------------------------------------------------------

pub fn load_care_team(env: &Env, care_plan_id: u64) -> Vec<CareTeamMember> {
    env.storage()
        .persistent()
        .get(&DataKey::PlanCareTeam(care_plan_id))
        .unwrap_or(Vec::new(env))
}

pub fn save_care_team(env: &Env, care_plan_id: u64, team: &Vec<CareTeamMember>) {
    env.storage()
        .persistent()
        .set(&DataKey::PlanCareTeam(care_plan_id), team);
}