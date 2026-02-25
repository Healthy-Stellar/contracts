#![no_std]

use soroban_sdk::{Address, Env, Vec};

use crate::types::{
    ComputedNeeds, DataKey, DietOrder, FoodIntakeRecord, MalnutritionScreening,
    NutritionAssessment, NutritionCarePlan, NutritionIntervention, OutcomeEvaluation,
    SupplementRecommendation, WeightEntry,
};

// -----------------------------------------------------------------------
// Counter helpers
// -----------------------------------------------------------------------

pub fn next_assessment_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::AssessmentCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::AssessmentCounter, &next);
    next
}

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

pub fn next_diet_order_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::DietOrderCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::DietOrderCounter, &next);
    next
}

// -----------------------------------------------------------------------
// NutritionAssessment
// -----------------------------------------------------------------------

pub fn save_assessment(env: &Env, a: &NutritionAssessment) {
    env.storage()
        .persistent()
        .set(&DataKey::Assessment(a.assessment_id), a);
}

pub fn load_assessment(env: &Env, id: u64) -> Option<NutritionAssessment> {
    env.storage().persistent().get(&DataKey::Assessment(id))
}

pub fn add_patient_assessment(env: &Env, patient_id: &Address, assessment_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PatientAssessments(patient_id.clone()))
        .unwrap_or(Vec::new(env));
    ids.push_back(assessment_id);
    env.storage()
        .persistent()
        .set(&DataKey::PatientAssessments(patient_id.clone()), &ids);
}

// -----------------------------------------------------------------------
// ComputedNeeds
// -----------------------------------------------------------------------

pub fn save_computed_needs(env: &Env, cn: &ComputedNeeds) {
    env.storage()
        .persistent()
        .set(&DataKey::ComputedNeeds(cn.assessment_id), cn);
}

pub fn load_computed_needs(env: &Env, assessment_id: u64) -> Option<ComputedNeeds> {
    env.storage()
        .persistent()
        .get(&DataKey::ComputedNeeds(assessment_id))
}

// -----------------------------------------------------------------------
// NutritionCarePlan
// -----------------------------------------------------------------------

pub fn save_care_plan(env: &Env, plan: &NutritionCarePlan) {
    env.storage()
        .persistent()
        .set(&DataKey::CarePlan(plan.care_plan_id), plan);
}

pub fn load_care_plan(env: &Env, id: u64) -> Option<NutritionCarePlan> {
    env.storage().persistent().get(&DataKey::CarePlan(id))
}

// -----------------------------------------------------------------------
// DietOrder
// -----------------------------------------------------------------------

pub fn save_diet_order(env: &Env, order: &DietOrder) {
    env.storage()
        .persistent()
        .set(&DataKey::DietOrder(order.order_id), order);
}

pub fn load_diet_order(env: &Env, id: u64) -> Option<DietOrder> {
    env.storage().persistent().get(&DataKey::DietOrder(id))
}

pub fn add_patient_diet_order(env: &Env, patient_id: &Address, order_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PatientDietOrders(patient_id.clone()))
        .unwrap_or(Vec::new(env));
    ids.push_back(order_id);
    env.storage()
        .persistent()
        .set(&DataKey::PatientDietOrders(patient_id.clone()), &ids);
}

// -----------------------------------------------------------------------
// NutritionIntervention (list per care plan)
// -----------------------------------------------------------------------

pub fn append_intervention(env: &Env, care_plan_id: u64, entry: &NutritionIntervention) {
    let mut list: Vec<NutritionIntervention> = env
        .storage()
        .persistent()
        .get(&DataKey::Interventions(care_plan_id))
        .unwrap_or(Vec::new(env));
    list.push_back(entry.clone());
    env.storage()
        .persistent()
        .set(&DataKey::Interventions(care_plan_id), &list);
}

pub fn load_interventions(env: &Env, care_plan_id: u64) -> Vec<NutritionIntervention> {
    env.storage()
        .persistent()
        .get(&DataKey::Interventions(care_plan_id))
        .unwrap_or(Vec::new(env))
}

// -----------------------------------------------------------------------
// FoodIntake
// -----------------------------------------------------------------------

pub fn append_food_intake(env: &Env, patient_id: &Address, record: &FoodIntakeRecord) {
    let mut list: Vec<FoodIntakeRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::FoodIntake(patient_id.clone()))
        .unwrap_or(Vec::new(env));
    list.push_back(record.clone());
    env.storage()
        .persistent()
        .set(&DataKey::FoodIntake(patient_id.clone()), &list);
}

pub fn load_food_intake(env: &Env, patient_id: &Address) -> Vec<FoodIntakeRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::FoodIntake(patient_id.clone()))
        .unwrap_or(Vec::new(env))
}

// -----------------------------------------------------------------------
// WeightHistory
// -----------------------------------------------------------------------

pub fn append_weight_entry(env: &Env, patient_id: &Address, entry: &WeightEntry) {
    let mut list: Vec<WeightEntry> = env
        .storage()
        .persistent()
        .get(&DataKey::WeightHistory(patient_id.clone()))
        .unwrap_or(Vec::new(env));
    list.push_back(entry.clone());
    env.storage()
        .persistent()
        .set(&DataKey::WeightHistory(patient_id.clone()), &list);
}

pub fn load_weight_history(env: &Env, patient_id: &Address) -> Vec<WeightEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::WeightHistory(patient_id.clone()))
        .unwrap_or(Vec::new(env))
}

// -----------------------------------------------------------------------
// MalnutritionScreening
// -----------------------------------------------------------------------

pub fn save_malnutrition_screening(env: &Env, s: &MalnutritionScreening) {
    env.storage()
        .persistent()
        .set(&DataKey::MalnutritionScreening(s.assessment_id), s);
}

pub fn load_malnutrition_screening(env: &Env, assessment_id: u64) -> Option<MalnutritionScreening> {
    env.storage()
        .persistent()
        .get(&DataKey::MalnutritionScreening(assessment_id))
}

// -----------------------------------------------------------------------
// Supplements
// -----------------------------------------------------------------------

pub fn append_supplement(env: &Env, care_plan_id: u64, rec: &SupplementRecommendation) {
    let mut list: Vec<SupplementRecommendation> = env
        .storage()
        .persistent()
        .get(&DataKey::Supplements(care_plan_id))
        .unwrap_or(Vec::new(env));
    list.push_back(rec.clone());
    env.storage()
        .persistent()
        .set(&DataKey::Supplements(care_plan_id), &list);
}

pub fn load_supplements(env: &Env, care_plan_id: u64) -> Vec<SupplementRecommendation> {
    env.storage()
        .persistent()
        .get(&DataKey::Supplements(care_plan_id))
        .unwrap_or(Vec::new(env))
}

// -----------------------------------------------------------------------
// OutcomeEvaluation
// -----------------------------------------------------------------------

pub fn save_outcome_evaluation(env: &Env, ev: &OutcomeEvaluation) {
    env.storage()
        .persistent()
        .set(&DataKey::OutcomeEvaluation(ev.care_plan_id), ev);
}

pub fn load_outcome_evaluation(env: &Env, care_plan_id: u64) -> Option<OutcomeEvaluation> {
    env.storage()
        .persistent()
        .get(&DataKey::OutcomeEvaluation(care_plan_id))
}
