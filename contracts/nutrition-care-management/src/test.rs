#![cfg(test)]

use super::*;
use soroban_sdk::{
    symbol_short, testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec,
};

// -----------------------------------------------------------------------
// Test helpers
// -----------------------------------------------------------------------

fn setup() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let patient = Address::generate(&env);
    let dietitian = Address::generate(&env);
    let provider = Address::generate(&env);
    (env, patient, dietitian, provider)
}

fn register(env: &Env) -> NutritionCareContractClient {
    let id = env.register(NutritionCareContract, ());
    NutritionCareContractClient::new(env, &id)
}

/// Create a minimal assessment and return its id.
fn create_assessment(
    env: &Env,
    client: &NutritionCareContractClient,
    patient: &Address,
    dietitian: &Address,
) -> u64 {
    let hash = BytesN::from_array(env, &[1u8; 32]);
    let mut risk_factors = Vec::new(env);
    risk_factors.push_back(String::from_str(env, "obesity"));

    // The generated client auto-unwraps Result<u64, Error> → u64
    client.conduct_nutrition_assessment(
        patient,
        dietitian,
        &1_000_000u64,
        &17550i64,
        &7030i64,
        &2290i64,
        &hash,
        &risk_factors,
    )
}

/// Create an assessment and then a care plan; return (assessment_id, care_plan_id).
fn create_plan(
    env: &Env,
    client: &NutritionCareContractClient,
    patient: &Address,
    dietitian: &Address,
) -> (u64, u64) {
    let assessment_id = create_assessment(env, client, patient, dietitian);

    let mut diagnoses = Vec::new(env);
    diagnoses.push_back(String::from_str(env, "Protein-energy malnutrition"));

    let goal = NutritionGoal {
        goal_description: String::from_str(env, "Achieve ideal body weight"),
        target_date: 2_000_000u64,
        measurement_method: String::from_str(env, "Weekly weigh-in"),
        achieved: false,
    };
    let mut goals = Vec::new(env);
    goals.push_back(goal);

    let mut interventions = Vec::new(env);
    interventions.push_back(String::from_str(env, "High-protein diet counselling"));

    let care_plan_id = client.create_nutrition_care_plan(
        &assessment_id,
        dietitian,
        &diagnoses,
        &goals,
        &interventions,
        &String::from_str(env, "weekly"),
    );

    (assessment_id, care_plan_id)
}

// -----------------------------------------------------------------------
// conduct_nutrition_assessment
// -----------------------------------------------------------------------

#[test]
fn test_conduct_assessment_success() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);

    let hash = BytesN::from_array(&env, &[0u8; 32]);
    let mut risk_factors = Vec::new(&env);
    risk_factors.push_back(String::from_str(&env, "diabetes_t2"));

    let id = client.conduct_nutrition_assessment(
        &patient,
        &dietitian,
        &1_000_000u64,
        &16000i64,
        &6500i64,
        &2539i64,
        &hash,
        &risk_factors,
    );

    assert_eq!(id, 1);
}

#[test]
fn test_conduct_assessment_increments_ids() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);

    let hash = BytesN::from_array(&env, &[2u8; 32]);
    let risk_factors = Vec::new(&env);

    let id1 = client.conduct_nutrition_assessment(
        &patient,
        &dietitian,
        &1_000_000u64,
        &17000i64,
        &7500i64,
        &2595i64,
        &hash,
        &risk_factors,
    );

    let patient2 = Address::generate(&env);
    let id2 = client.conduct_nutrition_assessment(
        &patient2,
        &dietitian,
        &1_100_000u64,
        &16500i64,
        &6000i64,
        &2204i64,
        &hash,
        &risk_factors,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_get_assessment_returns_stored_data() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let id = create_assessment(&env, &client, &patient, &dietitian);

    let stored = client.get_assessment(&id);
    assert_eq!(stored.assessment_id, id);
    assert_eq!(stored.patient_id, patient);
    assert_eq!(stored.dietitian_id, dietitian);
    assert_eq!(stored.height_cm_x100, 17550);
    assert_eq!(stored.weight_kg_x100, 7030);
    assert_eq!(stored.bmi_x100, 2290);
}

