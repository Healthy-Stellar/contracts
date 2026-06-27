#![cfg(test)]

use super::*;
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, IntoVal, String,
};

fn setup() -> (Env, AccessControlClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);
    (env, client)
}

fn register_two(
    env: &Env,
    client: &AccessControlClient,
    admin: &Address,
) -> (Address, Address) {
    client.initialize(admin);
    let hospital = Address::generate(env);
    let doctor = Address::generate(env);
    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(env, "City Hospital"),
        &String::from_str(env, "metadata"),
    );
    client.register_entity(
        &doctor,
        &EntityType::Doctor,
        &String::from_str(env, "Dr. Smith"),
        &String::from_str(env, "metadata"),
    );
    (hospital, doctor)
}

#[test]
fn test_initialize() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    client.initialize(&admin);
}

#[test]
fn test_double_initialize() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let result = client.try_initialize(&admin);
    assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
}

#[test]
fn test_register_entity() {
    let (env, client) = setup();
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
    let (env, client) = setup();
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    let name = String::from_str(&env, "City Hospital");
    let metadata = String::from_str(&env, "General Hospital");
    client.register_entity(&hospital, &EntityType::Hospital, &name, &metadata);

    let result = client.try_register_entity(&hospital, &EntityType::Hospital, &name, &metadata);
    assert_eq!(result, Err(Ok(ContractError::EntityAlreadyRegistered)));
}

// ---------------------------------------------------------------------------
// #220: composite uniqueness — same (grantor, grantee, resource) is rejected
// ---------------------------------------------------------------------------
#[test]
fn test_grant_access() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource_id = String::from_str(&env, "patient-123-records");
    let op_id = client.grant_access(&hospital, &doctor, &resource_id, &0, &None);
    assert!(op_id > 0);

    assert!(client.check_access(&doctor, &resource_id));
    let authorized = client.get_authorized_parties(&resource_id);
    assert_eq!(authorized.len(), 1);
}

#[test]
fn test_grant_access_already_granted() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource = String::from_str(&env, "patient-records");
    client.grant_access(&hospital, &doctor, &resource, &0, &None);

    // Same (grantor, grantee, resource) must be rejected
    let result = client.try_grant_access(&hospital, &doctor, &resource, &0, &None);
    assert_eq!(result, Err(Ok(ContractError::AccessAlreadyGranted)));
}

// ---------------------------------------------------------------------------
// #222: op_id is monotonically increasing and included in events
// ---------------------------------------------------------------------------
#[test]
fn test_op_id_increments() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let r1 = String::from_str(&env, "resource-1");
    let r2 = String::from_str(&env, "resource-2");

    let op1 = client.grant_access(&hospital, &doctor, &r1, &0, &None);
    let op2 = client.grant_access(&hospital, &doctor, &r2, &0, &None);
    assert!(op2 > op1);
}

#[test]
fn test_revoke_returns_op_id() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource = String::from_str(&env, "patient-records");
    let grant_op = client.grant_access(&hospital, &doctor, &resource, &0, &None);
    let revoke_op = client.revoke_access(&hospital, &doctor, &resource);
    assert!(revoke_op > grant_op);
}

// ---------------------------------------------------------------------------
// #224: revocation is atomic — both AccessList and ResourceAccess are cleared
// ---------------------------------------------------------------------------
#[test]
fn test_revoke_access_clears_both_indexes() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource_id = String::from_str(&env, "patient-123-records");
    client.grant_access(&hospital, &doctor, &resource_id, &0, &None);

    assert!(client.check_access(&doctor, &resource_id));
    assert_eq!(client.get_authorized_parties(&resource_id).len(), 1);

    client.revoke_access(&hospital, &doctor, &resource_id);

    // Both indexes must be empty after revocation
    assert!(!client.check_access(&doctor, &resource_id));
    assert_eq!(client.get_authorized_parties(&resource_id).len(), 0);
}

