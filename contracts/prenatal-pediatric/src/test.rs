#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _, vec, Address, BytesN, Env, String, Symbol,
};

fn setup() -> (Env, MaternalChildHealthContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MaternalChildHealthContract, ());
    let client = MaternalChildHealthContractClient::new(&env, &contract_id);
    (env, client)
}

fn seed_pregnancy(env: &Env, client: &MaternalChildHealthContractClient<'static>) -> (Address, Address, u64) {
    let patient = Address::generate(env);
    let provider = Address::generate(env);
    let pregnancy_id = client
        .create_pregnancy_record(
            &patient,
            &provider,
            &1_700_000_000,
            &1_725_000_000,
            &2,
            &1,
            &vec![env, Symbol::new(env, "diabetes")],
        );
    (patient, provider, pregnancy_id)
}

#[test]
fn test_create_pregnancy_record() {
    let (env, client) = setup();
    let (patient, provider, id) = seed_pregnancy(&env, &client);

    assert_eq!(id, 1);
    let record = client.get_pregnancy_record(&id);
    assert_eq!(record.patient_id, patient);
    assert_eq!(record.provider_id, provider);
    assert_eq!(record.gravida, 2);
    assert_eq!(record.para, 1);
}

#[test]
fn test_create_pregnancy_invalid_dates_fails() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    let res = client.try_create_pregnancy_record(
        &patient,
        &provider,
        &1_725_000_000,
        &1_700_000_000,
        &1,
        &2,
        &vec![&env],
    );
    assert!(res.is_err());
}

#[test]
fn test_prenatal_visit_screening_and_ultrasound() {
    let (env, client) = setup();
    let (_patient, _provider, pregnancy_id) = seed_pregnancy(&env, &client);

    client
        .record_prenatal_visit(
            &pregnancy_id,
            &1_701_000_000,
            &12,
            &6_850,
            &String::from_str(&env, "118/74"),
            &Some(14),
            &Some(145),
            &BytesN::from_array(&env, &[11u8; 32]),
        );

    client
        .record_prenatal_screening(
            &pregnancy_id,
            &Symbol::new(&env, "quad_screen"),
            &1_701_100_000,
            &BytesN::from_array(&env, &[22u8; 32]),
            &false,
        );

    client
        .record_ultrasound(
            &pregnancy_id,
            &1_701_200_000,
            &20,
            &Some(350),
            &Symbol::new(&env, "normal"),
            &String::from_str(&env, "posterior"),
            &BytesN::from_array(&env, &[33u8; 32]),
        );

    let pregnancy = client.get_pregnancy_record(&pregnancy_id);
    assert_eq!(pregnancy.prenatal_visits.len(), 1);

    let visit = client.get_prenatal_visit(&1);
    assert_eq!(visit.gestational_age_weeks, 12);
    let screening = client.get_prenatal_screening(&1);
    assert_eq!(screening.abnormal, false);
    let ultrasound = client.get_ultrasound(&1);
    assert_eq!(ultrasound.gestational_age, 20);
}

#[test]
fn test_labor_delivery_and_newborn_flow() {
    let (env, client) = setup();
    let (_patient, provider, pregnancy_id) = seed_pregnancy(&env, &client);

    let labor_id = client
        .document_labor_admission(
            &pregnancy_id,
            &1_724_900_000,
            &true,
            &Symbol::new(&env, "intact"),
            &6,
            &90,
        );

    let delivery_id = client
        .record_delivery(
            &labor_id,
            &1_725_000_000,
            &Symbol::new(&env, "vaginal"),
            &Symbol::new(&env, "vertex"),
            &vec![&env, Symbol::new(&env, "none")],
            &350,
            &provider,
        );

    let newborn1 = client
        .record_newborn(
            &delivery_id,
            &1_725_000_100,
            &symbol_short!("female"),
            &3200,
            &50,
            &34,
            &8,
            &9,
            &39,
        );

    let newborn2 = client
        .record_newborn(
            &delivery_id,
            &1_725_000_110,
            &symbol_short!("male"),
            &2900,
            &49,
            &33,
            &8,
            &9,
            &39,
        );

    assert_ne!(newborn1, newborn2);

    let delivery = client.get_delivery_record(&delivery_id);
    assert_eq!(delivery.newborn_ids.len(), 2);

    let newborn = client.get_newborn_record(&newborn1);
    assert_eq!(newborn.birth_weight_grams, 3200);

    let pregnancy = client.get_pregnancy_record(&pregnancy_id);
    assert_eq!(pregnancy.outcome, Some(symbol_short!("delivrd")));
}

#[test]
fn test_newborn_screening_and_missing_newborn() {
    let (env, client) = setup();
    let missing = Address::generate(&env);
    let res = client.try_record_newborn_screening(
        &missing,
        &Symbol::new(&env, "metabolic"),
        &1_725_010_000,
        &Symbol::new(&env, "normal"),
        &false,
    );
    assert!(res.is_err());
}

