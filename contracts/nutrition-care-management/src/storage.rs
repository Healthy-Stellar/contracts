use shared_contracts::safe_increment_persistent;
use soroban_sdk::{Address, Env, Vec};

use crate::types::{
    ClinicalOutcome, ComputedNeeds, DataKey, DietOrder, FoodIntakeRecord, MalnutritionScreening,
    NutritionAssessment, NutritionCarePlan, NutritionIntervention, OutcomeEvaluation,
    SupplementRecommendation, WeightEntry,
};

// -----------------------------------------------------------------------
// Counter helpers
// -----------------------------------------------------------------------

pub fn next_assessment_id(env: &Env) -> u64 {
    safe_increment_persistent(env, &DataKey::AssessmentCounter)
}

pub fn next_care_plan_id(env: &Env) -> u64 {
    safe_increment_persistent(env, &DataKey::CarePlanCounter)
}

pub fn next_diet_order_id(env: &Env) -> u64 {
    safe_increment_persistent(env, &DataKey::DietOrderCounter)
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

// -----------------------------------------------------------------------
// ClinicalOutcome (#393)
// -----------------------------------------------------------------------

pub fn next_outcome_id(env: &Env) -> u64 {
    safe_increment_persistent(env, &DataKey::OutcomeCounter)
}

pub fn save_clinical_outcome(env: &Env, outcome: &ClinicalOutcome) {
    env.storage()
        .persistent()
        .set(&DataKey::ClinicalOutcome(outcome.outcome_id), outcome);
}

pub fn load_clinical_outcome(env: &Env, outcome_id: u64) -> Option<ClinicalOutcome> {
    env.storage()
        .persistent()
        .get(&DataKey::ClinicalOutcome(outcome_id))
}

pub fn append_plan_outcome(env: &Env, care_plan_id: u64, outcome_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PlanOutcomes(care_plan_id))
        .unwrap_or(Vec::new(env));
    ids.push_back(outcome_id);
    env.storage()
        .persistent()
        .set(&DataKey::PlanOutcomes(care_plan_id), &ids);
}

pub fn load_plan_outcomes(env: &Env, care_plan_id: u64) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PlanOutcomes(care_plan_id))
        .unwrap_or(Vec::new(env))
}

pub fn get_plan_version(env: &Env, care_plan_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::PlanVersion(care_plan_id))
        .unwrap_or(1)
}

pub fn increment_plan_version(env: &Env, care_plan_id: u64) {
    let current = get_plan_version(env, care_plan_id);
    env.storage()
        .persistent()
        .set(&DataKey::PlanVersion(care_plan_id), &(current + 1));
}

pub fn add_authorized_provider(env: &Env, care_plan_id: u64, provider: &Address) {
    let mut providers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::AuthorizedProviders(care_plan_id))
        .unwrap_or(Vec::new(env));
    
    // Check if provider is already authorized
    for p in providers.iter() {
        if p == *provider {
            return; // Already authorized
        }
    }
    
    providers.push_back(provider.clone());
    env.storage()
        .persistent()
        .set(&DataKey::AuthorizedProviders(care_plan_id), &providers);
}

pub fn is_provider_authorized(env: &Env, care_plan_id: u64, provider: &Address) -> bool {
    let providers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::AuthorizedProviders(care_plan_id))
        .unwrap_or(Vec::new(env));
    
    for p in providers.iter() {
        if p == *provider {
            return true;
        }
    }
    false
}