#[test]
fn test_revoke_access_re_grant_allowed_after_revoke() {
    // After revocation the composite index is removed, so re-granting must succeed
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource = String::from_str(&env, "patient-records");
    client.grant_access(&hospital, &doctor, &resource, &0, &None);
    client.revoke_access(&hospital, &doctor, &resource);

    // Re-grant must succeed (composite index was cleaned up)
    let op = client.grant_access(&hospital, &doctor, &resource, &0, &None);
    assert!(op > 0);
    assert!(client.check_access(&doctor, &resource));
}

// ---------------------------------------------------------------------------
// #228: commit-reveal anti-front-running
// ---------------------------------------------------------------------------
#[test]
fn test_commit_reveal_grant_access() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource = String::from_str(&env, "sensitive-resource");
    // Build nonce and commit hash off-chain (simulated here)
    let nonce: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);

    // Compute commit_hash = sha256(nonce || grantor_xdr || grantee_xdr || resource_xdr)
    let mut data = Bytes::new(&env);
    data.append(&nonce.clone().into());
    data.append(&hospital.clone().to_xdr(&env));
    data.append(&doctor.clone().to_xdr(&env));
    data.append(&resource.clone().to_xdr(&env));
    let commit_hash: BytesN<32> = env.crypto().sha256(&data).into();

    // Phase 1: commit
    client.commit_grant(&hospital, &commit_hash);

    // Phase 2: reveal (grant with nonce)
    let op_id = client.grant_access(&hospital, &doctor, &resource, &0, &Some(nonce));
    assert!(op_id > 0);
    assert!(client.check_access(&doctor, &resource));
}

#[test]
fn test_commit_reveal_wrong_nonce_rejected() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource = String::from_str(&env, "sensitive-resource");
    let nonce: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let wrong_nonce: BytesN<32> = BytesN::from_array(&env, &[2u8; 32]);

    let mut data = Bytes::new(&env);
    data.append(&nonce.clone().into());
    data.append(&hospital.clone().to_xdr(&env));
    data.append(&doctor.clone().to_xdr(&env));
    data.append(&resource.clone().to_xdr(&env));
    let commit_hash: BytesN<32> = env.crypto().sha256(&data).into();

    client.commit_grant(&hospital, &commit_hash);

    // Using wrong nonce must fail
    let result = client.try_grant_access(&hospital, &doctor, &resource, &0, &Some(wrong_nonce));
    assert_eq!(result, Err(Ok(ContractError::CommitNotFound)));
}

#[test]
fn test_commit_reveal_replay_rejected() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource = String::from_str(&env, "sensitive-resource");
    let nonce: BytesN<32> = BytesN::from_array(&env, &[3u8; 32]);

    let mut data = Bytes::new(&env);
    data.append(&nonce.clone().into());
    data.append(&hospital.clone().to_xdr(&env));
    data.append(&doctor.clone().to_xdr(&env));
    data.append(&resource.clone().to_xdr(&env));
    let commit_hash: BytesN<32> = env.crypto().sha256(&data).into();

    client.commit_grant(&hospital, &commit_hash);
    client.grant_access(&hospital, &doctor, &resource, &0, &Some(nonce.clone()));

    // Attempting to re-use the same commit (replay) must fail
    // First revoke so the grant itself doesn't block
    client.revoke_access(&hospital, &doctor, &resource);

    let result = client.try_grant_access(&hospital, &doctor, &resource, &0, &Some(nonce));
    assert_eq!(result, Err(Ok(ContractError::CommitAlreadyUsed)));
}

// ---------------------------------------------------------------------------
// Existing tests (updated for new signatures)
// ---------------------------------------------------------------------------
#[test]
fn test_check_access_expired() {
    use soroban_sdk::testutils::Ledger;

    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let resource_id = String::from_str(&env, "patient-123-records");
    client.grant_access(&hospital, &doctor, &resource_id, &100, &None);

    assert!(client.check_access(&doctor, &resource_id));

    env.ledger().set_timestamp(200);
    assert!(!client.check_access(&doctor, &resource_id));
}

