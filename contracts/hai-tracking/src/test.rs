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
    assert_eq!(org.is_multidrug_resistant, true);
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

    let id = client
        .track_isolation_precaution(
            &patient,
            &Symbol::new(&env, "contact"),
            &1_799_990_000,
            &String::from_str(&env, "MRSA colonization"),
            &String::from_str(&env, "3 negative cultures"),
        );
    assert_eq!(id, 1);

    let active = client.get_active_isolations(&patient);
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap().precaution_type, Symbol::new(&env, "contact"));
}

#[test]
fn test_isolation_invalid_type_fails() {
    let (env, client) = setup();
    let patient = Address::generate(&env);

    let res = client.try_track_isolation_precaution(
        &patient,
        &Symbol::new(&env, "other"),
        &1_799_990_000,
        &String::from_str(&env, "Unknown"),
        &String::from_str(&env, "None"),
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
fn test_calculate_infection_rate() {
    let (env, client) = setup();
    let facility = Address::generate(&env);
    let reporter = Address::generate(&env);

    for _ in 0..2 {
        let patient = Address::generate(&env);
        report_case(
            &env,
            &client,
            &patient,
            &facility,
            "cauti",
            1_799_960_000,
            "ICU",
            &reporter,
        );
    }

    let rate = client.calculate_infection_rate(
        &facility,
        &Symbol::new(&env, "cauti"),
        &1_799_900_000,
        &1_800_000_000,
        &Some(String::from_str(&env, "ICU")),
    );

    assert_eq!(rate.numerator, 2);
    assert_eq!(rate.denominator, 1000);
    assert_eq!(rate.rate_per_1000_days_x100, 200);
}

#[test]
fn test_reporting_stewardship_and_alert_priority_validation() {
    let (env, client) = setup();
    let facility = Address::generate(&env);

    let report_id = client.report_to_nhsn(
        &facility,
        &202601,
        &BytesN::from_array(&env, &[5u8; 32]),
        &BytesN::from_array(&env, &[6u8; 32]),
    );
    assert_eq!(report_id, 1);

    client.track_antibiotic_stewardship(
        &facility,
        &String::from_str(&env, "vancomycin"),
        &150,
        &300,
        &202601,
    );

    let ok = client.alert_infection_control_team(
        &Symbol::new(&env, "outbreak"),
        &facility,
        &String::from_str(&env, "Cluster threshold exceeded"),
        &Symbol::new(&env, "high"),
    );
    assert_eq!(ok, ());

    let bad_priority = client.try_alert_infection_control_team(
        &Symbol::new(&env, "outbreak"),
        &facility,
        &String::from_str(&env, "Cluster threshold exceeded"),
        &Symbol::new(&env, "urgent"),
    );
    assert!(bad_priority.is_err());

    let bad_dot = client.try_track_antibiotic_stewardship(
        &facility,
        &String::from_str(&env, "cefepime"),
        &10,
        &0,
        &202601,
    );
    assert!(bad_dot.is_err());
}
