#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_create_doctor_profile() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DoctorRegistry);
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.create_doctor_profile(
        &doctor_wallet,
        &String::from_str(&env, "Dr. John Smith"),
        &String::from_str(&env, "Cardiology"),
        &institution_wallet,
    );

    let profile = client.get_doctor_profile(&doctor_wallet);

    assert_eq!(profile.name, String::from_str(&env, "Dr. John Smith"));
    assert_eq!(profile.specialization, String::from_str(&env, "Cardiology"));
    assert_eq!(profile.institution_wallet, institution_wallet);
    assert_eq!(profile.metadata, String::from_str(&env, ""));
}

#[test]
fn test_update_doctor_profile() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DoctorRegistry);
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.create_doctor_profile(
        &doctor_wallet,
        &String::from_str(&env, "Dr. Jane Doe"),
        &String::from_str(&env, "Neurology"),
        &institution_wallet,
    );

    client.update_doctor_profile(
        &doctor_wallet,
        &String::from_str(&env, "Pediatric Neurology"),
        &String::from_str(&env, "Board Certified, 15 years experience"),
    );

    let profile = client.get_doctor_profile(&doctor_wallet);

    assert_eq!(
        profile.specialization,
        String::from_str(&env, "Pediatric Neurology")
    );
    assert_eq!(
        profile.metadata,
        String::from_str(&env, "Board Certified, 15 years experience")
    );
    assert_eq!(profile.name, String::from_str(&env, "Dr. Jane Doe"));
}

#[test]
#[should_panic(expected = "Doctor profile already exists")]
fn test_duplicate_profile_creation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DoctorRegistry);
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.create_doctor_profile(
        &doctor_wallet,
        &String::from_str(&env, "Dr. Test"),
        &String::from_str(&env, "General Medicine"),
        &institution_wallet,
    );

    // Attempt to create again
    client.create_doctor_profile(
        &doctor_wallet,
        &String::from_str(&env, "Dr. Test"),
        &String::from_str(&env, "General Medicine"),
        &institution_wallet,
    );
}

#[test]
#[should_panic(expected = "Doctor profile not found")]
fn test_get_nonexistent_profile() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DoctorRegistry);
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);

    client.get_doctor_profile(&doctor_wallet);
}

#[test]
#[should_panic(expected = "Doctor profile not found")]
fn test_update_nonexistent_profile() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DoctorRegistry);
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.update_doctor_profile(
        &doctor_wallet,
        &String::from_str(&env, "Cardiology"),
        &String::from_str(&env, "Updated info"),
    );
}

#[test]
fn test_multiple_doctors() {
    let env = Env::default();
    let contract_id = env.register_contract(None, DoctorRegistry);
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let doctor1_wallet = Address::generate(&env);
    let doctor2_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.create_doctor_profile(
        &doctor1_wallet,
        &String::from_str(&env, "Dr. Alice"),
        &String::from_str(&env, "Oncology"),
        &institution_wallet,
    );

    client.create_doctor_profile(
        &doctor2_wallet,
        &String::from_str(&env, "Dr. Bob"),
        &String::from_str(&env, "Orthopedics"),
        &institution_wallet,
    );

    let profile1 = client.get_doctor_profile(&doctor1_wallet);
    let profile2 = client.get_doctor_profile(&doctor2_wallet);

    assert_eq!(profile1.name, String::from_str(&env, "Dr. Alice"));
    assert_eq!(profile1.specialization, String::from_str(&env, "Oncology"));

    assert_eq!(profile2.name, String::from_str(&env, "Dr. Bob"));
    assert_eq!(
        profile2.specialization,
        String::from_str(&env, "Orthopedics")
    );
}
