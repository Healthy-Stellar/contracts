#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_register_hospital() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "General Hospital"),
        &String::from_str(&env, "123 Main St, New York, NY"),
        &String::from_str(&env, "Services: ER, Surgery, Cardiology"),
    );

    let hospital = client.get_hospital(&hospital_wallet);

    assert_eq!(hospital.name, String::from_str(&env, "General Hospital"));
    assert_eq!(
        hospital.location,
        String::from_str(&env, "123 Main St, New York, NY")
    );
    assert_eq!(
        hospital.metadata,
        String::from_str(&env, "Services: ER, Surgery, Cardiology")
    );
}

#[test]
fn test_update_hospital() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "456 Oak Ave"),
        &String::from_str(&env, "General Services"),
    );

    client.update_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Services: ER, ICU, Pediatrics, Oncology"),
    );

    let hospital = client.get_hospital(&hospital_wallet);

    assert_eq!(
        hospital.metadata,
        String::from_str(&env, "Services: ER, ICU, Pediatrics, Oncology")
    );
    assert_eq!(hospital.name, String::from_str(&env, "City Hospital"));
}

#[test]
#[should_panic(expected = "Hospital already registered")]
fn test_duplicate_registration() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Test Hospital"),
        &String::from_str(&env, "Test Location"),
        &String::from_str(&env, "Test Metadata"),
    );

    // Attempt to register again
    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Test Hospital"),
        &String::from_str(&env, "Test Location"),
        &String::from_str(&env, "Test Metadata"),
    );
}

#[test]
#[should_panic(expected = "Hospital not found")]
fn test_get_nonexistent_hospital() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    client.get_hospital(&hospital_wallet);
}

#[test]
#[should_panic(expected = "Hospital not found")]
fn test_update_nonexistent_hospital() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.update_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Updated Metadata"),
    );
}
