#![cfg(test)]
#![allow(deprecated)]

use crate::types::*;
use crate::{DentalRecordsContract, DentalRecordsContractClient};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec};

fn create_env() -> (Env, DentalRecordsContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, DentalRecordsContract);
    let client = DentalRecordsContractClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_tooth_charting_systems() {
    let (env, client) = create_env();
    let patient_id = Address::generate(&env);
    let dentist_id = Address::generate(&env);

    // Create chart
    let chart_id = client.create_dental_chart(
        &patient_id,
        &dentist_id,
        &1672531200,
        &Symbol::new(&env, "universal"),
    );
    assert_eq!(chart_id, 1);

    // Record tooth condition
    let tooth_num = String::from_str(&env, "8"); // Universal notation for maxillary right central incisor
    client.record_tooth_condition(
        &chart_id,
        &tooth_num,
        &Some(Symbol::new(&env, "occlusal")),
        &Symbol::new(&env, "caries"),
        &Some(String::from_str(&env, "deep decay")),
    );
}

#[test]
fn test_periodontal_tracking() {
    let (env, client) = create_env();
    let patient_id = Address::generate(&env);
    let dentist_id = Address::generate(&env);

    let chart_id = client.create_dental_chart(
        &patient_id,
        &dentist_id,
        &1672531200,
        &Symbol::new(&env, "fdi"),
    );
    let tooth_num = String::from_str(&env, "11");

    // Record periodontal assessment
    client.record_periodontal_assessment(
        &chart_id,
        &tooth_num,
        &Symbol::new(&env, "mb"),
        &4,
        &1,
        &true,
        &Some(1),
    );
}

#[test]
fn test_treatment_planning_flow() {
    let (env, client) = create_env();
    let patient_id = Address::generate(&env);
    let dentist_id = Address::generate(&env);

    let procedure = PlannedProcedure {
        procedure_id: 1,
        procedure_code: String::from_str(&env, "D2391"),
        tooth_number: Some(String::from_str(&env, "18")),
        surfaces: Some(Vec::from_array(&env, [Symbol::new(&env, "occlusal")])),
        description: String::from_str(&env, "Resin composite ONE surface posterior"),
        priority: Symbol::new(&env, "high"),
        estimated_cost: 15000,
    };

    let plan_id = client.create_treatment_plan(
        &patient_id,
        &dentist_id,
        &1672531200,
        &Vec::from_array(&env, [procedure]),
        &false,
        &15000,
    );
    assert_eq!(plan_id, 1);

    let appt_id = client.schedule_dental_procedure(&plan_id, &1, &1672617600, &60, &false);
    assert_eq!(appt_id, 1);
}

#[test]
fn test_radiograph_management() {
    let (env, client) = create_env();
    let patient_id = Address::generate(&env);

    let image_hash = BytesN::from_array(&env, &[1u8; 32]);
    let radio_id = client.record_dental_radiograph(
        &patient_id,
        &Symbol::new(&env, "panoramic"),
        &1672531200,
        &Vec::new(&env),
        &Vec::from_array(&env, [String::from_str(&env, "impacted 38, 48")]),
        &image_hash,
    );
    assert_eq!(radio_id, 1);
}

#[test]
fn test_orthodontic_tracking_flow() {
    let (env, client) = create_env();
    let patient_id = Address::generate(&env);
    let orthodontist_id = Address::generate(&env);

    let plan_hash = BytesN::from_array(&env, &[2u8; 32]);
    let ortho_id = client.track_orthodontic_treatment(
        &patient_id,
        &orthodontist_id,
        &1672531200,
        &Symbol::new(&env, "braces"),
        &plan_hash,
        &24,
    );
    assert_eq!(ortho_id, 1);

    client.record_ortho_adjustment(
        &ortho_id,
        &1675123200,
        &Vec::from_array(&env, [String::from_str(&env, "tightened upper arch")]),
        &false,
        &4,
    );
}

#[test]
fn test_procedure_documentation_flow() {
    let (env, client) = create_env();
    let patient_id = Address::generate(&env);
    let dentist_id = Address::generate(&env);

    // Setup for document_procedure: requires plan, schedule.
    let plan_id = client.create_treatment_plan(
        &patient_id,
        &dentist_id,
        &1672531200,
        &Vec::new(&env),
        &false,
        &0,
    );
    let appt_id = client.schedule_dental_procedure(&plan_id, &1, &1672617600, &60, &true);

    let comp_proc = CompletedProcedure {
        procedure_code: String::from_str(&env, "D0120"),
        tooth_number: None,
        surfaces: None,
        materials_used: Vec::new(&env),
        technique: String::from_str(&env, "visual inspection"),
    };

    let inst_hash = BytesN::from_array(&env, &[3u8; 32]);
    client.document_procedure_performed(
        &appt_id,
        &dentist_id,
        &1672618000,
        &Vec::from_array(&env, [comp_proc]),
        &Vec::from_array(&env, [String::from_str(&env, "local anesthetic lidocaine")]),
        &None,
        &inst_hash,
    );

    // Prescribe rx
    let rx_id = client.prescribe_dental_medication(
        &patient_id,
        &dentist_id,
        &String::from_str(&env, "Amoxicillin 500mg"),
        &String::from_str(&env, "Prophylaxis"),
        &String::from_str(&env, "Take 1 cap 1hr prior to appt"),
    );
    assert_eq!(rx_id, 1);

    // Consent
    let consent_hash = BytesN::from_array(&env, &[4u8; 32]);
    client.document_informed_consent_dental(
        &patient_id,
        &String::from_str(&env, "Extraction 38"),
        &Vec::from_array(&env, [String::from_str(&env, "Bleeding, nerve damage")]),
        &Vec::from_array(&env, [String::from_str(&env, "Do nothing")]),
        &1672617500,
        &consent_hash,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_not_found() {
    let (env, client) = create_env();
    let tooth_num = String::from_str(&env, "8");
    client.record_tooth_condition(
        &999,
        &tooth_num,
        &Some(Symbol::new(&env, "occlusal")),
        &Symbol::new(&env, "caries"),
        &Some(String::from_str(&env, "deep decay")),
    );
}
