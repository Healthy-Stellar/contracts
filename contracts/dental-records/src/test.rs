//! Tests for the dental records contract.

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, BytesN, Env, String, Symbol, Vec,
};

fn setup() -> (Env, DentalRecordsContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    let contract_id = env.register(DentalRecordsContract, ());
    let client = DentalRecordsContractClient::new(&env, &contract_id);
    (env, client)
}

fn empty_procedures(env: &Env) -> Vec<PlannedProcedure> {
    Vec::new(env)
}

#[test]
fn test_create_dental_chart() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);

    let chart_id = client.create_dental_chart(
        &patient,
        &dentist,
        &1_700_000_000,
        &Symbol::new(&env, "fdi"),
    );

    assert_eq!(chart_id, 1);
}

#[test]
fn test_record_tooth_condition() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);

    let chart_id = client.create_dental_chart(
        &patient,
        &dentist,
        &1_700_000_000,
        &Symbol::new(&env, "fdi"),
    );

    client.record_tooth_condition(
        &chart_id,
        &String::from_str(&env, "11"),
        &ToothSurface::Occlusal,
        &ToothConditionKind::Caries,
        &None,
    );
}

fn create_chart(env: &Env, client: &DentalRecordsContractClient, notation: &str) -> u64 {
    let patient = Address::generate(env);
    let dentist = Address::generate(env);
    client.create_dental_chart(
        &patient,
        &dentist,
        &1_700_000_000,
        &Symbol::new(env, notation),
    )
}

#[test]
fn test_multi_surface_conditions_do_not_overwrite() {
    let (env, client) = setup();
    let chart_id = create_chart(&env, &client, "universal");
    let tooth = String::from_str(&env, "19");

    client.record_tooth_condition(
        &chart_id,
        &tooth,
        &ToothSurface::Mesial,
        &ToothConditionKind::Caries,
        &None,
    );
    client.record_tooth_condition(
        &chart_id,
        &tooth,
        &ToothSurface::Occlusal,
        &ToothConditionKind::Filling,
        &Some(String::from_str(&env, "amalgam")),
    );

    let mesial = client
        .get_tooth_condition(&chart_id, &tooth, &ToothSurface::Mesial)
        .unwrap();
    let occlusal = client
        .get_tooth_condition(&chart_id, &tooth, &ToothSurface::Occlusal)
        .unwrap();

    assert_eq!(mesial.condition, ToothConditionKind::Caries);
    assert_eq!(mesial.surface, ToothSurface::Mesial);
    assert_eq!(occlusal.condition, ToothConditionKind::Filling);
    assert_eq!(occlusal.surface, ToothSurface::Occlusal);
    assert_eq!(occlusal.recorded_at, 1_700_000_000);
    assert!(client
        .get_tooth_condition(&chart_id, &tooth, &ToothSurface::Distal)
        .is_none());
}

#[test]
fn test_record_tooth_condition_emits_event() {
    let (env, client) = setup();
    let chart_id = create_chart(&env, &client, "fdi");
    let before = env.events().all().len();

    client.record_tooth_condition(
        &chart_id,
        &String::from_str(&env, "36"),
        &ToothSurface::WholeTooth,
        &ToothConditionKind::Crown,
        &None,
    );

    assert_eq!(env.events().all().len(), before + 1);
}

#[test]
fn test_invalid_tooth_numbers_rejected() {
    let (env, client) = setup();
    let universal = create_chart(&env, &client, "universal");
    let fdi = create_chart(&env, &client, "fdi");
    let palmer = create_chart(&env, &client, "palmer");

    let cases = [
        (universal, "99"),
        (universal, ""),
        (universal, "0"),
        (universal, "33"),
        (universal, "U"),
        (fdi, "19"),
        (fdi, "49"),
        (fdi, "56"),
        (fdi, "8"),
        (palmer, "UR9"),
        (palmer, "XX1"),
        (palmer, "URF"),
    ];
    for (chart_id, tooth) in cases {
        let result = client.try_record_tooth_condition(
            &chart_id,
            &String::from_str(&env, tooth),
            &ToothSurface::Occlusal,
            &ToothConditionKind::Caries,
            &None,
        );
        assert_eq!(result, Err(Ok(Error::InvalidToothNumber)), "tooth {}", tooth);
    }

    let valid = [
        (universal, "1"),
        (universal, "32"),
        (universal, "A"),
        (universal, "T"),
        (fdi, "11"),
        (fdi, "48"),
        (fdi, "55"),
        (fdi, "85"),
        (palmer, "UR1"),
        (palmer, "LL8"),
        (palmer, "ULE"),
    ];
    for (chart_id, tooth) in valid {
        client.record_tooth_condition(
            &chart_id,
            &String::from_str(&env, tooth),
            &ToothSurface::Occlusal,
            &ToothConditionKind::Caries,
            &None,
        );
    }
}