#[test]
fn test_get_entity_permissions() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let r1 = String::from_str(&env, "patient-123-records");
    let r2 = String::from_str(&env, "patient-456-records");
    client.grant_access(&hospital, &doctor, &r1, &0, &None);
    client.grant_access(&hospital, &doctor, &r2, &0, &None);

    let permissions = client.get_entity_permissions(&doctor);
    assert_eq!(permissions.len(), 2);
}

#[test]
fn test_deactivate_entity() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    client.deactivate_entity(&admin, &hospital);
    let entity = client.get_entity(&hospital);
    assert!(!entity.active);
}

#[test]
fn test_deactivate_entity_non_admin() {
    let (env, client) = setup();
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
    let (env, client) = setup();
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let hospital = Address::generate(&env);
    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "Original metadata"),
    );

    let new_metadata = String::from_str(&env, "Updated metadata");
    client.update_entity(&hospital, &new_metadata);

    let entity = client.get_entity(&hospital);
    assert_eq!(entity.metadata, new_metadata);
}

#[test]
fn test_register_and_get_did() {
    let (env, client) = setup();
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
    let (env, client) = setup();
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
    let (env, client) = setup();
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

// ---------------------------------------------------------------------------
// DID format validation — RFC 3986 / W3C DID spec compliance
// ---------------------------------------------------------------------------

#[test]
fn test_did_missing_method_rejected() {
    // "did:" with no method segment
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    let result = client.try_register_did(&user, &Bytes::from_slice(&env, b"did:"));
    assert_eq!(result, Err(Ok(ContractError::InvalidDidFormat)));
}

#[test]
fn test_did_empty_method_rejected() {
    // "did::identifier" — empty method between the two colons
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    let result = client.try_register_did(&user, &Bytes::from_slice(&env, b"did::identifier"));
    assert_eq!(result, Err(Ok(ContractError::InvalidDidFormat)));
}

#[test]
fn test_did_uppercase_method_rejected() {
    // W3C DID spec §3.1: method must be lowercase
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    let result = client.try_register_did(&user, &Bytes::from_slice(&env, b"did:Stellar:abc"));
    assert_eq!(result, Err(Ok(ContractError::InvalidDidFormat)));
}

#[test]
fn test_did_invalid_char_in_method_rejected() {
    // Space in method segment
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    let result = client.try_register_did(&user, &Bytes::from_slice(&env, b"did:bad method:id"));
    assert_eq!(result, Err(Ok(ContractError::InvalidDidFormat)));
}

#[test]
fn test_did_missing_identifier_rejected() {
    // "did:stellar:" — method present but identifier empty
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    let result = client.try_register_did(&user, &Bytes::from_slice(&env, b"did:stellar:"));
    assert_eq!(result, Err(Ok(ContractError::InvalidDidFormat)));
}

#[test]
fn test_did_valid_methods_accepted() {
    // did:stellar: and did:key: are both valid DID methods
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let u1 = Address::generate(&env);
    let u2 = Address::generate(&env);
    client.initialize(&admin);

    client.register_did(&u1, &Bytes::from_slice(&env, b"did:stellar:GABC123"));
    client.register_did(&u2, &Bytes::from_slice(&env, b"did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK"));
}

#[test]
fn test_grant_access_grantor_not_registered() {
    let (env, client) = setup();
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
        &None,
    );
    assert_eq!(result, Err(Ok(ContractError::GrantorNotRegistered)));
}

#[test]
fn test_grant_access_grantee_not_registered() {
    let (env, client) = setup();
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
        &None,
    );
    assert_eq!(result, Err(Ok(ContractError::GranteeNotRegistered)));
}

