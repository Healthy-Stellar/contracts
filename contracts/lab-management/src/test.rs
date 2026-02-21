#![cfg(test)]
use super::*;
use soroban_sdk::{Address, Env, String, testutils::Address as _, vec};

#[test]
fn test_happy_path_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let lab = Address::generate(&env);

    let req = OrderRequest {
        test_panel: vec![&env, String::from_str(&env, "2345-7")],
        priority: Symbol::new(&env, "STAT"),
        clinical_info_hash: BytesN::from_array(&env, &[1u8; 32]),
        fasting_required: true,
        collection_date: Some(0),
    };

    // Test Ordering
    let order_id = client.order_lab_test(&provider, &patient, &req);
    assert_eq!(order_id, 0);

    // Test Assignment
    client.assign_lab(&order_id, &lab, &3600);

    // Test Submission
    let result = TestResult {
        test_code: String::from_str(&env, "2345-7"),
        test_name: String::from_str(&env, "Glucose"),
        value: String::from_str(&env, "450"),
        unit: String::from_str(&env, "mg/dL"),
        reference_range: String::from_str(&env, "70-99"),
        is_abnormal: true,
        abnormal_flag: Some(Symbol::new(&env, "CRITICAL")),
    };

    client.submit_results(
        &order_id,
        &lab,
        &BytesN::from_array(&env, &[2u8; 32]),
        &vec![&env, result],
        &true,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_fail_qc_check() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let lab = Address::generate(&env);

    let req = OrderRequest {
        test_panel: vec![&env, String::from_str(&env, "LOINC-1")],
        priority: Symbol::new(&env, "Routine"),
        clinical_info_hash: BytesN::from_array(&env, &[0u8; 32]),
        fasting_required: false,
        collection_date: None,
    };

    let id = client.order_lab_test(&provider, &patient, &req);

    // This must panic because qc_passed is false
    client.submit_results(
        &id,
        &lab,
        &BytesN::from_array(&env, &[0u8; 32]),
        &vec![&env],
        &false,
    );
}

#[test]
fn test_critical_value_alerting() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let lab = Address::generate(&env);
    let test_code = String::from_str(&env, "12345-1");
    let value = String::from_str(&env, "9.0");

    // Testing that the function executes and publishes events
    client.flag_critical_value(&0, &lab, &test_code, &value);
}

#[test]
#[should_panic] // Should panic because order 999 doesn't exist
fn test_fail_assign_nonexistent_order() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let lab = Address::generate(&env);
    client.assign_lab(&999, &lab, &0);
}
