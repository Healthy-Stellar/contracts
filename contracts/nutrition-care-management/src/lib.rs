#![no_std]

mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, BytesN, Env, String, Symbol, Vec,
};
use storage::*;
use types::*;

#[contract]
pub struct NutritionCareContract;

#[contractimpl]
impl NutritionCareContract {
    // ------------------------------------------------------------------
    // 1. conduct_nutrition_assessment
    // ------------------------------------------------------------------

    /// Record a new nutritional assessment for a patient.
    ///
    /// Floating-point values (height_cm, weight_kg, bmi) are passed as
    /// integer × 100 so that the Soroban `no_std` environment can handle
    /// them without requiring the `f32` primitive (which compiles fine in
    /// Rust but is blocked by some soroban-sdk serialisation constraints).
    /// Callers should multiply by 100 before invoking (e.g. 175.5 cm → 17550).
    pub fn conduct_nutrition_assessment(
        env: Env,
        patient_id: Address,
        dietitian_id: Address,
        assessment_date: u64,
        height_cm_x100: i64,
        weight_kg_x100: i64,
        bmi_x100: i64,
        dietary_history_hash: BytesN<32>,
        nutritional_risk_factors: Vec<String>,
    ) -> Result<u64, Error> {
        dietitian_id.require_auth();

        let assessment_id = next_assessment_id(&env);

        let assessment = NutritionAssessment {
            assessment_id,
            patient_id: patient_id.clone(),
            dietitian_id: dietitian_id.clone(),
            assessment_date,
            height_cm_x100,
            weight_kg_x100,
            bmi_x100,
            dietary_history_hash,
            nutritional_risk_factors,
            created_at: env.ledger().timestamp(),
        };

        save_assessment(&env, &assessment);
        add_patient_assessment(&env, &patient_id, assessment_id);

        env.events().publish(
            (Symbol::new(&env, "assessment_created"),),
            (assessment_id, patient_id, dietitian_id),
        );

        Ok(assessment_id)
    }

    // ------------------------------------------------------------------
    // 2. calculate_nutritional_needs
    // ------------------------------------------------------------------

    /// Calculate and store a patient's macronutrient and fluid requirements.
    ///
    /// Uses the Harris–Benedict equation (simplified) combined with the
    /// provided activity and stress factors.
    ///
    /// `stress_factor_x100` – the clinical stress factor × 100
    /// (e.g. major surgery = 1.4 → pass 140).
    ///
    /// Returns the computed `NutritionalNeeds`.
    pub fn calculate_nutritional_needs(
        env: Env,
        assessment_id: u64,
        activity_level: Symbol,
        stress_factor_x100: i64,
        special_considerations: Vec<String>,
    ) -> Result<NutritionalNeeds, Error> {
        let assessment = load_assessment(&env, assessment_id).ok_or(Error::AssessmentNotFound)?;

        // Activity multiplier × 100 based on keyword matching.
        // symbol_short! is the correct no_std way to compare Symbol values.
        let activity_factor_x100: i64 = if activity_level == symbol_short!("sedntry") {
            120
        } else if activity_level == symbol_short!("light") {
            137
        } else if activity_level == symbol_short!("moderate") {
            155
        } else if activity_level == symbol_short!("active") {
            172
        } else if activity_level == symbol_short!("vactive") {
            190
        } else {
            return Err(Error::InvalidActivityLevel);
        };

        // Simplified Mifflin-St Jeor base metabolic rate (BMR) in kcal/day.
        // BMR ≈ (10 × weight_kg) + (6.25 × height_cm) − 5 × 30 + 5
        // We work entirely in integer arithmetic.
        let weight_kg = assessment.weight_kg_x100 / 100;
        let height_cm = assessment.height_cm_x100 / 100;

        // BMR × 10 to keep precision, then divide at the end.
        let bmr_x10: i64 = (10 * weight_kg * 10) + (625 * height_cm / 10) - (5 * 30 * 10) + 50;

        // TDEE = BMR × activity_factor × stress_factor
        let tdee_x1000000: i64 = bmr_x10 * activity_factor_x100 * stress_factor_x100;
        let calories_per_day = (tdee_x1000000 / 10_000_000).max(1) as u32;

        // Macronutrients (stored as integer grams):
        //   Protein  1.2 g/kg
        //   Carbs    50 % of calories ÷ 4 kcal/g
        //   Fat      30 % of calories ÷ 9 kcal/g
        let protein_grams = (weight_kg * 12 / 10).max(1) as u32;
        let carb_grams = ((calories_per_day as i64 * 50) / (4 * 100)).max(1) as u32;
        let fat_grams = ((calories_per_day as i64 * 30) / (9 * 100)).max(1) as u32;

        // Fluid: 30 ml/kg body weight
        let fluid_ml = (weight_kg * 30).max(500) as u32;

        let needs = NutritionalNeeds {
            calories_per_day,
            protein_grams,
            carbohydrate_grams: carb_grams,
            fat_grams,
            fluid_ml,
        };

        let computed = ComputedNeeds {
            assessment_id,
            activity_level,
            stress_factor_x100,
            special_considerations,
            needs: needs.clone(),
            computed_at: env.ledger().timestamp(),
        };

        save_computed_needs(&env, &computed);

        env.events()
            .publish((Symbol::new(&env, "needs_calculated"),), (assessment_id,));

        Ok(needs)
    }