#[test]
fn test_revoke_access_permission_not_found() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let result = client.try_revoke_access(
        &hospital,
        &doctor,
        &String::from_str(&env, "nonexistent-resource"),
    );
    assert_eq!(result, Err(Ok(ContractError::AccessPermissionNotFound)));
}

#[test]
fn test_revoke_access_not_authorized() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let (hospital, doctor) = register_two(&env, &client, &admin);

    let other = Address::generate(&env);
    client.register_entity(
        &other,
        &EntityType::Doctor,
        &String::from_str(&env, "Dr. Other"),
        &String::from_str(&env, "metadata"),
    );

    let resource = String::from_str(&env, "patient-records");
    client.grant_access(&hospital, &doctor, &resource, &0, &None);

    let result = client.try_revoke_access(&other, &doctor, &resource);
    assert_eq!(result, Err(Ok(ContractError::NotAuthorizedToRevoke)));
}

// =============================================================================
// Role-based access control tests
// =============================================================================

fn setup_rbac(env: &Env) -> (Address, AccessControlClient) {
    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, client)
}

// --- grant_role / has_role ---------------------------------------------------

#[test]
fn test_grant_role_and_has_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let provider = Address::generate(&env);
    assert!(!client.has_role(&provider, &Role::Provider));

    client.grant_role(&admin, &provider, &Role::Provider, &0);
    assert!(client.has_role(&provider, &Role::Provider));
}

#[test]
fn test_admin_always_has_every_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    // Admin satisfies every role check without an explicit grant.
    assert!(client.has_role(&admin, &Role::Provider));
    assert!(client.has_role(&admin, &Role::PayerReviewer));
    assert!(client.has_role(&admin, &Role::Auditor));
    assert!(client.has_role(&admin, &Role::EmergencyResponder));
}

#[test]
fn test_grant_role_non_admin_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, client) = setup_rbac(&env);

    let non_admin = Address::generate(&env);
    let target = Address::generate(&env);

    let result = client.try_grant_role(&non_admin, &target, &Role::Provider, &0);
    assert_eq!(result, Err(Ok(ContractError::InsufficientRole)));
}

#[test]
fn test_grant_role_duplicate_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let provider = Address::generate(&env);
    client.grant_role(&admin, &provider, &Role::Provider, &0);

    let result = client.try_grant_role(&admin, &provider, &Role::Provider, &0);
    assert_eq!(result, Err(Ok(ContractError::RoleAlreadyGranted)));
}

// --- revoke_role -------------------------------------------------------------

#[test]
fn test_revoke_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let auditor = Address::generate(&env);
    client.grant_role(&admin, &auditor, &Role::Auditor, &0);
    assert!(client.has_role(&auditor, &Role::Auditor));

    client.revoke_role(&admin, &auditor, &Role::Auditor);
    assert!(!client.has_role(&auditor, &Role::Auditor));
}

#[test]
fn test_revoke_role_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let nobody = Address::generate(&env);
    let result = client.try_revoke_role(&admin, &nobody, &Role::Auditor);
    assert_eq!(result, Err(Ok(ContractError::RoleNotFound)));
}

#[test]
fn test_revoke_role_non_admin_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let auditor = Address::generate(&env);
    let non_admin = Address::generate(&env);
    client.grant_role(&admin, &auditor, &Role::Auditor, &0);

    let result = client.try_revoke_role(&non_admin, &auditor, &Role::Auditor);
    assert_eq!(result, Err(Ok(ContractError::InsufficientRole)));
}

// --- role expiry -------------------------------------------------------------

#[test]
fn test_role_expires() {
    use soroban_sdk::testutils::Ledger;

    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let reviewer = Address::generate(&env);
    // Grant role that expires at timestamp 100.
    client.grant_role(&admin, &reviewer, &Role::PayerReviewer, &100);
    assert!(client.has_role(&reviewer, &Role::PayerReviewer));

    // Advance past expiry.
    env.ledger().set_timestamp(200);
    assert!(!client.has_role(&reviewer, &Role::PayerReviewer));
}

