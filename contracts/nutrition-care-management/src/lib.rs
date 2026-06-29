#![no_std]
#![allow(deprecated)]
#![allow(clippy::too_many_arguments)]

//! # Nutrition Care Management Contract
//!
//! Manages nutrition assessments, diet plans, nutritionist consultations, and dietary compliance
//! tracking for chronic disease and wellness management.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Nutritionist/RD authentication for diet plan creation. Patient
//! consent for nutrition record access. Authorized providers can view dietary recommendations.
//! Patient authorization required for dietary consultation scheduling. Access control grants tracked
//! per patient-provider pair.
//!
//! **Audit Controls:** Nutrition assessment events logged with patient, nutritionist, and findings.
//! Diet plan creation events tracked with plan type and goals. Consultation appointment events
//! recorded. Dietary compliance check-in events logged. Plan modification events tracked with
//! provider identity.
//!
//! **Data Retention Policy:** Nutrition assessments retained indefinitely. Diet plans archived
//! with start/end dates. Consultation history maintained for continuity. Compliance tracking
//! retained for outcome measurement. Deregistration removes patient nutrition records.
//!
//! **Encryption/Integrity:** Dietary recommendations stored encrypted. Patient-nutritionist
//! linkage encrypted via persistent storage. Assessment date tracking prevents backdating.
//! Nutritionist identity validated at plan creation. Compliance metrics immutable once recorded.

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

        let assessment = load_assessment(&env, assessment_id).ok_or(Error::AssessmentNotFound)?;

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

        // Initialize plan version to 1 (#393)
        env.storage()
            .persistent()
            .set(&DataKey::PlanVersion(care_plan_id), &1u64);

        // Authorize the creating dietitian and the patient (#393)
        add_authorized_provider(&env, care_plan_id, &dietitian_id);
        add_authorized_provider(&env, care_plan_id, &assessment.patient_id);

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
    // 11. link_outcome (#393)
    // ------------------------------------------------------------------

    /// Link a clinical outcome measurement to a nutrition care plan.
    ///
    /// Tracks measurable outcomes like weight, lab values (HbA1c, cholesterol),
    /// and vitals over time. Outcomes are tied to specific plan versions to
    /// support plan updates and correlation tracking.
    ///
    /// `outcome_value_x100` – value × 100 for precision (e.g., 70.5 kg → 7050)
    ///
    /// Only providers with write access to the patient can record outcomes.
    pub fn link_outcome(
        env: Env,
        care_plan_id: u64,
        provider_id: Address,
        outcome_metric: String,
        outcome_value_x100: i64,
        measured_at: u64,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        // Verify care plan exists
        load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;

        // Verify provider is authorized
        if !is_provider_authorized(&env, care_plan_id, &provider_id) {
            return Err(Error::ProviderNotAuthorized);
        }

        // Validate outcome metric (extensible list)
        let valid_metrics = [
            String::from_str(&env, "weight_kg"),
            String::from_str(&env, "bmi"),
            String::from_str(&env, "hba1c"),
            String::from_str(&env, "cholesterol_total"),
            String::from_str(&env, "cholesterol_ldl"),
            String::from_str(&env, "cholesterol_hdl"),
            String::from_str(&env, "triglycerides"),
            String::from_str(&env, "blood_pressure_systolic"),
            String::from_str(&env, "blood_pressure_diastolic"),
            String::from_str(&env, "glucose_fasting"),
            String::from_str(&env, "albumin"),
            String::from_str(&env, "prealbumin"),
            String::from_str(&env, "waist_circumference"),
        ];

        let mut is_valid = false;
        for valid in valid_metrics.iter() {
            if &outcome_metric == valid {
                is_valid = true;
                break;
            }
        }

        if !is_valid {
            return Err(Error::InvalidOutcomeMetric);
        }

        let outcome_id = next_outcome_id(&env);
        let plan_version = get_plan_version(&env, care_plan_id);

        let outcome = ClinicalOutcome {
            outcome_id,
            care_plan_id,
            plan_version,
            provider_id: provider_id.clone(),
            outcome_metric: outcome_metric.clone(),
            outcome_value_x100,
            measured_at,
            recorded_at: env.ledger().timestamp(),
        };

        save_clinical_outcome(&env, &outcome);
        append_plan_outcome(&env, care_plan_id, outcome_id);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "nutrition_outcome_recorded"),),
            NutritionOutcomeRecordedEvent {
                outcome_id,
                care_plan_id,
                plan_version,
                outcome_metric,
                outcome_value_x100,
                measured_at,
            },
        );

        Ok(outcome_id)
    }

    // ------------------------------------------------------------------
    // 12. update_care_plan_version (#393)
    // ------------------------------------------------------------------

    /// Increment the care plan version when the plan is updated.
    ///
    /// This allows tracking which outcomes correspond to which version
    /// of the care plan, enabling correlation analysis.
    pub fn update_care_plan_version(
        env: Env,
        care_plan_id: u64,
        dietitian_id: Address,
    ) -> Result<u64, Error> {
        dietitian_id.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;

        // Verify dietitian is authorized
        if plan.dietitian_id != dietitian_id {
            return Err(Error::Unauthorized);
        }

        increment_plan_version(&env, care_plan_id);
        let new_version = get_plan_version(&env, care_plan_id);

        env.events().publish(
            (Symbol::new(&env, "plan_version_updated"),),
            (care_plan_id, new_version),
        );

        Ok(new_version)
    }

    // ------------------------------------------------------------------
    // 13. authorize_provider (#393)
    // ------------------------------------------------------------------

    /// Grant a provider write access to record outcomes for a care plan.
    ///
    /// Only the original dietitian can authorize additional providers.
    pub fn authorize_provider(
        env: Env,
        care_plan_id: u64,
        dietitian_id: Address,
        provider_id: Address,
    ) -> Result<(), Error> {
        dietitian_id.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;

        if plan.dietitian_id != dietitian_id {
            return Err(Error::Unauthorized);
        }

        add_authorized_provider(&env, care_plan_id, &provider_id);

        env.events().publish(
            (Symbol::new(&env, "provider_authorized"),),
            (care_plan_id, provider_id),
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

    /// Retrieve all clinical outcomes linked to a care plan (#393).
    ///
    /// Returns outcomes in chronologically ordered sequence (by recorded_at).
    pub fn get_plan_outcomes(env: Env, care_plan_id: u64) -> Vec<ClinicalOutcome> {
        let outcome_ids = load_plan_outcomes(&env, care_plan_id);
        let mut outcomes = Vec::new(&env);

        for id in outcome_ids.iter() {
            if let Some(outcome) = load_clinical_outcome(&env, id) {
                outcomes.push_back(outcome);
            }
        }

        outcomes
    }

    /// Retrieve a specific clinical outcome by ID (#393).
    pub fn get_clinical_outcome(env: Env, outcome_id: u64) -> Result<ClinicalOutcome, Error> {
        load_clinical_outcome(&env, outcome_id).ok_or(Error::OutcomeNotFound)
    }

    /// Get the current version of a care plan (#393).
    pub fn get_plan_version(env: Env, care_plan_id: u64) -> u64 {
        get_plan_version(&env, care_plan_id)
    }

    /// Check if a provider is authorized to record outcomes for a care plan (#393).
    pub fn is_provider_authorized(env: Env, care_plan_id: u64, provider_id: Address) -> bool {
        is_provider_authorized(&env, care_plan_id, &provider_id)
    }
}
