#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_double_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin); // Should panic
}

#[test]
fn test_register_entity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let name = String::from_str(&env, "City Hospital");
    let metadata = String::from_str(&env, "General Hospital");

    client.register_entity(&hospital, &EntityType::Hospital, &name, &metadata);

    let entity = client.get_entity(&hospital);
    assert_eq!(entity.name, name);
    assert_eq!(entity.entity_type, EntityType::Hospital);
    assert!(entity.active);
}

#[test]
#[should_panic(expected = "Entity already registered")]
fn test_duplicate_registration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let name = String::from_str(&env, "City Hospital");
    let metadata = String::from_str(&env, "General Hospital");

    client.register_entity(&hospital, &EntityType::Hospital, &name, &metadata);
    client.register_entity(&hospital, &EntityType::Hospital, &name, &metadata); // Should panic
}

#[test]
fn test_grant_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Register a hospital and a doctor
    let hospital = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    client.register_entity(
        &doctor,
        &EntityType::Doctor,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "metadata"),
    );

    // Hospital grants access to doctor for patient records
    let resource_id = String::from_str(&env, "patient-123-records");
    client.grant_access(&hospital, &doctor, &resource_id, &0);

    // Check that doctor has access
    assert!(client.check_access(&doctor, &resource_id));

    // Check authorized parties
    let authorized = client.get_authorized_parties(&resource_id);
    assert_eq!(authorized.len(), 1);
}

#[test]
fn test_revoke_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    client.register_entity(
        &doctor,
        &EntityType::Doctor,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "metadata"),
    );

    let resource_id = String::from_str(&env, "patient-123-records");
    client.grant_access(&hospital, &doctor, &resource_id, &0);

    // Verify access exists
    assert!(client.check_access(&doctor, &resource_id));

    // Revoke access
    client.revoke_access(&hospital, &doctor, &resource_id);

    // Verify access is revoked
    assert!(!client.check_access(&doctor, &resource_id));

    // Verify authorized parties is empty
    let authorized = client.get_authorized_parties(&resource_id);
    assert_eq!(authorized.len(), 0);
}

#[test]
fn test_check_access_expired() {
    use soroban_sdk::testutils::Ledger;

    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    client.register_entity(
        &doctor,
        &EntityType::Doctor,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "metadata"),
    );

    // Grant access with expiration at timestamp 100
    let resource_id = String::from_str(&env, "patient-123-records");
    client.grant_access(&hospital, &doctor, &resource_id, &100);

    // Access should be valid before expiration
    assert!(client.check_access(&doctor, &resource_id));

    // Advance ledger time past expiration
    env.ledger().set_timestamp(200);

    // Access should now be denied (expired)
    assert!(!client.check_access(&doctor, &resource_id));
}

#[test]
fn test_get_entity_permissions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    client.register_entity(
        &doctor,
        &EntityType::Doctor,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "metadata"),
    );

    // Grant multiple access permissions
    let resource_1 = String::from_str(&env, "patient-123-records");
    let resource_2 = String::from_str(&env, "patient-456-records");

    client.grant_access(&hospital, &doctor, &resource_1, &0);
    client.grant_access(&hospital, &doctor, &resource_2, &0);

    // Get all permissions for the doctor
    let permissions = client.get_entity_permissions(&doctor);
    assert_eq!(permissions.len(), 2);
}

#[test]
fn test_deactivate_entity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    // Deactivate the entity
    client.deactivate_entity(&admin, &hospital);

    // Verify entity is deactivated
    let entity = client.get_entity(&hospital);
    assert!(!entity.active);
}

#[test]
#[should_panic(expected = "Only admin can deactivate entities")]
fn test_deactivate_entity_non_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let non_admin = Address::generate(&env);

    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    // Non-admin tries to deactivate - should panic
    client.deactivate_entity(&non_admin, &hospital);
}

#[test]
fn test_update_entity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "Original metadata"),
    );

    // Update metadata
    let new_metadata = String::from_str(&env, "Updated metadata");
    client.update_entity(&hospital, &new_metadata);

    let entity = client.get_entity(&hospital);
    assert_eq!(entity.metadata, new_metadata);
}
