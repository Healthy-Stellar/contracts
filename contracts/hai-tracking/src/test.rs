#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Symbol,
};

fn setup() -> (Env, HAITrackingContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| {
        l.timestamp = 1_800_000_000;
        l.sequence_number = 100;
    });

    let contract_id = env.register(HAITrackingContract, ());
    let client = HAITrackingContractClient::new(&env, &contract_id);
    (env, client)
}

fn report_case(
    env: &Env,
    client: &HAITrackingContractClient<'static>,
    patient: &Address,
    facility: &Address,
    infection_type: &str,
    onset_date: u64,
    location: &str,
    reporter: &Address,
) -> u64 {
    client.report_infection(
        patient,
        facility,
        &Symbol::new(env, infection_type),
        &onset_date,
        &String::from_str(env, location),
        &false,
        &None,
        reporter,
    )
}

#[test]
fn test_report_infection_success() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);

    let id = report_case(
        &env,
        &client,
        &patient,
        &facility,
        "clabsi",
        1_799_900_000,
        "ICU",
        &reporter,
    );

    assert_eq!(id, 1);
    let case = client.get_infection_case(&id);
    assert_eq!(case.facility_id, facility);
    assert_eq!(case.location, String::from_str(&env, "ICU"));
}

#[test]
fn test_report_infection_invalid_type_fails() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);

    let res = client.try_report_infection(
        &patient,
        &facility,
        &Symbol::new(&env, "unknown"),
        &1_799_900_000,
        &String::from_str(&env, "Ward A"),
        &false,
        &None,
        &reporter,
    );

    assert!(res.is_err());
}

#[test]
fn test_record_organism_and_mdr_detection() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);

    let infection_id = report_case(
        &env,
        &client,
        &patient,
        &facility,
        "mrsa",
        1_799_900_000,
        "Ward A",
        &reporter,
    );

    client.record_organism(
        &infection_id,
        &String::from_str(&env, "staph_aureus"),
        &Symbol::new(&env, "blood"),
        &1_799_900_100,
        &BytesN::from_array(&env, &[1u8; 32]),
    );

    client.record_antibiotic_susceptibility(
        &infection_id,
        &String::from_str(&env, "staph_aureus"),
        &String::from_str(&env, "drug_a"),
        &Symbol::new(&env, "resistant"),
        &Some(150),
    );
    client.record_antibiotic_susceptibility(
        &infection_id,
        &String::from_str(&env, "staph_aureus"),
        &String::from_str(&env, "drug_b"),
        &Symbol::new(&env, "resistant"),
        &Some(200),
    );
    client.record_antibiotic_susceptibility(
        &infection_id,
        &String::from_str(&env, "staph_aureus"),
        &String::from_str(&env, "drug_c"),
        &Symbol::new(&env, "resistant"),
        &Some(250),
    );

    let case = client.get_infection_case(&infection_id);
    let org = case.organisms.get(0).unwrap();
    assert!(org.is_multidrug_resistant);
    assert_eq!(org.susceptibilities.len(), 3);
}

#[test]
fn test_record_antibiotic_invalid_value_fails() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);

    let infection_id = report_case(
        &env,
        &client,
        &patient,
        &facility,
        "mrsa",
        1_799_900_000,
        "Ward A",
        &reporter,
    );

    client.record_organism(
        &infection_id,
        &String::from_str(&env, "staph_aureus"),
        &Symbol::new(&env, "blood"),
        &1_799_900_100,
        &BytesN::from_array(&env, &[2u8; 32]),
    );

    let res = client.try_record_antibiotic_susceptibility(
        &infection_id,
        &String::from_str(&env, "staph_aureus"),
        &String::from_str(&env, "drug_x"),
        &Symbol::new(&env, "invalid"),
        &None,
    );
    assert!(res.is_err());
}