    // ------------------------------------------------------------------
    // 3. create_nutrition_care_plan
    // ------------------------------------------------------------------

    /// Create a nutrition care plan linked to a completed assessment.
    pub fn create_nutrition_care_plan(
        env: Env,
        assessment_id: u64,
        dietitian_id: Address,
        nutrition_diagnoses: Vec<String>,
        goals: Vec<NutritionGoal>,
        interventions: Vec<String>,
        follow_up_frequency: String,
    ) -> Result<u64, Error> {
        dietitian_id.require_auth();

        load_assessment(&env, assessment_id).ok_or(Error::AssessmentNotFound)?;

        let care_plan_id = next_care_plan_id(&env);

        let plan = NutritionCarePlan {
            care_plan_id,
            assessment_id,
            dietitian_id: dietitian_id.clone(),
            nutrition_diagnoses,
            goals,
            interventions,
            follow_up_frequency,
            created_at: env.ledger().timestamp(),
        };

        save_care_plan(&env, &plan);

        env.events().publish(
            (Symbol::new(&env, "care_plan_created"),),
            (care_plan_id, assessment_id, dietitian_id),
        );

        Ok(care_plan_id)
    }

    // ------------------------------------------------------------------
    // 4. order_therapeutic_diet
    // ------------------------------------------------------------------

    /// Place a therapeutic diet order for a patient.
    pub fn order_therapeutic_diet(
        env: Env,
        patient_id: Address,
        ordering_provider: Address,
        diet_type: Symbol,
        texture_modification: Option<Symbol>,
        fluid_restriction_ml: Option<u32>,
        calorie_target: Option<u32>,
        special_instructions: Option<String>,
    ) -> Result<u64, Error> {
        ordering_provider.require_auth();

        let order_id = next_diet_order_id(&env);

        let order = DietOrder {
            order_id,
            patient_id: patient_id.clone(),
            ordering_provider: ordering_provider.clone(),
            diet_type,
            texture_modification,
            fluid_restriction_ml,
            calorie_target,
            special_instructions,
            ordered_at: env.ledger().timestamp(),
            active: true,
        };

        save_diet_order(&env, &order);
        add_patient_diet_order(&env, &patient_id, order_id);

        env.events().publish(
            (Symbol::new(&env, "diet_order_placed"),),
            (order_id, patient_id, ordering_provider),
        );

        Ok(order_id)
    }

    // ------------------------------------------------------------------
    // 5. document_nutrition_intervention
    // ------------------------------------------------------------------