#[test]
fn test_get_assessment_not_found() {
    let (env, _, _, _) = setup();
    let client = register(&env);
    let result = client.try_get_assessment(&999);
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// calculate_nutritional_needs
// -----------------------------------------------------------------------

#[test]
fn test_calculate_needs_success() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);

    let considerations = Vec::new(&env);
    let needs = client.calculate_nutritional_needs(
        &assessment_id,
        &symbol_short!("moderate"),
        &125i64,
        &considerations,
    );

    assert!(needs.calories_per_day > 0);
    assert!(needs.protein_grams > 0);
    assert!(needs.carbohydrate_grams > 0);
    assert!(needs.fat_grams > 0);
    assert!(needs.fluid_ml > 0);
}

#[test]
fn test_calculate_needs_invalid_activity_level() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);

    let considerations = Vec::new(&env);
    let result = client.try_calculate_nutritional_needs(
        &assessment_id,
        &symbol_short!("flying"),
        &100i64,
        &considerations,
    );
    assert!(result.is_err());
}

#[test]
fn test_calculate_needs_assessment_not_found() {
    let (env, _, _, _) = setup();
    let client = register(&env);

    let considerations = Vec::new(&env);
    let result = client.try_calculate_nutritional_needs(
        &999,
        &symbol_short!("sedntry"),
        &100i64,
        &considerations,
    );
    assert!(result.is_err());
}

#[test]
fn test_calculate_needs_all_activity_levels() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let considerations = Vec::new(&env);

    let levels: &[(Symbol, &str)] = &[
        (symbol_short!("sedntry"), "sedntry"),
        (symbol_short!("light"), "light"),
        (symbol_short!("moderate"), "moderate"),
        (symbol_short!("active"), "active"),
        (symbol_short!("vactive"), "vactive"),
    ];

    for (sym, desc) in levels {
        let assessment_id = create_assessment(&env, &client, &patient, &dietitian);
        let needs =
            client.calculate_nutritional_needs(&assessment_id, sym, &100i64, &considerations);
        assert!(
            needs.calories_per_day > 0,
            "failed for activity level: {}",
            desc
        );
    }
}

#[test]
fn test_get_nutritional_needs_returns_stored() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);

    let considerations = Vec::new(&env);
    client.calculate_nutritional_needs(
        &assessment_id,
        &symbol_short!("light"),
        &110i64,
        &considerations,
    );

    let stored = client.get_nutritional_needs(&assessment_id);
    assert_eq!(stored.assessment_id, assessment_id);
    assert!(stored.needs.calories_per_day > 0);
}

// -----------------------------------------------------------------------
// create_nutrition_care_plan
// -----------------------------------------------------------------------

#[test]
fn test_create_care_plan_success() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let (_, plan_id) = create_plan(&env, &client, &patient, &dietitian);
    assert_eq!(plan_id, 1);
}

#[test]
fn test_create_care_plan_assessment_not_found() {
    let (env, _, dietitian, _) = setup();
    let client = register(&env);

    let mut diagnoses = Vec::new(&env);
    diagnoses.push_back(String::from_str(&env, "Malnutrition"));
    let goals = Vec::new(&env);
    let interventions = Vec::new(&env);

    let result = client.try_create_nutrition_care_plan(
        &999,
        &dietitian,
        &diagnoses,
        &goals,
        &interventions,
        &String::from_str(&env, "monthly"),
    );
    assert!(result.is_err());
}

#[test]
fn test_get_care_plan_returns_stored_data() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let (assessment_id, care_plan_id) = create_plan(&env, &client, &patient, &dietitian);

    let plan = client.get_care_plan(&care_plan_id);
    assert_eq!(plan.care_plan_id, care_plan_id);
    assert_eq!(plan.assessment_id, assessment_id);
    assert_eq!(plan.dietitian_id, dietitian);
}

// -----------------------------------------------------------------------
// order_therapeutic_diet
// -----------------------------------------------------------------------

