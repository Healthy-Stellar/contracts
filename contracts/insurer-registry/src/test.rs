#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, BytesN, Env, String};

fn dummy_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn register_insurer_with_anchor(
    env: &Env,
    client: &InsurerRegistryClient<'_>,
    insurer_wallet: &Address,
) {
    let issuer = Address::generate(env);
    client.register_insurer(
        insurer_wallet,
        &String::from_str(env, "HealthGuard Insurance"),
        &String::from_str(env, "INS-2026-12345"),
        &String::from_str(env, "Full medical coverage provider"),
        &issuer,
        &dummy_hash(env, 1),
        &dummy_hash(env, 2),
        &4_100_000_000_u64,
        &dummy_hash(env, 3),
    );
}

#[test]
fn test_register_insurer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.name, String::from_str(&env, "HealthGuard Insurance"));
    assert_eq!(insurer.license_id, String::from_str(&env, "INS-2026-12345"));
    assert_eq!(insurer.metadata, String::from_str(&env, "Full medical coverage provider"));
    assert_eq!(insurer.credential.credential_hash, dummy_hash(&env, 1));
}

#[test]
fn test_duplicate_registration() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);
    let result = client.try_register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Full medical coverage"),
        &issuer,
        &dummy_hash(&env, 4),
        &dummy_hash(&env, 5),
        &4_100_000_000_u64,
        &dummy_hash(&env, 6),
    );
    assert!(matches!(result, Err(Ok(ContractError::AlreadyRegistered))));
}

#[test]
fn test_update_insurer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);

    let new_metadata = String::from_str(&env, "Premium coverage with dental and vision");
    client.update_insurer(&insurer_wallet, &new_metadata);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.metadata, new_metadata);
    assert_eq!(insurer.name, String::from_str(&env, "HealthGuard Insurance"));
}

#[test]
fn test_update_nonexistent_insurer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let metadata = String::from_str(&env, "Updated metadata");
    env.mock_all_auths();

    let result = client.try_update_insurer(&insurer_wallet, &metadata);
    assert!(matches!(result, Err(Ok(ContractError::InsurerNotFound))));
}

#[test]
fn test_get_nonexistent_insurer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let result = client.try_get_insurer(&insurer_wallet);
    assert!(matches!(result, Err(Ok(ContractError::InsurerNotFound))));
}

#[test]
fn test_update_contact_details() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);

    let contact_details = String::from_str(&env, "phone: 555-0123, email: contact@healthguard.com");
    client.update_contact_details(&insurer_wallet, &contact_details);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.contact_details, contact_details);
}

#[test]
fn test_update_coverage_policies() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);

    let coverage = String::from_str(&env, "Medical: 80%, Dental: 50%, Vision: 100%");
    client.update_coverage_policies(&insurer_wallet, &coverage);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.coverage_policies, coverage);
}

#[test]
fn test_add_claims_reviewer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 1);
    assert_eq!(reviewers.get(0).unwrap(), reviewer_wallet);
}

#[test]
fn test_add_multiple_claims_reviewers() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewer3 = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer1);
    client.add_claims_reviewer(&insurer_wallet, &reviewer2);
    client.add_claims_reviewer(&insurer_wallet, &reviewer3);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 3);
}

#[test]
fn test_add_duplicate_reviewer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    let result = client.try_add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    assert!(matches!(result, Err(Ok(ContractError::ReviewerAlreadyAuthorized))));
}

#[test]
fn test_add_reviewer_to_nonexistent_insurer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    let result = client.try_add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    assert!(matches!(result, Err(Ok(ContractError::InsurerNotFound))));
}

#[test]
fn test_remove_claims_reviewer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    client.remove_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 0);
}

#[test]
fn test_remove_nonexistent_reviewer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);

    let result = client.try_remove_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    assert!(matches!(result, Err(Ok(ContractError::ReviewerNotFound))));
}

#[test]
fn test_is_authorized_reviewer() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    let unauthorized_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    assert!(client.is_authorized_reviewer(&insurer_wallet, &reviewer_wallet));
    assert!(!client.is_authorized_reviewer(&insurer_wallet, &unauthorized_wallet));
}

#[test]
fn test_get_claims_reviewers_empty() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 0);
}

#[test]
fn test_expired_insurer_anchor_disables_membership() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 100);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Coverage info"),
        &issuer,
        &dummy_hash(&env, 1),
        &dummy_hash(&env, 2),
        &150_u64,
        &dummy_hash(&env, 3),
    );
    assert!(client.is_insurer_active(&insurer_wallet));

    env.ledger().with_mut(|li| li.timestamp = 151);
    assert!(!client.is_insurer_active(&insurer_wallet));

    let result = client.try_update_contact_details(
        &insurer_wallet,
        &String::from_str(&env, "phone: 555-0123"),
    );
    assert!(matches!(result, Err(Ok(ContractError::CredentialExpired))));
}

#[test]
fn test_full_workflow() {
    let env = Env::default();
    let contract_id = env.register(InsurerRegistry, ());
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &insurer_wallet);

    let contact = String::from_str(&env, "555-0123, contact@healthguard.com");
    client.update_contact_details(&insurer_wallet, &contact);

    let coverage = String::from_str(&env, "Medical: 100%, Dental: 80%");
    client.update_coverage_policies(&insurer_wallet, &coverage);

    client.add_claims_reviewer(&insurer_wallet, &reviewer1);
    client.add_claims_reviewer(&insurer_wallet, &reviewer2);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.name, String::from_str(&env, "HealthGuard Insurance"));
    assert_eq!(insurer.license_id, String::from_str(&env, "INS-2026-12345"));
    assert_eq!(insurer.contact_details, contact);
    assert_eq!(insurer.coverage_policies, coverage);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 2);

    client.remove_claims_reviewer(&insurer_wallet, &reviewer1);
    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 1);
    assert_eq!(reviewers.get(0).unwrap(), reviewer2);
}
