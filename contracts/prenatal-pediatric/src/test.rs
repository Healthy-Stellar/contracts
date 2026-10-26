#![cfg(test)]

use super::*;
use provider_registry::{ProviderRegistry, ProviderRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    vec, Address, BytesN, Env, IntoVal, String, Symbol,
};

fn register_test_provider(
    env: &Env,
    registry: &ProviderRegistryClient<'static>,
    admin: &Address,
    provider: &Address,
) {
    let issuer = Address::generate(env);
    registry.register_provider(
        admin,
        provider,
        &String::from_str(env, "Dr. Provider"),
        &String::from_str(env, "General"),
        &String::from_str(env, "LIC-001"),
        &BytesN::from_array(env, &[1u8; 32]),
        &issuer,
        &BytesN::from_array(env, &[2u8; 32]),
        &u64::MAX,
        &BytesN::from_array(env, &[3u8; 32]),
    );
}

fn setup() -> (
    Env,
    MaternalChildHealthContractClient<'static>,
    ProviderRegistryClient<'static>,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let registry_id = env.register(ProviderRegistry, ());
    let registry = ProviderRegistryClient::new(&env, &registry_id);
    registry.initialize(&admin);

    let contract_id = env.register(MaternalChildHealthContract, (&admin, &registry_id));
    let client = MaternalChildHealthContractClient::new(&env, &contract_id);

    (env, client, registry, admin)
}