#[test]
fn test_order_diet_success() {
    let (env, patient, _, provider) = setup();
    let client = register(&env);

    let order_id = client.order_therapeutic_diet(
        &patient,
        &provider,
        &symbol_short!("diabetic"),
        &Some(symbol_short!("minced")),
        &Some(1500u32),
        &Some(1800u32),
        &Some(String::from_str(&env, "No added sugar")),
    );

    assert_eq!(order_id, 1);
}

#[test]
fn test_order_diet_no_optional_fields() {
    let (env, patient, _, provider) = setup();
    let client = register(&env);

    let order_id = client.order_therapeutic_diet(
        &patient,
        &provider,
        &symbol_short!("regular"),
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(order_id, 1);
}

#[test]
fn test_order_diet_increments_ids() {
    let (env, patient, _, provider) = setup();
    let client = register(&env);

    let id1 = client.order_therapeutic_diet(
        &patient,
        &provider,
        &symbol_short!("cardiac"),
        &None,
        &None,
        &None,
        &None,
    );

    let id2 = client.order_therapeutic_diet(
        &patient,
        &provider,
        &symbol_short!("renal"),
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_get_diet_order_stored_correctly() {
    let (env, patient, _, provider) = setup();
    let client = register(&env);

    let order_id = client.order_therapeutic_diet(
        &patient,
        &provider,
        &symbol_short!("low_sodum"),
        &Some(symbol_short!("soft")),
        &Some(2000u32),
        &Some(2200u32),
        &Some(String::from_str(&env, "Low potassium")),
    );

    let order = client.get_diet_order(&order_id);
    assert_eq!(order.order_id, order_id);
    assert_eq!(order.patient_id, patient);
    assert!(order.active);
    assert_eq!(order.fluid_restriction_ml, Some(2000u32));
    assert_eq!(order.calorie_target, Some(2200u32));
}

#[test]
fn test_get_diet_order_not_found() {
    let (env, _, _, _) = setup();
    let client = register(&env);
    let result = client.try_get_diet_order(&999);
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// document_nutrition_intervention
// -----------------------------------------------------------------------

#[test]
fn test_document_intervention_success() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let (_, care_plan_id) = create_plan(&env, &client, &patient, &dietitian);

    let mut topics = Vec::new(&env);
    topics.push_back(String::from_str(&env, "Carbohydrate counting"));
    topics.push_back(String::from_str(&env, "Portion control"));

    client.document_nutrition_intervention(
        &care_plan_id,
        &1_500_000u64,
        &symbol_short!("education"),
        &topics,
        &45u32,
        &symbol_short!("good"),
    );

    let interventions = client.get_interventions(&care_plan_id);
    assert_eq!(interventions.len(), 1);
    let recorded = interventions.get(0).unwrap();
    assert_eq!(recorded.care_plan_id, care_plan_id);
    assert_eq!(recorded.duration_minutes, 45);
}

#[test]
fn test_document_intervention_multiple_sessions() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let (_, care_plan_id) = create_plan(&env, &client, &patient, &dietitian);

    for _ in 0..3u32 {
        let topics = Vec::new(&env);
        client.document_nutrition_intervention(
            &care_plan_id,
            &1_500_000u64,
            &symbol_short!("counsel"),
            &topics,
            &30u32,
            &symbol_short!("fair"),
        );
    }

    let interventions = client.get_interventions(&care_plan_id);
    assert_eq!(interventions.len(), 3);
}

#[test]
fn test_document_intervention_care_plan_not_found() {
    let (env, _, _, _) = setup();
    let client = register(&env);
    let topics = Vec::new(&env);

    let result = client.try_document_nutrition_intervention(
        &999,
        &1_500_000u64,
        &symbol_short!("education"),
        &topics,
        &30u32,
        &symbol_short!("good"),
    );
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// track_food_intake
// -----------------------------------------------------------------------

#[test]
fn test_track_food_intake_success() {
    let (env, patient, _, _) = setup();
    let client = register(&env);

    let food = FoodItem {
        food_name: String::from_str(&env, "Brown rice"),
        portion_size: String::from_str(&env, "1 cup"),
        calories: 216u32,
        protein_grams: 5u32,
    };
    let mut foods = Vec::new(&env);
    foods.push_back(food);

    client.track_food_intake(
        &patient,
        &1_200_000u64,
        &symbol_short!("lunch"),
        &foods,
        &75u32,
    );

    let records = client.get_food_intake(&patient);
    assert_eq!(records.len(), 1);
    let r = records.get(0).unwrap();
    assert_eq!(r.percentage_consumed, 75);
    assert_eq!(r.foods_consumed.len(), 1);
}

#[test]
fn test_track_food_intake_multiple_meals() {
    let (env, patient, _, _) = setup();
    let client = register(&env);
    let foods = Vec::new(&env);

    for meal in &[
        symbol_short!("breakfst"),
        symbol_short!("lunch"),
        symbol_short!("dinner"),
    ] {
        client.track_food_intake(&patient, &1_200_000u64, meal, &foods, &100u32);
    }

    let records = client.get_food_intake(&patient);
    assert_eq!(records.len(), 3);
}

#[test]
fn test_track_food_intake_empty_foods_list() {
    let (env, patient, _, _) = setup();
    let client = register(&env);
    let foods = Vec::new(&env);

    client.track_food_intake(
        &patient,
        &1_200_000u64,
        &symbol_short!("snack"),
        &foods,
        &0u32,
    );

    let records = client.get_food_intake(&patient);
    assert_eq!(records.len(), 1);
}

// -----------------------------------------------------------------------
// monitor_weight_trend
// -----------------------------------------------------------------------

#[test]
fn test_monitor_weight_trend_success() {
    let (env, patient, _, _) = setup();
    let client = register(&env);

    client.monitor_weight_trend(
        &patient,
        &1_300_000u64,
        &7050i64,
        &symbol_short!("measured"),
    );

    let history = client.get_weight_history(&patient);
    assert_eq!(history.len(), 1);
    let entry = history.get(0).unwrap();
    assert_eq!(entry.weight_kg_x100, 7050);
}

#[test]
fn test_monitor_weight_trend_multiple_entries() {
    let (env, patient, _, _) = setup();
    let client = register(&env);

    let weights: &[(u64, i64)] = &[(1_000_000, 7200), (1_100_000, 7150), (1_200_000, 7100)];

    for (date, w) in weights {
        client.monitor_weight_trend(&patient, date, w, &symbol_short!("measured"));
    }

    let history = client.get_weight_history(&patient);
    assert_eq!(history.len(), 3);
    assert_eq!(history.get(0).unwrap().weight_kg_x100, 7200);
    assert_eq!(history.get(2).unwrap().weight_kg_x100, 7100);
}

#[test]
fn test_monitor_weight_patient_reported() {
    let (env, patient, _, _) = setup();
    let client = register(&env);

    client.monitor_weight_trend(
        &patient,
        &1_400_000u64,
        &8000i64,
        &symbol_short!("pt_rprtd"),
    );

    let history = client.get_weight_history(&patient);
    let entry = history.get(0).unwrap();
    assert_eq!(entry.method, symbol_short!("pt_rprtd"));
}

// -----------------------------------------------------------------------
// assess_malnutrition_risk
// -----------------------------------------------------------------------

#[test]
fn test_assess_malnutrition_risk_must_success() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);

    client.assess_malnutrition_risk(
        &assessment_id,
        &symbol_short!("must"),
        &3u32,
        &symbol_short!("high"),
    );

    let screening = client.get_malnutrition_screening(&assessment_id);
    assert_eq!(screening.assessment_id, assessment_id);
    assert_eq!(screening.score, 3);
}

#[test]
fn test_assess_malnutrition_risk_nrs2002() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);

    client.assess_malnutrition_risk(
        &assessment_id,
        &symbol_short!("nrs2002"),
        &2u32,
        &symbol_short!("medium"),
    );

    let screening = client.get_malnutrition_screening(&assessment_id);
    assert_eq!(screening.score, 2);
}

