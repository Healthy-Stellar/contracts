#![cfg(test)]
use super::*;
use soroban_sdk::{Address, BytesN, Env, String, Symbol, testutils::Address as _, vec};
use provider_registry::{ProviderRegistry, ProviderRegistryClient};

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


/// Set up an env with a registered provider registry and a registered
/// provider, mirroring test_provider_registration_verification.
/// (order_lab_test verifies provider registration against the registry.)
fn setup_env_with_registered_provider()
-> (Env, LabManagementContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    // Register + initialize the provider registry
    let provider_registry_id = env.register_contract(None, ProviderRegistry);
    let pr_client = ProviderRegistryClient::new(&env, &provider_registry_id);
    let admin = Address::generate(&env);
    pr_client.initialize(&admin);

    // Register the lab-management contract, pointing it at the registry
    let lab_admin = Address::generate(&env);
    let contract_id = env.register(LabManagementContract, (&lab_admin, &provider_registry_id));
    let client = LabManagementContractClient::new(&env, &contract_id);

    // Register the provider that will place lab orders
    let provider = Address::generate(&env);
    pr_client.register_provider(
        &admin,
        &provider,
        &String::from_str(&env, "Dr. Lab"),
        &String::from_str(&env, "Pathology"),
        &String::from_str(&env, "LAB123"),
        &BytesN::from_array(&env, &[1u8; 32]),
        &admin,
        &BytesN::from_array(&env, &[2u8; 32]),
        &(env.ledger().timestamp() + 86400),
        &BytesN::from_array(&env, &[3u8; 32]),
    );

    let patient = Address::generate(&env);
    (env, client, provider, patient)
}

/// Set up a provider registry and register a single provider, then register a
/// lab-management contract pointed at that registry.  Returns the client and
/// the registered provider address.
fn setup_registered_provider(
    env: &Env,
) -> (LabManagementContractClient<'static>, Address) {
    let provider_registry_id = env.register_contract(None, ProviderRegistry);
    let pr_client = ProviderRegistryClient::new(env, &provider_registry_id);
    let admin = Address::generate(env);
    pr_client.initialize(&admin);

    let contract_id = env.register(LabManagementContract, (&admin, &provider_registry_id));
    let client = LabManagementContractClient::new(env, &contract_id);

    let provider = Address::generate(env);
    pr_client.register_provider(
        &admin,
        &provider,
        &String::from_str(env, "Dr. Lab"),
        &String::from_str(env, "Pathology"),
        &String::from_str(env, "LAB123"),
        &BytesN::from_array(env, &[1u8; 32]),
        &admin,
        &BytesN::from_array(env, &[2u8; 32]),
        &(env.ledger().timestamp() + 86400),
        &BytesN::from_array(env, &[3u8; 32]),
    );

    (client, provider)
}

// ── existing tests (unchanged behaviour) ─────────────────────────────────────

#[test]
fn test_happy_path_lifecycle() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

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
fn test_fail_qc_check() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    let lab = Address::generate(&env);

    let req = OrderRequest {
        test_panel: vec![&env, String::from_str(&env, "LOINC-1")],
        priority: Symbol::new(&env, "Routine"),
        clinical_info_hash: BytesN::from_array(&env, &[0u8; 32]),
        fasting_required: false,
        collection_date: None,
    };

    let id = client.order_lab_test(&provider, &patient, &req);
    // The order must be assigned to the lab before results can be submitted.
    client.assign_lab(&id, &lab, &0);
    // A failed QC run is recorded (status = QcFailed) rather than reverting.
    client.submit_results(
        &id,
        &lab,
        &BytesN::from_array(&env, &[0u8; 32]),
        &vec![&env],
        &false,
    );
    assert_eq!(client.get_order_status(&id), LabOrderStatus::QcFailed);
}

#[test]
fn test_critical_value_alerting() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, provider) = setup_registered_provider(&env);

    let patient = Address::generate(&env);
    let lab = Address::generate(&env);
    let test_code = String::from_str(&env, "12345-1");
    let value = String::from_str(&env, "9.0");

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    client.assign_lab(&order_id, &lab, &3600);

    client.flag_critical_value(&order_id, &lab, &test_code, &value);
}

/// A lab that has never been assigned to the order (order.lab_id is None)
/// must not be able to flag a critical value for it.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_flag_critical_value_unassigned_lab_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, provider) = setup_registered_provider(&env);

    let patient = Address::generate(&env);
    let lab = Address::generate(&env);
    let test_code = String::from_str(&env, "12345-1");
    let value = String::from_str(&env, "9.0");

    // Order is created but never assigned to any lab.
    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));

    client.flag_critical_value(&order_id, &lab, &test_code, &value);
}

