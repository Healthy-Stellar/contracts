#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};

fn create_test_env() -> (Env, Address, Address, BytesN<32>, BytesN<32>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let patient_id = BytesN::from_array(&env, &[1u8; 32]);
    let hospital_id = BytesN::from_array(&env, &[2u8; 32]);

    (env, admin, patient, patient_id, hospital_id)
}

#[test]
fn test_initiate_discharge_planning() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let admission_date = 1000u64;
    let expected_discharge_date = 2000u64;

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &admission_date,
        &expected_discharge_date,
    );

    assert_eq!(plan_id, 0);

    // Verify plan was stored
    let plan = client.get_discharge_plan(&plan_id);
    assert_eq!(plan.patient_id, patient_id);
    assert_eq!(plan.hospital_id, hospital_id);
    assert_eq!(plan.admission_date, admission_date);
    assert_eq!(plan.expected_discharge_date, expected_discharge_date);
}

#[test]
fn test_initiate_discharge_planning_invalid_dates() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let admission_date = 2000u64;
    let expected_discharge_date = 1000u64; // Before admission

    let result = client.try_initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &admission_date,
        &expected_discharge_date,
    );
    assert!(result.is_err());
}

#[test]
fn test_assess_discharge_readiness() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    // First create a discharge plan
    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    // Assess readiness
    let notes = String::from_str(&env, "Patient stable");
    let assessment = client.assess_discharge_readiness(
        &admin,
        &plan_id,
        &85u32,
        &80u32,
        &90u32,
        &notes,
    );

    assert_eq!(assessment.discharge_plan_id, plan_id);
    assert_eq!(assessment.medical_stability_score, 85);
    assert_eq!(assessment.functional_status_score, 80);
    assert_eq!(assessment.support_system_score, 90);
    assert_eq!(assessment.overall_score, 85);
    assert_eq!(assessment.readiness_level, ReadinessLevel::Ready);
}

#[test]
fn test_assess_discharge_readiness_needs_preparation() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let notes = String::from_str(&env, "Needs home health setup");
    let assessment = client.assess_discharge_readiness(
        &admin,
        &plan_id,
        &70u32,
        &60u32,
        &65u32,
        &notes,
    );

    assert_eq!(assessment.overall_score, 65);
    assert_eq!(assessment.readiness_level, ReadinessLevel::NeedsPreparation);
}

#[test]
fn test_assess_discharge_readiness_not_ready() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let notes = String::from_str(&env, "Medical instability");
    let assessment = client.assess_discharge_readiness(
        &admin,
        &plan_id,
        &40u32,
        &50u32,
        &45u32,
        &notes,
    );

    assert_eq!(assessment.overall_score, 45);
    assert_eq!(assessment.readiness_level, ReadinessLevel::NotReady);
}

#[test]
fn test_create_discharge_orders() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let mut medications = Vec::new(&env);
    medications.push_back(DischargeMedication {
        medication_name: String::from_str(&env, "Aspirin"),
        dosage: String::from_str(&env, "81mg"),
        frequency: String::from_str(&env, "Once daily"),
        duration: String::from_str(&env, "Ongoing"),
        instructions: String::from_str(&env, "Take with food"),
    });

    let instructions = String::from_str(&env, "Rest and follow up");
    let restrictions = String::from_str(&env, "No heavy lifting");

    client.create_discharge_orders(&admin, &plan_id, &medications, &instructions, &restrictions);
}

#[test]
fn test_arrange_home_health() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let agency_id = BytesN::from_array(&env, &[3u8; 32]);
    let service_type = String::from_str(&env, "Nursing");
    let frequency = String::from_str(&env, "3x per week");
    let start_date = 2100u64;

    client.arrange_home_health(&admin, &plan_id, &agency_id, &service_type, &frequency, &start_date);
}

#[test]
fn test_order_dme_for_discharge() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let mut equipment_list = Vec::new(&env);
    equipment_list.push_back(String::from_str(&env, "Walker"));
    equipment_list.push_back(String::from_str(&env, "Wheelchair"));

    let supplier_id = BytesN::from_array(&env, &[4u8; 32]);
    let delivery_date = 1900u64;

    client.order_dme_for_discharge(&admin, &plan_id, &equipment_list, &supplier_id, &delivery_date);
}

#[test]
fn test_schedule_followup_appointments() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let mut appointments = Vec::new(&env);
    appointments.push_back(FollowUpAppointment {
        provider_id: BytesN::from_array(&env, &[5u8; 32]),
        appointment_type: String::from_str(&env, "Primary Care"),
        scheduled_date: 2500u64,
        location: String::from_str(&env, "Clinic A"),
        notes: String::from_str(&env, "Post-discharge checkup"),
    });

    let appointment_ids = client.schedule_followup_appointments(&admin, &plan_id, &appointments);
    assert_eq!(appointment_ids.len(), 1);
}

#[test]
fn test_provide_discharge_education() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let mut topics = Vec::new(&env);
    topics.push_back(String::from_str(&env, "Medication management"));
    topics.push_back(String::from_str(&env, "Wound care"));

    let mut materials = Vec::new(&env);
    materials.push_back(String::from_str(&env, "Medication list"));
    materials.push_back(String::from_str(&env, "Care instructions"));

    client.provide_discharge_education(&admin, &plan_id, &topics, &materials, &85u32);
}

#[test]
fn test_coordinate_with_snf() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let snf_id = BytesN::from_array(&env, &[6u8; 32]);
    let transfer_date = 2050u64;
    let care_requirements = String::from_str(&env, "24/7 nursing care");

    client.coordinate_with_snf(&admin, &plan_id, &snf_id, &transfer_date, &care_requirements);
}

