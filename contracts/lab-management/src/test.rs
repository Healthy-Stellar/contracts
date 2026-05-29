#![cfg(test)]
use super::*;
use soroban_sdk::{Address, Env, String, testutils::Address as _, vec};

// ── helpers ──────────────────────────────────────────────────────────────────

fn make_req(env: &Env) -> OrderRequest {
    OrderRequest {
        test_panel: vec![env, String::from_str(env, "2345-7")],
        priority: Symbol::new(env, "STAT"),
        clinical_info_hash: BytesN::from_array(env, &[1u8; 32]),
        fasting_required: true,
        collection_date: Some(0),
    }
}

fn make_result(env: &Env) -> TestResult {
    TestResult {
        test_code: String::from_str(env, "2345-7"),
        test_name: String::from_str(env, "Glucose"),
        value: String::from_str(env, "450"),
        unit: String::from_str(env, "mg/dL"),
        reference_range: String::from_str(env, "70-99"),
        is_abnormal: true,
        abnormal_flag: Some(Symbol::new(env, "CRITICAL")),
    }
}

// ── existing tests (unchanged behaviour) ─────────────────────────────────────

#[test]
fn test_happy_path_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let lab = Address::generate(&env);

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    assert_eq!(order_id, 0);

    client.assign_lab(&order_id, &lab, &3600);

    client.submit_results(
        &order_id,
        &lab,
        &BytesN::from_array(&env, &[2u8; 32]),
        &vec![&env, make_result(&env)],
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

    client.flag_critical_value(&0, &lab, &test_code, &value);
}

#[test]
#[should_panic]
fn test_fail_assign_nonexistent_order() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let lab = Address::generate(&env);
    client.assign_lab(&999, &lab, &0);
}

// ── u64 ID correctness tests ──────────────────────────────────────────────────

/// IDs are assigned sequentially starting from 0.
#[test]
fn test_order_ids_are_sequential() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let id0 = client.order_lab_test(&provider, &patient, &make_req(&env));
    let id1 = client.order_lab_test(&provider, &patient, &make_req(&env));
    let id2 = client.order_lab_test(&provider, &patient, &make_req(&env));

    assert_eq!(id0, 0u64);
    assert_eq!(id1, 1u64);
    assert_eq!(id2, 2u64);
}

/// Records stored under different IDs are independent — reading one does not
/// return the other.  This guards against key-collision caused by truncation.
#[test]
fn test_distinct_ids_store_independent_records() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let lab_a = Address::generate(&env);
    let lab_b = Address::generate(&env);

    let id0 = client.order_lab_test(&provider, &patient, &make_req(&env));
    let id1 = client.order_lab_test(&provider, &patient, &make_req(&env));

    // Assign different labs to each order.
    client.assign_lab(&id0, &lab_a, &0);
    client.assign_lab(&id1, &lab_b, &0);

    // Submit results only for id0.
    client.submit_results(
        &id0,
        &lab_a,
        &BytesN::from_array(&env, &[10u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );

    // id1 must still be in "Assigned" state — not "Completed".
    // If truncation caused a collision, id1 would have been overwritten.
    // We verify by attempting to submit results for id1 with lab_b (which
    // would panic if the order had been corrupted to point at lab_a).
    client.submit_results(
        &id1,
        &lab_b,
        &BytesN::from_array(&env, &[11u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );
}

/// An ID that would have been truncated by a u64→u32 cast (i.e. any value
/// above u32::MAX) must be stored and retrieved correctly.
#[test]
fn test_id_above_u32_max_stored_and_retrieved() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());

    // Seed the counter to u32::MAX so the next order gets ID u32::MAX.
    // We write directly into instance storage to avoid ordering u32::MAX orders.
    env.as_contract(&contract_id, || {
        env.storage()
            .instance()
            .set(&DataKey::LabCounter, &(u32::MAX as u64));
    });

    let client = LabManagementContractClient::new(&env, &contract_id);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let lab = Address::generate(&env);

    // This order gets ID == u32::MAX (0xFFFF_FFFF).
    let id = client.order_lab_test(&provider, &patient, &make_req(&env));
    assert_eq!(id, u32::MAX as u64);

    // Assign and submit — both must succeed, proving the full u64 key is used.
    client.assign_lab(&id, &lab, &0);
    client.submit_results(
        &id,
        &lab,
        &BytesN::from_array(&env, &[99u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );
}

/// An ID strictly above u32::MAX must also work without any truncation.
#[test]
fn test_id_strictly_above_u32_max_no_collision() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());

    // Seed counter to u32::MAX so the first call returns u32::MAX,
    // and the second call returns u32::MAX + 1.
    env.as_contract(&contract_id, || {
        env.storage()
            .instance()
            .set(&DataKey::LabCounter, &(u32::MAX as u64));
    });

    let client = LabManagementContractClient::new(&env, &contract_id);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let lab_a = Address::generate(&env);
    let lab_b = Address::generate(&env);

    let id_at_max = client.order_lab_test(&provider, &patient, &make_req(&env));
    let id_above_max = client.order_lab_test(&provider, &patient, &make_req(&env));

    assert_eq!(id_at_max, u32::MAX as u64);
    assert_eq!(id_above_max, (u32::MAX as u64) + 1);

    // Both IDs must be independently addressable.
    client.assign_lab(&id_at_max, &lab_a, &0);
    client.assign_lab(&id_above_max, &lab_b, &0);

    // Submit for id_above_max with lab_b — would panic if the key had been
    // truncated to 0 (colliding with id_at_max which is assigned to lab_a).
    client.submit_results(
        &id_above_max,
        &lab_b,
        &BytesN::from_array(&env, &[77u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );
}

/// When the counter is at u64::MAX, order_lab_test must panic with
/// OrderIdOverflow rather than silently wrapping.
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_order_id_overflow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());

    // Seed the counter to u64::MAX so the next increment overflows.
    env.as_contract(&contract_id, || {
        env.storage()
            .instance()
            .set(&DataKey::LabCounter, &u64::MAX);
    });

    let client = LabManagementContractClient::new(&env, &contract_id);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // This must panic with OrderIdOverflow (error code 5).
    client.order_lab_test(&provider, &patient, &make_req(&env));
}
