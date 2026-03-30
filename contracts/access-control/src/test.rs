#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    Address, Bytes, Env, IntoVal, String,
};

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
fn test_double_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let result = client.try_initialize(&admin);
    assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
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

    let result = client.try_register_entity(&hospital, &EntityType::Hospital, &name, &metadata);
    assert_eq!(result, Err(Ok(ContractError::EntityAlreadyRegistered)));
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

    let result = client.try_deactivate_entity(&non_admin, &hospital);
    assert_eq!(result, Err(Ok(ContractError::OnlyAdminCanDeactivate)));
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

#[test]
fn test_register_and_get_did() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    client.initialize(&admin);

    let did = Bytes::from_slice(&env, b"did:stellar:patient:abc123");
    client.register_did(&patient, &did);

    let stored = client.get_did(&patient).unwrap();
    assert_eq!(stored, did);
}

#[test]
fn test_register_did_invalid_format_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    client.initialize(&admin);

    let invalid = Bytes::from_slice(&env, b"stellar:provider:abc123");
    let result = client.try_register_did(&provider, &invalid);
    assert!(matches!(result, Err(Ok(ContractError::InvalidDidFormat))));
}

#[test]
fn test_register_did_self_registration_only() {
    let env = Env::default();
    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let attacker = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "initialize",
                args: (&admin,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .initialize(&admin);

    let did = Bytes::from_slice(&env, b"did:stellar:patient:secure1");
    let unauthorized = client
        .mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "register_did",
                args: (&patient, &did).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_register_did(&patient, &did);

    assert!(unauthorized.is_err());
}

#[test]
fn test_register_did_update_replaces_value() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    client.initialize(&admin);

    let did_v1 = Bytes::from_slice(&env, b"did:stellar:provider:old");
    let did_v2 = Bytes::from_slice(&env, b"did:stellar:provider:new");
    client.register_did(&provider, &did_v1);
    client.register_did(&provider, &did_v2);

    let stored = client.get_did(&provider).unwrap();
    assert_eq!(stored, did_v2);
}

#[test]
fn test_grant_access_grantor_not_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let unregistered = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.register_entity(
        &doctor,
        &EntityType::Doctor,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "metadata"),
    );

    let result = client.try_grant_access(
        &unregistered,
        &doctor,
        &String::from_str(&env, "resource-1"),
        &0,
    );
    assert_eq!(result, Err(Ok(ContractError::GrantorNotRegistered)));
}

#[test]
fn test_grant_access_grantee_not_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let unregistered = Address::generate(&env);

    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    let result = client.try_grant_access(
        &hospital,
        &unregistered,
        &String::from_str(&env, "resource-1"),
        &0,
    );
    assert_eq!(result, Err(Ok(ContractError::GranteeNotRegistered)));
}

#[test]
fn test_grant_access_already_granted() {
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

    let resource = String::from_str(&env, "patient-records");
    client.grant_access(&hospital, &doctor, &resource, &0);

    let result = client.try_grant_access(&hospital, &doctor, &resource, &0);
    assert_eq!(result, Err(Ok(ContractError::AccessAlreadyGranted)));
}

#[test]
fn test_revoke_access_permission_not_found() {
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

    let result = client.try_revoke_access(
        &hospital,
        &doctor,
        &String::from_str(&env, "nonexistent-resource"),
    );
    assert_eq!(result, Err(Ok(ContractError::AccessPermissionNotFound)));
}

#[test]
fn test_revoke_access_not_authorized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let doctor = Address::generate(&env);
    let other = Address::generate(&env);

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
    client.register_entity(
        &other,
        &EntityType::Doctor,
        &String::from_str(&env, "Dr. Other"),
        &String::from_str(&env, "metadata"),
    );

    let resource = String::from_str(&env, "patient-records");
    client.grant_access(&hospital, &doctor, &resource, &0);

    // `other` is not the grantor and not the admin
    let result = client.try_revoke_access(&other, &doctor, &resource);
    assert_eq!(result, Err(Ok(ContractError::NotAuthorizedToRevoke)));
}