#[test]
fn test_newborn_screening_success() {
    let (env, client) = setup();
    let (_patient, provider, pregnancy_id) = seed_pregnancy(&env, &client);

    let labor_id = client.document_labor_admission(
        &pregnancy_id,
        &1_724_900_000,
        &true,
        &Symbol::new(&env, "intact"),
        &5,
        &85,
    );
    let delivery_id = client.record_delivery(
        &labor_id,
        &1_725_000_000,
        &Symbol::new(&env, "vaginal"),
        &Symbol::new(&env, "vertex"),
        &vec![&env],
        &275,
        &provider,
    );
    let newborn = client.record_newborn(
        &delivery_id,
        &1_725_000_120,
        &symbol_short!("female"),
        &3100,
        &50,
        &34,
        &8,
        &9,
        &39,
    );

    client.record_newborn_screening(
        &newborn,
        &Symbol::new(&env, "hearing"),
        &1_725_010_500,
        &Symbol::new(&env, "pass"),
        &false,
    );
}

#[test]
fn test_pediatric_growth_milestones_well_child() {
    let (env, client) = setup();
    let patient = Address::generate(&env);

    client
        .track_pediatric_growth(&patient, &1_730_000_000, &12, &980, &7550, &Some(4600), &1720);

    client
        .record_developmental_milestone(
            &patient,
            &1_730_000_100,
            &12,
            &Symbol::new(&env, "motor"),
            &vec![&env, Symbol::new(&env, "walks")],
            &vec![&env],
        );

    client
        .track_well_child_visit(
            &patient,
            &1_730_000_200,
            &12,
            &vec![&env, Symbol::new(&env, "mmr")],
            &true,
            &BytesN::from_array(&env, &[44u8; 32]),
        );

    let growth = client.get_growth_record(&patient, &12);
    assert_eq!(growth.measurements.weight_kg_x100, 980);
}

#[test]
fn test_invalid_inputs_failures() {
    let (env, client) = setup();
    let (_patient, provider, pregnancy_id) = seed_pregnancy(&env, &client);

    let bad_labor = client.try_document_labor_admission(
        &pregnancy_id,
        &1_724_900_000,
        &true,
        &Symbol::new(&env, "intact"),
        &11,
        &80,
    );
    assert!(bad_labor.is_err());

    let labor_id = client.document_labor_admission(
        &pregnancy_id,
        &1_724_900_000,
        &true,
        &Symbol::new(&env, "intact"),
        &5,
        &80,
    );
    let delivery_id = client.record_delivery(
        &labor_id,
        &1_725_000_000,
        &Symbol::new(&env, "c_section"),
        &Symbol::new(&env, "breech"),
        &vec![&env, Symbol::new(&env, "fetal_distress")],
        &650,
        &provider,
    );
    let bad_newborn = client.try_record_newborn(
        &delivery_id,
        &1_725_000_100,
        &symbol_short!("male"),
        &2800,
        &49,
        &33,
        &11,
        &9,
        &39,
    );
    assert!(bad_newborn.is_err());

    let patient = Address::generate(&env);
    let bad_growth = client.try_track_pediatric_growth(
        &patient,
        &1_730_000_000,
        &12,
        &0,
        &7550,
        &None,
        &1720,
    );
    assert!(bad_growth.is_err());
}

#[test]
fn test_calculate_growth_percentiles() {
    let (env, client) = setup();
    let patient = Address::generate(&env);

    let percentiles = client
        .calculate_growth_percentiles(
            &patient,
            &symbol_short!("female"),
            &12,
            &PediatricMeasurements {
                weight_kg_x100: 960,
                height_cm_x100: 7500,
                head_circumference_cm_x100: Some(4550),
                bmi_x100: 1700,
            },
        );

    assert!(percentiles.weight_percentile_x100 >= 0);
    assert!(percentiles.weight_percentile_x100 <= 10_000);

    let bad = client.try_calculate_growth_percentiles(
        &patient,
        &Symbol::new(&env, "other"),
        &12,
        &PediatricMeasurements {
            weight_kg_x100: 960,
            height_cm_x100: 7500,
            head_circumference_cm_x100: None,
            bmi_x100: 1700,
        },
    );
    assert!(bad.is_err());
}

#[test]
fn test_nonexistent_getters_fail() {
    let (env, client) = setup();
    let res1 = client.try_get_pregnancy_record(&999);
    let res_labor = client.try_get_labor_record(&999);
    let res2 = client.try_get_delivery_record(&999);
    let res3 = client.try_get_newborn_record(&Address::generate(&env));
    let res_growth = client.try_get_growth_record(&Address::generate(&env), &12);

    assert!(res1.is_err());
    assert!(res_labor.is_err());
    assert!(res2.is_err());
    assert!(res3.is_err());
    assert!(res_growth.is_err());
}