    /// Document a nutrition intervention session against a care plan.
    pub fn document_nutrition_intervention(
        env: Env,
        care_plan_id: u64,
        intervention_date: u64,
        intervention_type: Symbol,
        topics_covered: Vec<String>,
        duration_minutes: u32,
        patient_comprehension: Symbol,
    ) -> Result<(), Error> {
        load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;

        let entry = NutritionIntervention {
            care_plan_id,
            intervention_date,
            intervention_type,
            topics_covered,
            duration_minutes,
            patient_comprehension,
            recorded_at: env.ledger().timestamp(),
        };

        append_intervention(&env, care_plan_id, &entry);

        env.events().publish(
            (Symbol::new(&env, "intervention_documented"),),
            (care_plan_id, intervention_date),
        );

        Ok(())
    }

    // ------------------------------------------------------------------
    // 6. track_food_intake
    // ------------------------------------------------------------------

    /// Record a patient's food intake for a specific meal.
    pub fn track_food_intake(
        env: Env,
        patient_id: Address,
        meal_date: u64,
        meal_type: Symbol,
        foods_consumed: Vec<FoodItem>,
        percentage_consumed: u32,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let record = FoodIntakeRecord {
            patient_id: patient_id.clone(),
            meal_date,
            meal_type,
            foods_consumed,
            percentage_consumed,
            recorded_at: env.ledger().timestamp(),
        };

        append_food_intake(&env, &patient_id, &record);

        env.events().publish(
            (Symbol::new(&env, "food_intake_tracked"),),
            (patient_id, meal_date),
        );

        Ok(())
    }

    // ------------------------------------------------------------------
    // 7. monitor_weight_trend
    // ------------------------------------------------------------------

    /// Record a weight measurement for ongoing trend monitoring.
    ///
    /// `weight_kg_x100` – weight in kg × 100 (e.g. 7030 for 70.30 kg).
    pub fn monitor_weight_trend(
        env: Env,
        patient_id: Address,
        measurement_date: u64,
        weight_kg_x100: i64,
        method: Symbol,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let entry = WeightEntry {
            patient_id: patient_id.clone(),
            measurement_date,
            weight_kg_x100,
            method,
            recorded_at: env.ledger().timestamp(),
        };

        append_weight_entry(&env, &patient_id, &entry);

        env.events().publish(
            (Symbol::new(&env, "weight_recorded"),),
            (patient_id, measurement_date, weight_kg_x100),
        );

        Ok(())
    }

    // ------------------------------------------------------------------
    // 8. assess_malnutrition_risk
    // ------------------------------------------------------------------

    /// Attach a malnutrition-risk screening result to an assessment.
    ///
    /// Valid screening tools: `must`, `nrs2002`, `mna`.
    /// Valid risk levels: `low`, `medium`, `high`.
    pub fn assess_malnutrition_risk(
        env: Env,
        assessment_id: u64,
        screening_tool: Symbol,
        score: u32,
        risk_level: Symbol,
    ) -> Result<(), Error> {
        load_assessment(&env, assessment_id).ok_or(Error::AssessmentNotFound)?;

        // Validate screening tool
        let valid_tools = [
            symbol_short!("must"),
            symbol_short!("nrs2002"),
            symbol_short!("mna"),
        ];
        if !valid_tools.contains(&screening_tool) {
            return Err(Error::InvalidScreeningTool);
        }

        // Validate risk level
        let valid_risks = [
            symbol_short!("low"),
            symbol_short!("medium"),
            symbol_short!("high"),
        ];
        if !valid_risks.contains(&risk_level) {
            return Err(Error::InvalidRiskLevel);
        }

        let screening = MalnutritionScreening {
            assessment_id,
            screening_tool,
            score,
            risk_level,
            screened_at: env.ledger().timestamp(),
        };

        save_malnutrition_screening(&env, &screening);

        env.events().publish(
            (Symbol::new(&env, "malnutrition_risk_assessed"),),
            (assessment_id, score),
        );

        Ok(())
    }

    // ------------------------------------------------------------------
    // 9. recommend_supplements
    // ------------------------------------------------------------------