#[test]
fn test_outbreak_detection_success() {
    let (env, client) = setup();
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);

    for _ in 0..4 {
        let patient = Address::generate(&env);
        report_case(
            &env,
            &client,
            &patient,
            &facility,
            "mrsa",
            1_799_950_000,
            "Ward A",
            &reporter,
        );
    }

    let outbreak_id = client
        .identify_outbreak_cluster(
            &Symbol::new(&env, "mrsa"),
            &facility,
            &String::from_str(&env, "Ward A"),
            &30,
            &3,
        )
        .unwrap();

    assert_eq!(outbreak_id, 1);

    let outbreaks = client.get_active_outbreaks(&facility);
    assert_eq!(outbreaks.len(), 1);
    assert_eq!(outbreaks.get(0).unwrap().case_count, 4);
}

#[test]
fn test_outbreak_detection_below_threshold() {
    let (env, client) = setup();
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);
    let patient = Address::generate(&env);

    report_case(
        &env,
        &client,
        &patient,
        &facility,
        "mrsa",
        1_799_950_000,
        "Ward A",
        &reporter,
    );

    let result = client.identify_outbreak_cluster(
        &Symbol::new(&env, "mrsa"),
        &facility,
        &String::from_str(&env, "Ward A"),
        &30,
        &3,
    );

    assert_eq!(result, None);
}

#[test]
fn test_initiate_outbreak_investigation_not_found() {
    let (env, client) = setup();
    let investigator = Address::generate(&env);

    let res = client.try_initiate_outbreak_investigation(
        &77,
        &investigator,
        &String::from_str(&env, "protocol-v1"),
    );

    assert!(res.is_err());
}

#[test]
fn test_isolation_and_active_lookup() {
    let (env, client) = setup();
    let patient = Address::generate(&env);

    let indicated_by = Address::generate(&env);
    let id = client.track_isolation_precaution(
        &patient,
        &Symbol::new(&env, "contact"),
        &1_799_990_000,
        &String::from_str(&env, "MRSA colonization"),
        &String::from_str(&env, "3 negative cultures"),
        &indicated_by,
    );
    assert_eq!(id, 1);

    let active = client.get_active_isolations(&patient);
    assert_eq!(active.len(), 1);
    assert_eq!(
        active.get(0).unwrap().precaution_type,
        Symbol::new(&env, "contact")
    );
}

#[test]
fn test_isolation_invalid_type_fails() {
    let (env, client) = setup();
    let patient = Address::generate(&env);

    let indicated_by = Address::generate(&env);
    let res = client.try_track_isolation_precaution(
        &patient,
        &Symbol::new(&env, "other"),
        &1_799_990_000,
        &String::from_str(&env, "Unknown"),
        &String::from_str(&env, "None"),
        &indicated_by,
    );

    assert!(res.is_err());
}

#[test]
fn test_hand_hygiene_validation() {
    let (env, client) = setup();
    let facility = Address::generate(&env);
    let observer = Address::generate(&env);

    let res = client.try_track_hand_hygiene_compliance(
        &facility,
        &String::from_str(&env, "ICU"),
        &1_799_990_000,
        &10,
        &11,
        &observer,
    );

    assert!(res.is_err());
}

#[test]
fn test_alert_infection_control_team() {
    let (env, client) = setup();
    let facility = Address::generate(&env);

    client.alert_infection_control_team(
        &Symbol::new(&env, "critical"),
        &facility,
        &String::from_str(&env, "Suspected outbreak in ICU"),
        &Symbol::new(&env, "high"),
    );
}