fn seed_pregnancy(
    env: &Env,
    client: &MaternalChildHealthContractClient<'static>,
    registry: &ProviderRegistryClient<'static>,
    admin: &Address,
) -> (Address, Address, u64) {
    let patient = Address::generate(env);
    let provider = Address::generate(env);
    register_test_provider(env, registry, admin, &provider);
    let pregnancy_id = client.create_pregnancy_record(
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
    let (env, client, registry, admin) = setup();
    let (patient, provider, id) = seed_pregnancy(&env, &client, &registry, &admin);

    assert_eq!(id, 1);
    let record = client.get_pregnancy_record(&patient, &id);
    assert_eq!(record.patient_id, patient);
    assert_eq!(record.provider_id, provider);
    assert_eq!(record.gravida, 2);
    assert_eq!(record.para, 1);
}

#[test]
fn test_create_pregnancy_invalid_dates_fails() {
    let (env, client, registry, admin) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    register_test_provider(&env, &registry, &admin, &provider);

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
fn test_create_pregnancy_record_rejects_unconsented_patient() {
    let (env, client, registry, admin) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    register_test_provider(&env, &registry, &admin, &provider);

    let gravida = 2u32;
    let para = 1u32;
    let risk_factors = vec![&env, Symbol::new(&env, "diabetes")];

    // Only the provider signs; the patient never authorized this record.
    let result = client
        .mock_auths(&[MockAuth {
            address: &provider,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "create_pregnancy_record",
                args: (
                    &patient,
                    &provider,
                    1_700_000_000u64,
                    1_725_000_000u64,
                    gravida,
                    para,
                    &risk_factors,
                )
                    .into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_create_pregnancy_record(
            &patient,
            &provider,
            &1_700_000_000,
            &1_725_000_000,
            &gravida,
            &para,
            &risk_factors,
        );

    assert!(result.is_err());
}

#[test]
fn test_create_pregnancy_record_rejects_unregistered_provider() {
    let (env, client, _registry, _admin) = setup();
    let patient = Address::generate(&env);
    // Never registered in the provider-registry.
    let unregistered_provider = Address::generate(&env);

    let result = client.try_create_pregnancy_record(
        &patient,
        &unregistered_provider,
        &1_700_000_000,
        &1_725_000_000,
        &2,
        &1,
        &vec![&env, Symbol::new(&env, "diabetes")],
    );

    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_prenatal_visit_screening_and_ultrasound() {
    let (env, client, registry, admin) = setup();
    let (_patient, provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);

    client.record_prenatal_visit(
        &pregnancy_id,
        &1_701_000_000,
        &12,
        &6_850,
        &String::from_str(&env, "118/74"),
        &Some(14),
        &Some(145),
        &BytesN::from_array(&env, &[11u8; 32]),
    );

    client.record_prenatal_screening(
        &pregnancy_id,
        &Symbol::new(&env, "quad_screen"),
        &1_701_100_000,
        &BytesN::from_array(&env, &[22u8; 32]),
        &false,
    );

    client.record_ultrasound(
        &pregnancy_id,
        &1_701_200_000,
        &20,
        &Some(350),
        &Symbol::new(&env, "normal"),
        &String::from_str(&env, "posterior"),
        &BytesN::from_array(&env, &[33u8; 32]),
    );

    let pregnancy = client.get_pregnancy_record(&provider, &pregnancy_id);
    assert_eq!(pregnancy.prenatal_visits.len(), 1);

    let visit = client.get_prenatal_visit(&provider, &1);
    assert_eq!(visit.gestational_age_weeks, 12);
    let screening = client.get_prenatal_screening(&provider, &1);
    assert!(!screening.abnormal);
    let ultrasound = client.get_ultrasound(&provider, &1);
    assert_eq!(ultrasound.gestational_age, 20);
}

#[test]
fn test_labor_delivery_and_newborn_flow() {
    let (env, client, registry, admin) = setup();
    let (_patient, provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);

    let labor_id = client.document_labor_admission(
        &pregnancy_id,
        &1_724_900_000,
        &true,
        &Symbol::new(&env, "intact"),
        &6,
        &90,
    );

    let delivery_id = client.record_delivery(
        &labor_id,
        &1_725_000_000,
        &Symbol::new(&env, "vaginal"),
        &Symbol::new(&env, "vertex"),
        &vec![&env, Symbol::new(&env, "none")],
        &350,
        &provider,
    );

    let newborn1 = client.record_newborn(
        &provider,
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

    let newborn2 = client.record_newborn(
        &provider,
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

    let delivery = client.get_delivery_record(&provider, &delivery_id);
    assert_eq!(delivery.newborn_ids.len(), 2);

    let newborn = client.get_newborn_record(&provider, &newborn1);
    assert_eq!(newborn.birth_weight_grams, 3200);

    let pregnancy = client.get_pregnancy_record(&provider, &pregnancy_id);
    assert_eq!(pregnancy.outcome, Some(symbol_short!("delivrd")));
}

#[test]
fn test_record_delivery_rejects_provider_not_on_pregnancy() {
    let (env, client, registry, admin) = setup();
    let (_patient, _provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);

    // A different, otherwise-registered provider who was never assigned to
    // this pregnancy tries to record the delivery.
    let outsider_provider = Address::generate(&env);
    register_test_provider(&env, &registry, &admin, &outsider_provider);

    let labor_id = client.document_labor_admission(
        &pregnancy_id,
        &1_724_900_000,
        &true,
        &Symbol::new(&env, "intact"),
        &6,
        &90,
    );

    let res = client.try_record_delivery(
        &labor_id,
        &1_725_000_000,
        &Symbol::new(&env, "vaginal"),
        &Symbol::new(&env, "vertex"),
        &vec![&env, Symbol::new(&env, "none")],
        &350,
        &outsider_provider,
    );

    assert_eq!(res, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_newborn_screening_and_missing_newborn() {
    let (env, client, _registry, _admin) = setup();
    let provider = Address::generate(&env);
    let missing = Address::generate(&env);
    let res = client.try_record_newborn_screening(
        &provider,
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
    let (env, client, registry, admin) = setup();
    let (_patient, provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);

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
        &provider,
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
        &provider,
        &newborn,
        &Symbol::new(&env, "hearing"),
        &1_725_010_500,
        &Symbol::new(&env, "pass"),
        &false,
    );
}

#[test]
fn test_pediatric_growth_milestones_well_child() {
    let (env, client, registry, admin) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    register_test_provider(&env, &registry, &admin, &provider);

    client.track_pediatric_growth(
        &provider,
        &patient,
        &1_730_000_000,
        &12,
        &980,
        &7550,
        &Some(4600),
        &1720,
    );

    client.record_developmental_milestone(
        &provider,
        &patient,
        &1_730_000_100,
        &12,
        &Symbol::new(&env, "motor"),
        &vec![&env, Symbol::new(&env, "walks")],
        &vec![&env],
    );

    client.track_well_child_visit(
        &provider,
        &patient,
        &1_730_000_200,
        &12,
        &vec![&env, Symbol::new(&env, "mmr")],
        &true,
        &BytesN::from_array(&env, &[44u8; 32]),
    );

    let growth = client.get_growth_record(&patient, &patient, &12);
    assert_eq!(growth.measurements.weight_kg_x100, 980);
}

#[test]
fn test_invalid_inputs_failures() {
    let (env, client, registry, admin) = setup();
    let (_patient, provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);

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
        &provider,
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
        &provider,
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
    let (env, client, _registry, _admin) = setup();
    let patient = Address::generate(&env);

    let percentiles = client.calculate_growth_percentiles(
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
#[should_panic]
fn test_prenatal_visit_requires_provider_auth() {
    let (env, client, registry, admin) = setup();
    let (_patient, _provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);

    env.set_auths(&[]);
    client.record_prenatal_visit(
        &pregnancy_id,
        &1_701_000_000,
        &12,
        &6_850,
        &String::from_str(&env, "118/74"),
        &Some(14),
        &Some(145),
        &BytesN::from_array(&env, &[11u8; 32]),
    );
}

#[test]
#[should_panic]
fn test_document_labor_admission_requires_provider_auth() {
    let (env, client, registry, admin) = setup();
    let (_patient, _provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);

    env.set_auths(&[]);
    client.document_labor_admission(
        &pregnancy_id,
        &1_724_900_000,
        &true,
        &Symbol::new(&env, "intact"),
        &5,
        &80,
    );
}

#[test]
fn test_record_newborn_rejects_mismatched_provider() {
    let (env, client, registry, admin) = setup();
    let (_patient, provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);
    let impostor = Address::generate(&env);

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
        &Symbol::new(&env, "vaginal"),
        &Symbol::new(&env, "vertex"),
        &vec![&env],
        &300,
        &provider,
    );

    let res = client.try_record_newborn(
        &impostor,
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
    assert!(res.is_err());
}

#[test]
#[should_panic]
fn test_track_pediatric_growth_requires_provider_auth() {
    let env = Env::default();
    let contract_id = env.register(MaternalChildHealthContract, ());
    let client = MaternalChildHealthContractClient::new(&env, &contract_id);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    client.track_pediatric_growth(
        &provider,
        &patient,
        &1_730_000_000,
        &12,
        &980,
        &7550,
        &Some(4600),
        &1720,
    );
}

#[test]
fn test_nonexistent_getters_fail() {
    let (env, client, _registry, _admin) = setup();
    let requester = Address::generate(&env);
    let res1 = client.try_get_pregnancy_record(&requester, &999);
    let res_labor = client.try_get_labor_record(&requester, &999);
    let res2 = client.try_get_delivery_record(&requester, &999);
    let res3 = client.try_get_newborn_record(&requester, &Address::generate(&env));
    let res_growth = client.try_get_growth_record(&requester, &Address::generate(&env), &12);

    assert!(res1.is_err());
    assert!(res_labor.is_err());
    assert!(res2.is_err());
    assert!(res3.is_err());
    assert!(res_growth.is_err());
}

// -----------------------------------------------------------------------
// #885 — record getters enforce access control
// -----------------------------------------------------------------------

/// Seeds a pregnancy with one visit, screening, ultrasound, labor, delivery
/// and newborn. Returns (patient, provider, pregnancy_id, labor_id,
/// delivery_id, newborn_id).
fn seed_full_pregnancy(
    env: &Env,
    client: &MaternalChildHealthContractClient<'static>,
    registry: &ProviderRegistryClient<'static>,
    admin: &Address,
) -> (Address, Address, u64, u64, u64, Address) {
    let (patient, provider, pregnancy_id) = seed_pregnancy(env, client, registry, admin);

    client.record_prenatal_visit(
        &pregnancy_id,
        &1_701_000_000,
        &12,
        &6_850,
        &String::from_str(env, "118/74"),
        &Some(14),
        &Some(145),
        &BytesN::from_array(env, &[11u8; 32]),
    );
    client.record_prenatal_screening(
        &pregnancy_id,
        &Symbol::new(env, "quad_screen"),
        &1_701_100_000,
        &BytesN::from_array(env, &[22u8; 32]),
        &false,
    );
    client.record_ultrasound(
        &pregnancy_id,
        &1_701_200_000,
        &20,
        &Some(350),
        &Symbol::new(env, "normal"),
        &String::from_str(env, "posterior"),
        &BytesN::from_array(env, &[33u8; 32]),
    );
    let labor_id = client.document_labor_admission(
        &pregnancy_id,
        &1_724_900_000,
        &true,
        &Symbol::new(env, "intact"),
        &6,
        &90,
    );
    let delivery_id = client.record_delivery(
        &labor_id,
        &1_725_000_000,
        &Symbol::new(env, "vaginal"),
        &Symbol::new(env, "vertex"),
        &vec![env],
        &300,
        &provider,
    );
    let newborn = client.record_newborn(
        &provider,
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
    (
        patient,
        provider,
        pregnancy_id,
        labor_id,
        delivery_id,
        newborn,
    )
}

#[test]
fn test_getters_reject_unrelated_requester() {
    let (env, client, registry, admin) = setup();
    let (_patient, _provider, pregnancy_id, labor_id, delivery_id, newborn) =
        seed_full_pregnancy(&env, &client, &registry, &admin);

    // Even a credentialed provider not on this pregnancy is rejected.
    let outsider = Address::generate(&env);
    register_test_provider(&env, &registry, &admin, &outsider);

    let denied = Err(Ok(Error::Unauthorized));
    assert_eq!(
        client
            .try_get_pregnancy_record(&outsider, &pregnancy_id)
            .map(|_| ()),
        denied
    );
    assert_eq!(
        client.try_get_prenatal_visit(&outsider, &1).map(|_| ()),
        denied
    );
    assert_eq!(
        client.try_get_prenatal_screening(&outsider, &1).map(|_| ()),
        denied
    );
    assert_eq!(client.try_get_ultrasound(&outsider, &1).map(|_| ()), denied);
    assert_eq!(
        client
            .try_get_labor_record(&outsider, &labor_id)
            .map(|_| ()),
        denied
    );
    assert_eq!(
        client
            .try_get_delivery_record(&outsider, &delivery_id)
            .map(|_| ()),
        denied
    );
    assert_eq!(
        client
            .try_get_newborn_record(&outsider, &newborn)
            .map(|_| ()),
        denied
    );
}

#[test]
fn test_getters_allow_patient_and_provider() {
    let (env, client, registry, admin) = setup();
    let (patient, provider, pregnancy_id, labor_id, delivery_id, newborn) =
        seed_full_pregnancy(&env, &client, &registry, &admin);

    for reader in [&patient, &provider] {
        assert_eq!(
            client
                .get_pregnancy_record(reader, &pregnancy_id)
                .pregnancy_id,
            pregnancy_id
        );
        assert_eq!(
            client.get_prenatal_visit(reader, &1).pregnancy_id,
            pregnancy_id
        );
        assert_eq!(
            client.get_prenatal_screening(reader, &1).pregnancy_id,
            pregnancy_id
        );
        assert_eq!(client.get_ultrasound(reader, &1).pregnancy_id, pregnancy_id);
        assert_eq!(
            client.get_labor_record(reader, &labor_id).labor_id,
            labor_id
        );
        assert_eq!(
            client.get_delivery_record(reader, &delivery_id).delivery_id,
            delivery_id
        );
        assert_eq!(
            client.get_newborn_record(reader, &newborn).delivery_id,
            delivery_id
        );
    }
}

#[test]
fn test_family_member_access_grant_and_revoke() {
    let (env, client, registry, admin) = setup();
    let (patient, _provider, pregnancy_id, _labor_id, _delivery_id, newborn) =
        seed_full_pregnancy(&env, &client, &registry, &admin);
    let family = Address::generate(&env);

    assert!(client.try_get_newborn_record(&family, &newborn).is_err());

    client.grant_record_access(&patient, &family);
    assert!(client.has_record_access(&patient, &family));
    client.get_pregnancy_record(&family, &pregnancy_id);
    client.get_newborn_record(&family, &newborn);

    client.revoke_record_access(&patient, &family);
    assert!(!client.has_record_access(&patient, &family));
    assert_eq!(
        client.try_get_pregnancy_record(&family, &pregnancy_id),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn test_growth_record_access_control() {
    let (env, client, registry, admin) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    register_test_provider(&env, &registry, &admin, &provider);

    client.track_pediatric_growth(
        &provider,
        &patient,
        &1_730_000_000,
        &12,
        &980,
        &7550,
        &Some(4600),
        &1720,
    );

    assert_eq!(
        client
            .get_growth_record(&provider, &patient, &12)
            .provider_id,
        provider
    );

    let outsider = Address::generate(&env);
    assert_eq!(
        client
            .try_get_growth_record(&outsider, &patient, &12)
            .map(|_| ()),
        Err(Ok(Error::Unauthorized))
    );

    client.grant_record_access(&patient, &outsider);
    client.get_growth_record(&outsider, &patient, &12);
}

#[test]
#[should_panic]
fn test_getter_requires_requester_auth() {
    let (env, client, registry, admin) = setup();
    let (patient, _provider, pregnancy_id) = seed_pregnancy(&env, &client, &registry, &admin);

    env.set_auths(&[]);
    client.get_pregnancy_record(&patient, &pregnancy_id);
}

#[test]
#[should_panic]
fn test_grant_record_access_requires_patient_auth() {
    let (env, client, _registry, _admin) = setup();
    let patient = Address::generate(&env);

    env.set_auths(&[]);
    client.grant_record_access(&patient, &Address::generate(&env));
}

// -----------------------------------------------------------------------
// #886 — record_newborn_screening validates the provider
// -----------------------------------------------------------------------

#[test]
fn test_newborn_screening_rejects_unregistered_provider() {
    let (env, client, registry, admin) = setup();
    let (_patient, _provider, _pregnancy_id, _labor_id, _delivery_id, newborn) =
        seed_full_pregnancy(&env, &client, &registry, &admin);
    let impostor = Address::generate(&env);

    let res = client.try_record_newborn_screening(
        &impostor,
        &newborn,
        &Symbol::new(&env, "hearing"),
        &1_725_010_500,
        &Symbol::new(&env, "fail"),
        &true,
    );
    assert_eq!(res, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_newborn_screening_allows_registered_pediatric_provider() {
    let (env, client, registry, admin) = setup();
    let (_patient, _provider, _pregnancy_id, _labor_id, _delivery_id, newborn) =
        seed_full_pregnancy(&env, &client, &registry, &admin);
    let pediatrician = Address::generate(&env);
    register_test_provider(&env, &registry, &admin, &pediatrician);

    client.record_newborn_screening(
        &pediatrician,
        &newborn,
        &Symbol::new(&env, "hearing"),
        &1_725_010_500,
        &Symbol::new(&env, "pass"),
        &false,
    );
}