    /// Add a supplement recommendation to an existing nutrition care plan.
    pub fn recommend_supplements(
        env: Env,
        care_plan_id: u64,
        dietitian_id: Address,
        supplement_type: Symbol,
        dosage: String,
        rationale: String,
    ) -> Result<(), Error> {
        dietitian_id.require_auth();

        load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;

        let rec = SupplementRecommendation {
            care_plan_id,
            dietitian_id: dietitian_id.clone(),
            supplement_type,
            dosage,
            rationale,
            recommended_at: env.ledger().timestamp(),
        };

        append_supplement(&env, care_plan_id, &rec);

        env.events().publish(
            (Symbol::new(&env, "supplement_recommended"),),
            (care_plan_id, dietitian_id),
        );

        Ok(())
    }

    // ------------------------------------------------------------------
    // 10. evaluate_nutrition_outcomes
    // ------------------------------------------------------------------

    /// Record a formal outcome evaluation for a nutrition care plan.
    ///
    /// `weight_change_kg_x100` – signed weight change in kg × 100
    /// (negative = weight loss, e.g. −200 = −2.00 kg).
    pub fn evaluate_nutrition_outcomes(
        env: Env,
        care_plan_id: u64,
        evaluation_date: u64,
        weight_change_kg_x100: i64,
        lab_improvements: Vec<String>,
        goals_met: Vec<String>,
        continue_care: bool,
    ) -> Result<(), Error> {
        load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;

        let evaluation = OutcomeEvaluation {
            care_plan_id,
            evaluation_date,
            weight_change_kg_x100,
            lab_improvements,
            goals_met,
            continue_care,
            evaluated_at: env.ledger().timestamp(),
        };

        save_outcome_evaluation(&env, &evaluation);

        env.events().publish(
            (Symbol::new(&env, "outcomes_evaluated"),),
            (care_plan_id, evaluation_date, continue_care),
        );

        Ok(())
    }

    // ------------------------------------------------------------------
    // Query helpers
    // ------------------------------------------------------------------

    /// Retrieve a stored nutrition assessment.
    pub fn get_assessment(env: Env, assessment_id: u64) -> Result<NutritionAssessment, Error> {
        load_assessment(&env, assessment_id).ok_or(Error::AssessmentNotFound)
    }

    /// Retrieve the computed nutritional needs for an assessment.
    pub fn get_nutritional_needs(env: Env, assessment_id: u64) -> Result<ComputedNeeds, Error> {
        load_computed_needs(&env, assessment_id).ok_or(Error::AssessmentNotFound)
    }

    /// Retrieve a nutrition care plan.
    pub fn get_care_plan(env: Env, care_plan_id: u64) -> Result<NutritionCarePlan, Error> {
        load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)
    }

    /// Retrieve a therapeutic diet order.
    pub fn get_diet_order(env: Env, order_id: u64) -> Result<DietOrder, Error> {
        load_diet_order(&env, order_id).ok_or(Error::DietOrderNotFound)
    }

    /// Retrieve all documented interventions for a care plan.
    pub fn get_interventions(env: Env, care_plan_id: u64) -> Vec<NutritionIntervention> {
        load_interventions(&env, care_plan_id)
    }

    /// Retrieve the food-intake history for a patient.
    pub fn get_food_intake(env: Env, patient_id: Address) -> Vec<FoodIntakeRecord> {
        load_food_intake(&env, &patient_id)
    }

    /// Retrieve the weight-trend history for a patient.
    pub fn get_weight_history(env: Env, patient_id: Address) -> Vec<WeightEntry> {
        load_weight_history(&env, &patient_id)
    }

    /// Retrieve a malnutrition screening result.
    pub fn get_malnutrition_screening(
        env: Env,
        assessment_id: u64,
    ) -> Result<MalnutritionScreening, Error> {
        load_malnutrition_screening(&env, assessment_id).ok_or(Error::AssessmentNotFound)
    }

    /// Retrieve all supplement recommendations for a care plan.
    pub fn get_supplements(env: Env, care_plan_id: u64) -> Vec<SupplementRecommendation> {
        load_supplements(&env, care_plan_id)
    }

    /// Retrieve the latest outcome evaluation for a care plan.
    pub fn get_outcome_evaluation(env: Env, care_plan_id: u64) -> Result<OutcomeEvaluation, Error> {
        load_outcome_evaluation(&env, care_plan_id).ok_or(Error::CarePlanNotFound)
    }
}