#[test]
fn test_assess_malnutrition_risk_mna() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);

    client.assess_malnutrition_risk(
        &assessment_id,
        &symbol_short!("mna"),
        &1u32,
        &symbol_short!("low"),
    );

    let screening = client.get_malnutrition_screening(&assessment_id);
    assert_eq!(screening.risk_level, symbol_short!("low"));
}

#[test]
fn test_assess_malnutrition_invalid_tool() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);

    let result = client.try_assess_malnutrition_risk(
        &assessment_id,
        &symbol_short!("unknown"),
        &2u32,
        &symbol_short!("high"),
    );
    assert!(result.is_err());
}

#[test]
fn test_assess_malnutrition_invalid_risk_level() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);

    let result = client.try_assess_malnutrition_risk(
        &assessment_id,
        &symbol_short!("must"),
        &2u32,
        &symbol_short!("extreme"),
    );
    assert!(result.is_err());
}

#[test]
fn test_assess_malnutrition_assessment_not_found() {
    let (env, _, _, _) = setup();
    let client = register(&env);

    let result = client.try_assess_malnutrition_risk(
        &999,
        &symbol_short!("mna"),
        &1u32,
        &symbol_short!("low"),
    );
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// recommend_supplements
// -----------------------------------------------------------------------

#[test]
fn test_recommend_supplements_success() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let (_, care_plan_id) = create_plan(&env, &client, &patient, &dietitian);

    client.recommend_supplements(
        &care_plan_id,
        &dietitian,
        &symbol_short!("vitd"),
        &String::from_str(&env, "1000 IU/day"),
        &String::from_str(&env, "Deficiency confirmed by serum 25-OH-D"),
    );

    let supplements = client.get_supplements(&care_plan_id);
    assert_eq!(supplements.len(), 1);
    let rec = supplements.get(0).unwrap();
    assert_eq!(rec.supplement_type, symbol_short!("vitd"));
}