#[test]
fn test_complete_discharge() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let actual_discharge_date = 1950u64;
    let destination = String::from_str(&env, "Home");

    client.complete_discharge(&admin, &plan_id, &actual_discharge_date, &destination);

    // Verify plan status updated
    let plan = client.get_discharge_plan(&plan_id);
    assert_eq!(plan.status, DischargeStatus::Completed);
    assert_eq!(plan.actual_discharge_date, Some(actual_discharge_date));
}

#[test]
fn test_track_readmission_risk_high() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let mut risk_factors = Vec::new(&env);
    risk_factors.push_back(String::from_str(&env, "Multiple comorbidities"));
    risk_factors.push_back(String::from_str(&env, "Limited support system"));

    let mitigation_plan = String::from_str(&env, "Weekly home health visits");

    client.track_readmission_risk(&admin, &plan_id, &risk_factors, &80u32, &mitigation_plan);
}

#[test]
fn test_track_readmission_risk_medium() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let mut risk_factors = Vec::new(&env);
    risk_factors.push_back(String::from_str(&env, "Recent surgery"));

    let mitigation_plan = String::from_str(&env, "Follow-up in 2 weeks");

    client.track_readmission_risk(&admin, &plan_id, &risk_factors, &60u32, &mitigation_plan);
}

#[test]
fn test_track_readmission_risk_low() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    let mut risk_factors = Vec::new(&env);
    risk_factors.push_back(String::from_str(&env, "Good support system"));

    let mitigation_plan = String::from_str(&env, "Standard follow-up");

    client.track_readmission_risk(&admin, &plan_id, &risk_factors, &30u32, &mitigation_plan);
}

#[test]
fn test_assess_readiness_nonexistent_plan() {
    let (env, admin, _patient, _patient_id, _hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    let notes = String::from_str(&env, "Test");
    let result =
        client.try_assess_discharge_readiness(&admin, &999u64, &80u32, &80u32, &80u32, &notes);
    assert!(result.is_err());
}

#[test]
fn test_multiple_discharge_plans() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    // Create first plan
    let plan_id_1 = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    // Create second plan
    let patient_id_2 = BytesN::from_array(&env, &[10u8; 32]);
    let plan_id_2 = client.initiate_discharge_planning(
        &admin,
        &patient_id_2,
        &hospital_id,
        &1500u64,
        &2500u64,
    );

    assert_eq!(plan_id_1, 0);
    assert_eq!(plan_id_2, 1);

    // Verify both plans exist independently
    let plan1 = client.get_discharge_plan(&plan_id_1);
    let plan2 = client.get_discharge_plan(&plan_id_2);

    assert_eq!(plan1.patient_id, patient_id);
    assert_eq!(plan2.patient_id, patient_id_2);
}

#[test]
fn test_full_discharge_workflow() {
    let (env, admin, _patient, patient_id, hospital_id) = create_test_env();
    let contract_id = env.register_contract(None, HospitalDischargeContract);
    let client = HospitalDischargeContractClient::new(&env, &contract_id);

    // 1. Initiate discharge planning
    let plan_id = client.initiate_discharge_planning(
        &admin,
        &patient_id,
        &hospital_id,
        &1000u64,
        &2000u64,
    );

    // 2. Assess readiness
    let notes = String::from_str(&env, "Ready for discharge");
    client.assess_discharge_readiness(&admin, &plan_id, &85u32, &80u32, &90u32, &notes);

    // 3. Create discharge orders
    let mut medications = Vec::new(&env);
    medications.push_back(DischargeMedication {
        medication_name: String::from_str(&env, "Aspirin"),
        dosage: String::from_str(&env, "81mg"),
        frequency: String::from_str(&env, "Daily"),
        duration: String::from_str(&env, "Ongoing"),
        instructions: String::from_str(&env, "With food"),
    });
    client.create_discharge_orders(
        &admin,
        &plan_id,
        &medications,
        &String::from_str(&env, "Rest"),
        &String::from_str(&env, "No lifting"),
    );

    // 4. Arrange home health
    let agency_id = BytesN::from_array(&env, &[3u8; 32]);
    client.arrange_home_health(
        &admin,
        &plan_id,
        &agency_id,
        &String::from_str(&env, "Nursing"),
        &String::from_str(&env, "3x/week"),
        &2100u64,
    );

    // 5. Schedule follow-up
    let mut appointments = Vec::new(&env);
    appointments.push_back(FollowUpAppointment {
        provider_id: BytesN::from_array(&env, &[5u8; 32]),
        appointment_type: String::from_str(&env, "Primary Care"),
        scheduled_date: 2500u64,
        location: String::from_str(&env, "Clinic"),
        notes: String::from_str(&env, "Checkup"),
    });
    client.schedule_followup_appointments(&admin, &plan_id, &appointments);

    // 6. Provide education
    let mut topics = Vec::new(&env);
    topics.push_back(String::from_str(&env, "Medications"));
    let mut materials = Vec::new(&env);
    materials.push_back(String::from_str(&env, "Med list"));
    client.provide_discharge_education(&admin, &plan_id, &topics, &materials, &90u32);

    // 7. Track readmission risk
    let mut risk_factors = Vec::new(&env);
    risk_factors.push_back(String::from_str(&env, "Age"));
    client.track_readmission_risk(
        &admin,
        &plan_id,
        &risk_factors,
        &40u32,
        &String::from_str(&env, "Monitor"),
    );

    // 8. Complete discharge
    client.complete_discharge(
        &admin,
        &plan_id,
        &1950u64,
        &String::from_str(&env, "Home"),
    );

    // Verify final state
    let plan = client.get_discharge_plan(&plan_id);
    assert_eq!(plan.status, DischargeStatus::Completed);
}