/// A lab other than the one actually assigned to the order must not be able
/// to flag a critical value for it.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_flag_critical_value_wrong_lab_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, provider) = setup_registered_provider(&env);

    let patient = Address::generate(&env);
    let lab_a = Address::generate(&env);
    let lab_b = Address::generate(&env);
    let test_code = String::from_str(&env, "12345-1");
    let value = String::from_str(&env, "9.0");

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    client.assign_lab(&order_id, &lab_a, &3600);

    // lab_b is not the assigned lab.
    client.flag_critical_value(&order_id, &lab_b, &test_code, &value);
}

/// Flagging a critical value against an order_id that was never created
/// must fail rather than silently emitting an event.
#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_flag_critical_value_nonexistent_order_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(
        LabManagementContract,
        (&Address::generate(&env), &Address::generate(&env)),
    );
    let client = LabManagementContractClient::new(&env, &contract_id);

    let lab = Address::generate(&env);
    let test_code = String::from_str(&env, "12345-1");
    let value = String::from_str(&env, "9.0");

    client.flag_critical_value(&999, &lab, &test_code, &value);
}

// ── order state machine tests (#1104) ────────────────────────────────────────

/// A completed order cannot be reassigned to another lab.
#[test]
#[should_panic]
fn test_assign_lab_rejects_completed_order() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    let lab_a = Address::generate(&env);
    let lab_b = Address::generate(&env);

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    client.assign_lab(&order_id, &lab_a, &3600);
    client.submit_results(
        &order_id,
        &lab_a,
        &BytesN::from_array(&env, &[2u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );
    assert_eq!(client.get_order_status(&order_id), LabOrderStatus::Completed);

    // Reassigning a completed order must be rejected.
    client.assign_lab(&order_id, &lab_b, &3600);
}

/// A completed order's results cannot be silently overwritten by resubmitting.
#[test]
#[should_panic]
fn test_submit_results_rejects_completed_order() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    let lab = Address::generate(&env);

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    client.assign_lab(&order_id, &lab, &3600);
    client.submit_results(
        &order_id,
        &lab,
        &BytesN::from_array(&env, &[2u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );

    // A second submission on a completed order must be rejected.
    client.submit_results(
        &order_id,
        &lab,
        &BytesN::from_array(&env, &[9u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );
}

/// The assigned lab can amend a completed order, recording a reason and
/// moving the order to the Amended state.
#[test]
fn test_amend_results_records_history() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    let lab = Address::generate(&env);

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    client.assign_lab(&order_id, &lab, &3600);
    client.submit_results(
        &order_id,
        &lab,
        &BytesN::from_array(&env, &[2u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );

    client.amend_results(
        &order_id,
        &lab,
        &BytesN::from_array(&env, &[7u8; 32]),
        &String::from_str(&env, "corrected glucose value"),
    );

    assert_eq!(client.get_order_status(&order_id), LabOrderStatus::Amended);
    assert_eq!(client.get_amendments(&order_id).len(), 1);
}

/// A lab that is not the assigned lab cannot amend a completed order.
#[test]
#[should_panic]
fn test_amend_results_rejects_wrong_lab() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    let lab = Address::generate(&env);
    let other_lab = Address::generate(&env);

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    client.assign_lab(&order_id, &lab, &3600);
    client.submit_results(
        &order_id,
        &lab,
        &BytesN::from_array(&env, &[2u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );

    client.amend_results(
        &order_id,
        &other_lab,
        &BytesN::from_array(&env, &[7u8; 32]),
        &String::from_str(&env, "tampered"),
    );
}

/// The admin can rotate the provider registry.
#[test]
fn test_set_provider_registry() {
    let env = Env::default();
    env.mock_all_auths();

    let provider_registry_id = env.register_contract(None, ProviderRegistry);
    let pr_client = ProviderRegistryClient::new(&env, &provider_registry_id);
    let admin = Address::generate(&env);
    pr_client.initialize(&admin);

    let contract_id = env.register(LabManagementContract, (&admin, &provider_registry_id));
    let client = LabManagementContractClient::new(&env, &contract_id);

    let new_registry_id = env.register_contract(None, ProviderRegistry);
    client.set_provider_registry(&new_registry_id);
}