#[test]
fn test_recommend_multiple_supplements() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let (_, care_plan_id) = create_plan(&env, &client, &patient, &dietitian);

    for sup in &[
        symbol_short!("vitd"),
        symbol_short!("iron"),
        symbol_short!("omega3"),
    ] {
        client.recommend_supplements(
            &care_plan_id,
            &dietitian,
            sup,
            &String::from_str(&env, "daily"),
            &String::from_str(&env, "Clinical indication"),
        );
    }

    let supplements = client.get_supplements(&care_plan_id);
    assert_eq!(supplements.len(), 3);
}

#[test]
fn test_recommend_supplements_care_plan_not_found() {
    let (env, _, dietitian, _) = setup();
    let client = register(&env);

    let result = client.try_recommend_supplements(
        &999,
        &dietitian,
        &symbol_short!("zinc"),
        &String::from_str(&env, "15 mg/day"),
        &String::from_str(&env, "Wound healing support"),
    );
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// evaluate_nutrition_outcomes
// -----------------------------------------------------------------------

#[test]
fn test_evaluate_outcomes_success() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let (_, care_plan_id) = create_plan(&env, &client, &patient, &dietitian);

    let mut lab = Vec::new(&env);
    lab.push_back(String::from_str(&env, "HbA1c improved from 8.5 to 7.2"));

    let mut goals_met = Vec::new(&env);
    goals_met.push_back(String::from_str(&env, "Achieve ideal body weight"));

    client.evaluate_nutrition_outcomes(
        &care_plan_id,
        &2_000_000u64,
        &-200i64,
        &lab,
        &goals_met,
        &true,
    );

    let ev = client.get_outcome_evaluation(&care_plan_id);
    assert_eq!(ev.care_plan_id, care_plan_id);
    assert_eq!(ev.weight_change_kg_x100, -200);
    assert!(ev.continue_care);
    assert_eq!(ev.goals_met.len(), 1);
}