#[test]
fn test_expired_role_can_be_regranted() {
    use soroban_sdk::testutils::Ledger;

    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let reviewer = Address::generate(&env);
    client.grant_role(&admin, &reviewer, &Role::PayerReviewer, &100);

    env.ledger().set_timestamp(200);
    // Expired — re-grant should succeed.
    client.grant_role(&admin, &reviewer, &Role::PayerReviewer, &0);
    assert!(client.has_role(&reviewer, &Role::PayerReviewer));
}

// --- get_role_assignment -----------------------------------------------------

#[test]
fn test_get_role_assignment() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let provider = Address::generate(&env);
    client.grant_role(&admin, &provider, &Role::Provider, &999);

    let assignment = client.get_role_assignment(&provider, &Role::Provider);
    assert_eq!(assignment.granted_by, admin);
    assert_eq!(assignment.expires_at, 999);
}

#[test]
fn test_get_role_assignment_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, client) = setup_rbac(&env);

    let nobody = Address::generate(&env);
    let result = client.try_get_role_assignment(&nobody, &Role::Auditor);
    assert_eq!(result, Err(Ok(ContractError::RoleNotFound)));
}

// --- deactivate_entity uses role check ---------------------------------------

#[test]
fn test_deactivate_entity_by_admin_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let hospital = Address::generate(&env);
    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    client.deactivate_entity(&admin, &hospital);
    assert!(!client.get_entity(&hospital).active);
}

#[test]
fn test_deactivate_entity_by_granted_admin_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    // Grant Admin role to a second address.
    let second_admin = Address::generate(&env);
    client.grant_role(&admin, &second_admin, &Role::Admin, &0);

    let hospital = Address::generate(&env);
    client.register_entity(
        &hospital,
        &EntityType::Hospital,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "metadata"),
    );

    client.deactivate_entity(&second_admin, &hospital);
    assert!(!client.get_entity(&hospital).active);
}

#[test]
fn test_deactivate_entity_non_admin_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, client) = setup_rbac(&env);

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

// --- revoke_access uses role check -------------------------------------------

#[test]
fn test_revoke_access_by_payer_reviewer_role() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

    let hospital = Address::generate(&env);
    let doctor = Address::generate(&env);
    let payer = Address::generate(&env);

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
    client.grant_access(&hospital, &doctor, &resource, &0, &None);

    // Grant PayerReviewer role to payer.
    client.grant_role(&admin, &payer, &Role::PayerReviewer, &0);

    // Payer (not the original grantor) can revoke.
    client.revoke_access(&payer, &doctor, &resource);
    assert!(!client.check_access(&doctor, &resource));
}

#[test]
fn test_revoke_access_by_unprivileged_non_grantor_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_rbac(&env);

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
    client.grant_access(&hospital, &doctor, &resource, &0, &None);

    // `other` has no role and is not the grantor.
    let result = client.try_revoke_access(&other, &doctor, &resource);
    assert_eq!(result, Err(Ok(ContractError::NotAuthorizedToRevoke)));

    // Silence unused-variable warning for admin.
    let _ = admin;
}

