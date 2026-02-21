#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env, String};

#[test]
fn test_register_insurer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Full medical coverage provider");

    // Mock authorization
    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);

    // Verify insurer was registered
    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.name, name);
    assert_eq!(insurer.license_id, license_id);
    assert_eq!(insurer.metadata, metadata);
}

#[test]
#[should_panic(expected = "Insurer already registered")]
fn test_duplicate_registration() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Full medical coverage");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);
    // Attempt to register again - should panic
    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);
}

#[test]
fn test_update_insurer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Basic coverage");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);

    // Update metadata
    let new_metadata = String::from_str(&env, "Premium coverage with dental and vision");
    client.update_insurer(&insurer_wallet, &new_metadata);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.metadata, new_metadata);
    assert_eq!(insurer.name, name); // Name should remain unchanged
}

#[test]
#[should_panic(expected = "Insurer not found")]
fn test_update_nonexistent_insurer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let metadata = String::from_str(&env, "Updated metadata");

    env.mock_all_auths();

    // Attempt to update non-existent insurer
    client.update_insurer(&insurer_wallet, &metadata);
}

#[test]
#[should_panic(expected = "Insurer not found")]
fn test_get_nonexistent_insurer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);

    // Attempt to get non-existent insurer
    client.get_insurer(&insurer_wallet);
}

#[test]
fn test_update_contact_details() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);

    // Update contact details
    let contact_details = String::from_str(&env, "phone: 555-0123, email: contact@healthguard.com");
    client.update_contact_details(&insurer_wallet, &contact_details);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.contact_details, contact_details);
}

#[test]
fn test_update_coverage_policies() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);

    // Update coverage policies
    let coverage = String::from_str(&env, "Medical: 80%, Dental: 50%, Vision: 100%");
    client.update_coverage_policies(&insurer_wallet, &coverage);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.coverage_policies, coverage);
}

// =====================================================
//         CLAIMS REVIEWERS TESTS
// =====================================================

#[test]
fn test_add_claims_reviewer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 1);
    assert_eq!(reviewers.get(0).unwrap(), reviewer_wallet);
}

#[test]
fn test_add_multiple_claims_reviewers() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewer3 = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);
    client.add_claims_reviewer(&insurer_wallet, &reviewer1);
    client.add_claims_reviewer(&insurer_wallet, &reviewer2);
    client.add_claims_reviewer(&insurer_wallet, &reviewer3);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 3);
}

#[test]
#[should_panic(expected = "Reviewer already authorized")]
fn test_add_duplicate_reviewer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    // Attempt to add same reviewer again - should panic
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
}

#[test]
#[should_panic(expected = "Insurer not registered")]
fn test_add_reviewer_to_nonexistent_insurer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);

    env.mock_all_auths();

    // Attempt to add reviewer to non-existent insurer
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
}

#[test]
fn test_remove_claims_reviewer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 1);

    // Remove the reviewer
    client.remove_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 0);
}

#[test]
#[should_panic(expected = "Reviewer not found")]
fn test_remove_nonexistent_reviewer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);

    // Attempt to remove a reviewer that was never added
    client.remove_claims_reviewer(&insurer_wallet, &reviewer_wallet);
}

#[test]
fn test_is_authorized_reviewer() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    let unauthorized_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    // Check authorized reviewer
    assert!(client.is_authorized_reviewer(&insurer_wallet, &reviewer_wallet));

    // Check unauthorized address
    assert!(!client.is_authorized_reviewer(&insurer_wallet, &unauthorized_wallet));
}

#[test]
fn test_get_claims_reviewers_empty() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Coverage info");

    env.mock_all_auths();

    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 0);
}

#[test]
fn test_full_workflow() {
    let env = Env::default();
    let contract_id = env.register_contract(None, InsurerRegistry);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);

    env.mock_all_auths();

    // Register insurer
    let name = String::from_str(&env, "HealthGuard Insurance");
    let license_id = String::from_str(&env, "INS-2026-12345");
    let metadata = String::from_str(&env, "Comprehensive health coverage");
    client.register_insurer(&insurer_wallet, &name, &license_id, &metadata);

    // Update contact details
    let contact = String::from_str(&env, "555-0123, contact@healthguard.com");
    client.update_contact_details(&insurer_wallet, &contact);

    // Update coverage policies
    let coverage = String::from_str(&env, "Medical: 100%, Dental: 80%");
    client.update_coverage_policies(&insurer_wallet, &coverage);

    // Add reviewers
    client.add_claims_reviewer(&insurer_wallet, &reviewer1);
    client.add_claims_reviewer(&insurer_wallet, &reviewer2);

    // Verify all data
    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.name, name);
    assert_eq!(insurer.license_id, license_id);
    assert_eq!(insurer.contact_details, contact);
    assert_eq!(insurer.coverage_policies, coverage);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 2);

    // Remove one reviewer
    client.remove_claims_reviewer(&insurer_wallet, &reviewer1);
    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 1);
    assert_eq!(reviewers.get(0).unwrap(), reviewer2);
}