#[test]
fn test_periodontal_assessment_rejects_invalid_tooth() {
    let (env, client) = setup();
    let chart_id = create_chart(&env, &client, "fdi");

    let result = client.try_record_periodontal_assessment(
        &chart_id,
        &String::from_str(&env, "99"),
        &PerioSite::Buccal,
        &3,
        &0,
        &false,
        &None,
    );
    assert_eq!(result, Err(Ok(Error::InvalidToothNumber)));
}

#[test]
fn test_create_chart_rejects_unknown_notation() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);

    let result = client.try_create_dental_chart(
        &patient,
        &dentist,
        &1_700_000_000,
        &Symbol::new(&env, "klingon"),
    );
    assert_eq!(result, Err(Ok(Error::InvalidNotationSystem)));
}

#[test]
fn test_create_chart_rejects_future_date() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);

    let result = client.try_create_dental_chart(
        &patient,
        &dentist,
        &(1_700_000_000 + 86_400),
        &Symbol::new(&env, "fdi"),
    );
    assert_eq!(result, Err(Ok(Error::InvalidPastDate)));
}

#[test]
fn test_record_periodontal_assessment() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);

    let chart_id = client.create_dental_chart(
        &patient,
        &dentist,
        &1_700_000_000,
        &Symbol::new(&env, "fdi"),
    );

    client.record_periodontal_assessment(
        &chart_id,
        &String::from_str(&env, "11"),
        &PerioSite::MesioBuccal,
        &3,
        &0,
        &false,
        &None,
    );
}

#[test]
fn test_create_treatment_plan() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);

    let plan_id = client.create_treatment_plan(
        &patient,
        &dentist,
        &1_700_000_000,
        &empty_procedures(&env),
        &false,
        &1000,
    );

    assert_eq!(plan_id, 1);
}

#[test]
fn test_schedule_dental_procedure() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);

    let plan_id = client.create_treatment_plan(
        &patient,
        &dentist,
        &1_700_000_000,
        &empty_procedures(&env),
        &false,
        &1000,
    );

    let appt_id = client.schedule_dental_procedure(
        &plan_id,
        &1,
        &(env.ledger().timestamp() + 86_400),
        &30,
        &false,
    );

    assert_eq!(appt_id, 1);
}

#[test]
fn test_document_procedure_performed() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);

    let plan_id = client.create_treatment_plan(
        &patient,
        &dentist,
        &1_700_000_000,
        &empty_procedures(&env),
        &false,
        &1000,
    );

    let appt_id = client.schedule_dental_procedure(
        &plan_id,
        &1,
        &(env.ledger().timestamp() + 86_400),
        &30,
        &false,
    );

    client.document_procedure_performed(
        &appt_id,
        &dentist,
        &env.ledger().timestamp(),
        &Vec::new(&env),
        &Vec::new(&env),
        &None,
        &BytesN::from_array(&env, &[0u8; 32]),
    );
}

#[test]
fn test_document_procedure_performed_unauthorized_dentist() {
    let (env, client) = setup();
    let patient = Address::generate(&env);
    let dentist = Address::generate(&env);
    let other_dentist = Address::generate(&env);

    let plan_id = client.create_treatment_plan(
        &patient,
        &dentist,
        &1_700_000_000,
        &empty_procedures(&env),
        &false,
        &1000,
    );

    let appt_id = client.schedule_dental_procedure(
        &plan_id,
        &1,
        &(env.ledger().timestamp() + 86_400),
        &30,
        &false,
    );

    let result = client.try_document_procedure_performed(
        &appt_id,
        &other_dentist,
        &env.ledger().timestamp(),
        &Vec::new(&env),
        &Vec::new(&env),
        &None,
        &BytesN::from_array(&env, &[0u8; 32]),
    );

    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_document_procedure_performed_missing_appointment() {
    let (env, client) = setup();
    let dentist = Address::generate(&env);

    let result = client.try_document_procedure_performed(
        &999,
        &dentist,
        &env.ledger().timestamp(),
        &Vec::new(&env),
        &Vec::new(&env),
        &None,
        &BytesN::from_array(&env, &[0u8; 32]),
    );

    assert_eq!(result, Err(Ok(Error::NotFound)));
}