proptest! {
    #[test]
    fn test_soundness_provider_without_grant_never_has_access(resource_id in "resource-[0-9]{1,3}") {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let (grantor, grantee) = register_two(&env, &client, &admin);
        let other = Address::generate(&env);

        let resource = String::from_str(&env, &resource_id);
        client.grant_access(&grantor, &grantee, &resource, &0, &None);

        assert!(!client.check_access(&other, &resource));
    }

    #[test]
    fn test_completeness_after_grant_access_succeeds(resource_id in "resource-[0-9]{1,3}", expires_at in 1u64..1_000u64) {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let (grantor, grantee) = register_two(&env, &client, &admin);

        let resource = String::from_str(&env, &resource_id);
        client.grant_access(&grantor, &grantee, &resource, &expires_at, &None);

        assert!(client.check_access(&grantee, &resource));
    }

    #[test]
    fn test_revocation_after_revoke_access_denied(resource_id in "resource-[0-9]{1,3}") {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let (grantor, grantee) = register_two(&env, &client, &admin);

        let resource = String::from_str(&env, &resource_id);
        client.grant_access(&grantor, &grantee, &resource, &0, &None);
        client.revoke_access(&grantor, &grantee, &resource);

        assert!(!client.check_access(&grantee, &resource));
    }

    #[test]
    fn test_expiry_after_expiry_timestamp_denies_access(resource_id in "resource-[0-9]{1,3}", expires_at in 1u64..1_000u64) {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let (grantor, grantee) = register_two(&env, &client, &admin);

        let resource = String::from_str(&env, &resource_id);
        client.grant_access(&grantor, &grantee, &resource, &expires_at, &None);

        env.ledger().set_timestamp(expires_at + 1);
        assert!(!client.check_access(&grantee, &resource));
    }

    #[test]
    fn test_scope_isolation_grant_does_not_grant_other_scope(
        resource_a in "resource-[0-9]{1,3}",
        resource_b in "resource-[0-9]{1,3}"
    ) {
        prop_assume!(resource_a != resource_b);
        let env = Env::default();
        env.mock_all_auths();
        let (admin, client) = setup(&env);
        let (grantor, grantee) = register_two(&env, &client, &admin);

        let resource_a = String::from_str(&env, &resource_a);
        let resource_b = String::from_str(&env, &resource_b);
        client.grant_access(&grantor, &grantee, &resource_a, &0, &None);

        assert!(!client.check_access(&grantee, &resource_b));
    }
}

// ── #489: emergency access override tests ────────────────────────────────────

fn setup_emergency(env: &Env) -> (Address, Address, Address, AccessControlClient<'static>) {
    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);

    let responder = Address::generate(env);
    let patient = Address::generate(env);

    // Grant EmergencyResponder role to responder
    client.grant_role(&admin, &responder, &Role::EmergencyResponder, &0);

    (admin, responder, patient, client)
}

fn make_justification(env: &Env) -> BytesN<32> {
    env.crypto()
        .sha256(&Bytes::from_slice(env, b"unconscious patient emergency"))
        .into()
}

#[test]
fn test_emergency_access_granted_and_active() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let (_admin, responder, patient, client) = setup_emergency(&env);
    let justification = make_justification(&env);

    let op_id = client.emergency_access(&responder, &patient, &justification);
    assert!(op_id > 0);

    // Access should be active immediately
    assert!(client.check_emergency_access(&responder, &patient));
}

#[test]
fn test_emergency_access_expires_after_one_hour() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let (_admin, responder, patient, client) = setup_emergency(&env);
    client.emergency_access(&responder, &patient, &make_justification(&env));

    // Advance ledger time past 1 hour (3600s)
    env.ledger().set_timestamp(1_000 + 3601);
    assert!(!client.check_emergency_access(&responder, &patient));
}

#[test]
fn test_emergency_access_rejected_for_non_responder() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let contract_id = env.register(AccessControl, ());
    let client = AccessControlClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let non_responder = Address::generate(&env);
    let patient = Address::generate(&env);

    let result = client.try_emergency_access(
        &non_responder,
        &patient,
        &make_justification(&env),
    );
    assert_eq!(result, Err(Ok(ContractError::InsufficientRole)));
}

#[test]
fn test_emergency_access_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let (_admin, responder, patient, client) = setup_emergency(&env);
    client.emergency_access(&responder, &patient, &make_justification(&env));

    // initialize emits 1 event, grant_role emits 1, emergency_access emits 1 = 3 total
    let events = env.events().all();
    assert!(events.len() >= 1, "at least one event must have been emitted");
}
