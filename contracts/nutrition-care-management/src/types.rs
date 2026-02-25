#![no_std]

use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol, Vec};

// -----------------------------------------------------------------------
// Error types
// -----------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    AssessmentNotFound = 2,
    CarePlanNotFound = 3,
    DietOrderNotFound = 4,
    InvalidScreeningTool = 5,
    InvalidRiskLevel = 6,
    AssessmentAlreadyHasMalnutritionRisk = 7,
    WeightRecordingFailed = 8,
    InvalidActivityLevel = 9,
}

// -----------------------------------------------------------------------
// Core data structures required by the spec
// -----------------------------------------------------------------------

/// Macronutrient and fluid needs calculated for a patient.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NutritionalNeeds {
    pub calories_per_day: u32,
    pub protein_grams: u32, // stored as integer (grams * 10 for one decimal) – we use u32 for no_std compat
    pub carbohydrate_grams: u32,
    pub fat_grams: u32,
    pub fluid_ml: u32,
}

/// A single nutrition goal inside a care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NutritionGoal {
    pub goal_description: String,
    pub target_date: u64,
    pub measurement_method: String,
    pub achieved: bool,
}

/// A food item logged during a meal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoodItem {
    pub food_name: String,
    pub portion_size: String,
    pub calories: u32,
    pub protein_grams: u32,
}

// -----------------------------------------------------------------------
// Internal record structs
// -----------------------------------------------------------------------

/// Top-level nutrition assessment record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NutritionAssessment {
    pub assessment_id: u64,
    pub patient_id: Address,
    pub dietitian_id: Address,
    pub assessment_date: u64,
    /// Height stored as centimetres × 100 to avoid floats (e.g. 175.5 cm → 17550).
    pub height_cm_x100: i64,
    /// Weight stored as kilograms × 100 (e.g. 70.3 kg → 7030).
    pub weight_kg_x100: i64,
    /// BMI stored as value × 100 (e.g. 22.9 → 2290).
    pub bmi_x100: i64,
    pub dietary_history_hash: BytesN<32>,
    pub nutritional_risk_factors: Vec<String>,
    pub created_at: u64,
}

/// Computed nutritional needs linked to an assessment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedNeeds {
    pub assessment_id: u64,
    pub activity_level: Symbol,
    /// Stress factor × 100 (e.g. 1.25 → 125).
    pub stress_factor_x100: i64,
    pub special_considerations: Vec<String>,
    pub needs: NutritionalNeeds,
    pub computed_at: u64,
}

/// A nutrition care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NutritionCarePlan {
    pub care_plan_id: u64,
    pub assessment_id: u64,
    pub dietitian_id: Address,
    pub nutrition_diagnoses: Vec<String>,
    pub goals: Vec<NutritionGoal>,
    pub interventions: Vec<String>,
    pub follow_up_frequency: String,
    pub created_at: u64,
}

/// A therapeutic diet order.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DietOrder {
    pub order_id: u64,
    pub patient_id: Address,
    pub ordering_provider: Address,
    /// regular | cardiac | diabetic | renal | low_sodium
    pub diet_type: Symbol,
    pub texture_modification: Option<Symbol>,
    pub fluid_restriction_ml: Option<u32>,
    pub calorie_target: Option<u32>,
    pub special_instructions: Option<String>,
    pub ordered_at: u64,
    pub active: bool,
}

/// A documented nutrition intervention session.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NutritionIntervention {
    pub care_plan_id: u64,
    pub intervention_date: u64,
    /// counseling | education | meal_planning
    pub intervention_type: Symbol,
    pub topics_covered: Vec<String>,
    pub duration_minutes: u32,
    /// excellent | good | fair | poor
    pub patient_comprehension: Symbol,
    pub recorded_at: u64,
}

/// A single food-intake record tied to a patient.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoodIntakeRecord {
    pub patient_id: Address,
    pub meal_date: u64,
    /// breakfast | lunch | dinner | snack
    pub meal_type: Symbol,
    pub foods_consumed: Vec<FoodItem>,
    /// 0‑100 percent of the meal consumed.
    pub percentage_consumed: u32,
    pub recorded_at: u64,
}

/// A weight measurement entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightEntry {
    pub patient_id: Address,
    pub measurement_date: u64,
    /// Weight stored as kilograms × 100.
    pub weight_kg_x100: i64,
    /// measured | patient_reported
    pub method: Symbol,
    pub recorded_at: u64,
}

/// Malnutrition risk screening linked to an assessment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MalnutritionScreening {
    pub assessment_id: u64,
    /// must | nrs2002 | mna
    pub screening_tool: Symbol,
    pub score: u32,
    /// low | medium | high
    pub risk_level: Symbol,
    pub screened_at: u64,
}

/// A supplement recommendation inside a care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplementRecommendation {
    pub care_plan_id: u64,
    pub dietitian_id: Address,
    pub supplement_type: Symbol,
    pub dosage: String,
    pub rationale: String,
    pub recommended_at: u64,
}

/// Outcome evaluation record for a care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutcomeEvaluation {
    pub care_plan_id: u64,
    pub evaluation_date: u64,
    /// Weight-change stored as kilograms × 100 (may be negative).
    pub weight_change_kg_x100: i64,
    pub lab_improvements: Vec<String>,
    pub goals_met: Vec<String>,
    pub continue_care: bool,
    pub evaluated_at: u64,
}

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Auto-increment counter for assessments.
    AssessmentCounter,
    /// Auto-increment counter for care plans.
    CarePlanCounter,
    /// Auto-increment counter for diet orders.
    DietOrderCounter,

    /// assessment_id → NutritionAssessment
    Assessment(u64),
    /// assessment_id → ComputedNeeds
    ComputedNeeds(u64),
    /// care_plan_id → NutritionCarePlan
    CarePlan(u64),
    /// order_id → DietOrder
    DietOrder(u64),
    /// care_plan_id → Vec<NutritionIntervention>
    Interventions(u64),
    /// patient_id → Vec<FoodIntakeRecord>
    FoodIntake(Address),
    /// patient_id → Vec<WeightEntry>
    WeightHistory(Address),
    /// assessment_id → MalnutritionScreening
    MalnutritionScreening(u64),
    /// care_plan_id → Vec<SupplementRecommendation>
    Supplements(u64),
    /// care_plan_id → OutcomeEvaluation
    OutcomeEvaluation(u64),
    /// patient_id → Vec<u64> (assessment ids)
    PatientAssessments(Address),
    /// patient_id → Vec<u64> (diet order ids)
    PatientDietOrders(Address),
}