#[test]
fn test_evaluate_outcomes_discontinue_care() {
    let (env, patient, dietitian, _) = setup();
    let client = register(&env);
    let (_, care_plan_id) = create_plan(&env, &client, &patient, &dietitian);

    let lab = Vec::new(&env);
    let goals_met = Vec::new(&env);

    client.evaluate_nutrition_outcomes(
        &care_plan_id,
        &2_000_000u64,
        &0i64,
        &lab,
        &goals_met,
        &false,
    );

    let ev = client.get_outcome_evaluation(&care_plan_id);
    assert!(!ev.continue_care);
}

#[test]
fn test_evaluate_outcomes_care_plan_not_found() {
    let (env, _, _, _) = setup();
    let client = register(&env);
    let lab = Vec::new(&env);
    let goals_met = Vec::new(&env);

    let result =
        client.try_evaluate_nutrition_outcomes(&999, &2_000_000u64, &0i64, &lab, &goals_met, &true);
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// End-to-end workflow test
// -----------------------------------------------------------------------

#[test]
fn test_full_nutrition_care_workflow() {
    let (env, patient, dietitian, provider) = setup();
    let client = register(&env);

    // Step 1: Assessment
    let assessment_id = create_assessment(&env, &client, &patient, &dietitian);
    assert_eq!(assessment_id, 1);

    // Step 2: Calculate needs
    let considerations = Vec::new(&env);
    let needs = client.calculate_nutritional_needs(
        &assessment_id,
        &symbol_short!("moderate"),
        &125i64,
        &considerations,
    );
    assert!(needs.calories_per_day > 0);

    // Step 3: Malnutrition screening
    client.assess_malnutrition_risk(
        &assessment_id,
        &symbol_short!("must"),
        &2u32,
        &symbol_short!("medium"),
    );

    // Step 4: Create care plan
    let (_, care_plan_id) = create_plan(&env, &client, &patient, &dietitian);
    assert_eq!(care_plan_id, 1);

    // Step 5: Order diet
    let order_id = client.order_therapeutic_diet(
        &patient,
        &provider,
        &symbol_short!("diabetic"),
        &None,
        &None,
        &Some(1800u32),
        &None,
    );
    assert_eq!(order_id, 1);

    // Step 6: Document intervention
    let mut topics = Vec::new(&env);
    topics.push_back(String::from_str(&env, "Meal planning"));
    client.document_nutrition_intervention(
        &care_plan_id,
        &1_500_000u64,
        &symbol_short!("mealplan"),
        &topics,
        &60u32,
        &symbol_short!("excllnt"),
    );

    // Step 7: Track food intake
    let food = FoodItem {
        food_name: String::from_str(&env, "Grilled chicken"),
        portion_size: String::from_str(&env, "150 g"),
        calories: 247u32,
        protein_grams: 46u32,
    };
    let mut foods = Vec::new(&env);
    foods.push_back(food);
    client.track_food_intake(
        &patient,
        &1_600_000u64,
        &symbol_short!("dinner"),
        &foods,
        &100u32,
    );

    // Step 8: Record weight
    client.monitor_weight_trend(
        &patient,
        &1_700_000u64,
        &6950i64,
        &symbol_short!("measured"),
    );

    // Step 9: Recommend supplement
    client.recommend_supplements(
        &care_plan_id,
        &dietitian,
        &symbol_short!("vitb12"),
        &String::from_str(&env, "500 mcg/day"),
        &String::from_str(&env, "Deficiency risk due to diet"),
    );

    // Step 10: Evaluate outcomes
    let mut lab = Vec::new(&env);
    lab.push_back(String::from_str(&env, "Blood glucose normalised"));
    let mut goals_met = Vec::new(&env);
    goals_met.push_back(String::from_str(&env, "Achieve ideal body weight"));

    client.evaluate_nutrition_outcomes(
        &care_plan_id,
        &2_000_000u64,
        &-80i64,
        &lab,
        &goals_met,
        &true,
    );

    // Verify final state
    let ev = client.get_outcome_evaluation(&care_plan_id);
    assert_eq!(ev.weight_change_kg_x100, -80);
    assert!(ev.continue_care);

    let history = client.get_weight_history(&patient);
    assert_eq!(history.len(), 1);

    let supplements = client.get_supplements(&care_plan_id);
    assert_eq!(supplements.len(), 1);
}