#[test]
fn test_alert_infection_control_team_invalid_priority_fails() {
    let (env, client) = setup();
    let facility = Address::generate(&env);

    let res = client.try_alert_infection_control_team(
        &Symbol::new(&env, "critical"),
        &facility,
        &String::from_str(&env, "Suspected outbreak in ICU"),
        &Symbol::new(&env, "urgent"),
    );

    assert!(res.is_err());
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_report_infection_requires_auth() {
    let env = Env::default();
    let contract_id = env.register(HAITrackingContract, ());
    let client = HAITrackingContractClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.report_infection(
        &patient,
        &facility,
        &Symbol::new(&env, "clabsi"),
        &1_799_900_000,
        &String::from_str(&env, "ICU"),
        &false,
        &None,
        &reporter,
    );
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_identify_outbreak_cluster_requires_auth() {
    let env = Env::default();
    let contract_id = env.register(HAITrackingContract, ());
    let client = HAITrackingContractClient::new(&env, &contract_id);

    let facility = Address::generate(&env);

    client.identify_outbreak_cluster(
        &Symbol::new(&env, "mrsa"),
        &facility,
        &String::from_str(&env, "Ward A"),
        &30,
        &3,
    );
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_track_isolation_precaution_requires_auth() {
    let env = Env::default();
    let contract_id = env.register(HAITrackingContract, ());
    let client = HAITrackingContractClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let indicated_by = Address::generate(&env);

    client.track_isolation_precaution(
        &patient,
        &Symbol::new(&env, "contact"),
        &1_799_990_000,
        &String::from_str(&env, "MRSA colonization"),
        &String::from_str(&env, "3 negative cultures"),
        &indicated_by,
    );
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_record_patient_days_requires_auth() {
    let env = Env::default();
    let contract_id = env.register(HAITrackingContract, ());
    let client = HAITrackingContractClient::new(&env, &contract_id);

    let recorder = Address::generate(&env);

    client.record_patient_days(&1, &100, &recorder);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_configure_ward_rate_requires_auth() {
    let env = Env::default();
    let contract_id = env.register(HAITrackingContract, ());
    let client = HAITrackingContractClient::new(&env, &contract_id);

    let facility = Address::generate(&env);

    client.configure_ward_rate(
        &facility,
        &String::from_str(&env, "Ward A"),
        &Symbol::new(&env, "mrsa"),
        &7,
        &200,
        &100,
    );
}

#[test]
fn test_calculate_infection_rate() {
    let (env, client) = setup();
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);
    let unit = String::from_str(&env, "ICU");

    for _ in 0..2 {
        let patient = Address::generate(&env);
        report_case(
            &env,
            &client,
            &patient,
            &facility,
            "clabsi",
            1_799_900_000,
            "ICU",
            &reporter,
        );
    }

    client.record_census(
        &facility,
        &unit,
        &1_799_900_000,
        &10_000,
        &500,
        &200,
        &100,
        &facility,
    );
    client.record_census(
        &facility,
        &String::from_str(&env, "Ward B"),
        &1_799_900_000,
        &5_000,
        &100,
        &50,
        &25,
        &facility,
    );
    client.configure_ward_rate(
        &facility,
        &unit,
        &Symbol::new(&env, "clabsi"),
        &30,
        &200,
        &500,
    );

    let ward_rate = client.calculate_infection_rate(
        &facility,
        &Symbol::new(&env, "clabsi"),
        &1_799_000_000,
        &1_800_000_000,
        &Some(unit.clone()),
    );
    assert_eq!(ward_rate.numerator, 2);
    assert_eq!(ward_rate.denominator, 500);
    assert_eq!(ward_rate.rate_per_1000_days_x100, 400);
    assert_eq!(ward_rate.sir_x100, Some(80));

    let facility_rate = client.calculate_infection_rate(
        &facility,
        &Symbol::new(&env, "clabsi"),
        &1_799_000_000,
        &1_800_000_000,
        &None,
    );
    assert_eq!(facility_rate.numerator, 2);
    assert_eq!(facility_rate.denominator, 600);
    assert_eq!(facility_rate.sir_x100, None);
}

#[test]
fn test_record_patient_days_only_facility_can_update_case() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);
    let infection_id = report_case(
        &env,
        &client,
        &patient,
        &facility,
        "mrsa",
        1_799_900_000,
        "Ward A",
        &reporter,
    );

    let result = client.try_record_patient_days(&infection_id, &100, &reporter);
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));

    client.record_patient_days(&infection_id, &100, &facility);
    assert_eq!(client.get_infection_case(&infection_id).patient_days, Some(100));
}

#[test]
fn test_record_census_rejects_non_facility_reporter() {
    let (env, client) = setup();
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);

    let result = client.try_record_census(
        &facility,
        &String::from_str(&env, "ICU"),
        &1_799_900_000,
        &100,
        &50,
        &20,
        &10,
        &reporter,
    );
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));
}
