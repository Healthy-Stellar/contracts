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
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);
    client.initialize(&provider_registry_id);

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

/// Set up a provider registry and register a single provider for use
/// with a pre-existing env + lab-management client.  Returns the
/// registered provider address.
fn setup_registered_provider(
    env: &Env,
    client: &LabManagementContractClient<'static>,
) -> Address {
    let provider_registry_id = env.register_contract(None, ProviderRegistry);
    let pr_client = ProviderRegistryClient::new(env, &provider_registry_id);
    let admin = Address::generate(env);
    pr_client.initialize(&admin);

    client.initialize(&provider_registry_id);

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

    provider
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
#[should_panic(expected = "Error(Contract, #3)")]
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
    let provider = setup_registered_provider(&env, &client);

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
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);
    let provider = setup_registered_provider(&env, &client);

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
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);
    let provider = setup_registered_provider(&env, &client);

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
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);

    let lab = Address::generate(&env);
    let test_code = String::from_str(&env, "12345-1");
    let value = String::from_str(&env, "9.0");

    client.flag_critical_value(&999, &lab, &test_code, &value);
}

/// The lab actually assigned to the order must be able to flag a critical
/// value successfully.
#[test]
fn test_flag_critical_value_correct_lab_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);
    let provider = setup_registered_provider(&env, &client);

    let patient = Address::generate(&env);
    let lab = Address::generate(&env);
    let test_code = String::from_str(&env, "12345-1");
    let value = String::from_str(&env, "9.0");

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    client.assign_lab(&order_id, &lab, &3600);

    let result = client.try_flag_critical_value(&order_id, &lab, &test_code, &value);
    assert!(result.is_ok());
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
    let (env, client, provider, patient) = setup_env_with_registered_provider();

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
    let (env, client, provider, patient) = setup_env_with_registered_provider();

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
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    // Seed the counter to u32::MAX so the next order gets ID u32::MAX.
    // We write directly into instance storage to avoid ordering u32::MAX orders.
    env.as_contract(&client.address, || {
        env.storage()
            .instance()
            .set(&DataKey::LabCounter, &(u32::MAX as u64));
    });

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
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    // Seed counter to u32::MAX so the first call returns u32::MAX,
    // and the second call returns u32::MAX + 1.
    env.as_contract(&client.address, || {
        env.storage()
            .instance()
            .set(&DataKey::LabCounter, &(u32::MAX as u64));
    });

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
#[should_panic(expected = "Error(Contract, #4)")]
fn test_order_id_overflow_panics() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    // Seed the counter to u64::MAX so the next increment overflows.
    env.as_contract(&client.address, || {
        env.storage()
            .instance()
            .set(&DataKey::LabCounter, &u64::MAX);
    });

    // This must panic with OrderIdOverflow (error code 4).
    client.order_lab_test(&provider, &patient, &make_req(&env));
}

#[test]
fn test_provider_registration_verification() {
    let env = Env::default();
    env.mock_all_auths();

    // Register ProviderRegistry and initialize it
    let provider_registry_id = env.register_contract(None, ProviderRegistry);
    let pr_client = ProviderRegistryClient::new(&env, &provider_registry_id);
    let admin = Address::generate(&env);
    pr_client.initialize(&admin);

    // Register LabManagementContract and initialize it with ProviderRegistry
    let lab_contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &lab_contract_id);
    client.initialize(&provider_registry_id);

    let unregistered_provider = Address::generate(&env);
    let registered_provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // Register one provider but not the other
    pr_client.register_provider(
        &admin,
        &registered_provider,
        &String::from_str(&env, "Dr. Lab"),
        &String::from_str(&env, "Pathology"),
        &String::from_str(&env, "LAB123"),
        &BytesN::from_array(&env, &[1; 32]),
        &admin,
        &BytesN::from_array(&env, &[2; 32]),
        &(env.ledger().timestamp() + 86400),
        &BytesN::from_array(&env, &[3; 32]),
    );

    // Try to order from unregistered provider
    let res = client.try_order_lab_test(&unregistered_provider, &patient, &make_req(&env));
    assert!(res.is_err()); // ProviderNotRegistered

    // Order from registered provider should succeed
    let result = client.try_order_lab_test(&registered_provider, &patient, &make_req(&env));
    assert!(result.is_ok());
    let order_id = result.unwrap().unwrap();
    assert_eq!(order_id, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_submit_results_by_unassigned_lab_returns_error() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    let lab_a = Address::generate(&env);
    let lab_b = Address::generate(&env);

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));
    client.assign_lab(&order_id, &lab_a, &3600);

    // lab_b attempts to submit results for an order assigned to lab_a
    client.submit_results(
        &order_id,
        &lab_b,
        &BytesN::from_array(&env, &[2u8; 32]),
        &vec![&env, make_result(&env)],
        &true,
    );
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_assign_lab_without_auth_returns_error() {
    let (env, client, provider, patient) = setup_env_with_registered_provider();

    let lab = Address::generate(&env);

    let order_id = client.order_lab_test(&provider, &patient, &make_req(&env));

    // Disable mocked auths: assign_lab requires the order's provider to
    // authorize, which is no longer satisfied.
    env.mock_auths(&[]);
    client.assign_lab(&order_id, &lab, &3600);
}

// ── regression: unregistered-provider guard ───────────────────────────────────

/// Regression test: `order_lab_test` must reject a provider that has never
/// been registered in the provider registry, returning `ProviderNotRegistered`
/// (error code 5).
#[test]
fn test_order_lab_test_rejects_unregistered_provider() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy a provider registry but do NOT register the provider in it.
    let provider_registry_id = env.register_contract(None, ProviderRegistry);
    let pr_client = ProviderRegistryClient::new(&env, &provider_registry_id);
    let admin = Address::generate(&env);
    pr_client.initialize(&admin);

    // Deploy the lab-management contract and wire it to the registry.
    let contract_id = env.register(LabManagementContract, ());
    let client = LabManagementContractClient::new(&env, &contract_id);
    client.initialize(&provider_registry_id);

    let unregistered_provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // Must fail with ProviderNotRegistered (error code 5).
    let res = client.try_order_lab_test(&unregistered_provider, &patient, &make_req(&env));
    assert_eq!(
        res.err().expect("expected ProviderNotRegistered error"),
        Ok(Error::ProviderNotRegistered),
    );
}
