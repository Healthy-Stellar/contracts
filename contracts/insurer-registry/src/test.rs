#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, BytesN, Env, String};

fn dummy_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn create_registry(env: &Env) -> (Address, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let contract_id = env.register(InsurerRegistry, (admin.clone(),));
    (admin, contract_id)
}

fn register_insurer_with_anchor(
    env: &Env,
    client: &InsurerRegistryClient<'_>,
    admin: &Address,
    insurer_wallet: &Address,
) {
    let issuer = Address::generate(env);
    client.set_trusted_issuer(admin, &issuer, &true);
    client.register_insurer(
        insurer_wallet,
        &String::from_str(env, "HealthGuard Insurance"),
        &String::from_str(env, "INS-2026-12345"),
        &String::from_str(env, "Full medical coverage provider"),
        &dummy_hash(env, 1),
        &issuer,
        &dummy_hash(env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(env, 3),
    );
}

#[test]
fn test_register_insurer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.name, String::from_str(&env, "HealthGuard Insurance"));
    assert_eq!(insurer.license_id, String::from_str(&env, "INS-2026-12345"));
    assert_eq!(insurer.metadata, String::from_str(&env, "Full medical coverage provider"));
    assert_eq!(insurer.credential.credential_hash, dummy_hash(&env, 1));
}

#[test]
fn test_duplicate_registration() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);
    client.set_trusted_issuer(&admin, &issuer, &true);
    let result = client.try_register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Full medical coverage"),
        &dummy_hash(&env, 4),
        &issuer,
        &dummy_hash(&env, 5),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 6),
    );
    assert!(matches!(result, Err(Ok(Error::InsurerAlreadyRegistered))));
}

#[test]
fn test_update_insurer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);

    let new_metadata = String::from_str(&env, "Premium coverage with dental and vision");
    client.update_insurer(&insurer_wallet, &new_metadata);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.metadata, new_metadata);
    assert_eq!(insurer.name, String::from_str(&env, "HealthGuard Insurance"));
}

#[test]
fn test_update_nonexistent_insurer() {
    let env = Env::default();
    let (_admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let metadata = String::from_str(&env, "Updated metadata");
    env.mock_all_auths();

    let result = client.try_update_insurer(&insurer_wallet, &metadata);
    assert!(matches!(result, Err(Ok(Error::InsurerNotFound))));
}

#[test]
fn test_get_nonexistent_insurer() {
    let env = Env::default();
    let (_admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let result = client.try_get_insurer(&insurer_wallet);
    assert!(matches!(result, Err(Ok(Error::InsurerNotFound))));
}

#[test]
fn test_update_contact_details() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);

    let contact_details = String::from_str(&env, "phone: 555-0123, email: contact@healthguard.com");
    client.update_contact_details(&insurer_wallet, &contact_details);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.contact_details, contact_details);
}

#[test]
fn test_update_coverage_policies() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);

    let coverage = String::from_str(&env, "Medical: 80%, Dental: 50%, Vision: 100%");
    client.update_coverage_policies(&insurer_wallet, &coverage);

    let insurer = client.get_insurer(&insurer_wallet);
    assert_eq!(insurer.coverage_policies, coverage);
}

#[test]
fn test_add_claims_reviewer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 1);
    assert_eq!(reviewers.get(0).unwrap(), reviewer_wallet);
}

#[test]
fn test_add_multiple_claims_reviewers() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let reviewer3 = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer1);
    client.add_claims_reviewer(&insurer_wallet, &reviewer2);
    client.add_claims_reviewer(&insurer_wallet, &reviewer3);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 3);
}

#[test]
fn test_add_duplicate_reviewer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    let result = client.try_add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    assert!(matches!(result, Err(Ok(Error::ReviewerAlreadyAuthorized))));
}

#[test]
fn test_add_reviewer_to_nonexistent_insurer() {
    let env = Env::default();
    let (_admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    let result = client.try_add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    assert!(matches!(result, Err(Ok(Error::InsurerNotFound))));
}

#[test]
fn test_remove_claims_reviewer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    client.remove_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 0);
}

#[test]
fn test_remove_nonexistent_reviewer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);

    let result = client.try_remove_claims_reviewer(&insurer_wallet, &reviewer_wallet);
    assert!(matches!(result, Err(Ok(Error::ReviewerNotFound))));
}

#[test]
fn test_is_authorized_reviewer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer_wallet = Address::generate(&env);
    let unauthorized_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);
    client.add_claims_reviewer(&insurer_wallet, &reviewer_wallet);

    assert!(client.is_authorized_reviewer(&insurer_wallet, &reviewer_wallet));
    assert!(!client.is_authorized_reviewer(&insurer_wallet, &unauthorized_wallet));
}

#[test]
fn test_get_claims_reviewers_empty() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);

    let reviewers = client.get_claims_reviewers(&insurer_wallet);
    assert_eq!(reviewers.len(), 0);
}

#[test]
fn test_expired_insurer_anchor_disables_membership() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 100);
    client.set_trusted_issuer(&admin, &issuer, &true);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Coverage info"),
        &dummy_hash(&env, 1),
        &issuer,
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
    assert!(matches!(result, Err(Ok(Error::NotAuthorized))));
}

#[test]
fn test_full_workflow() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    env.mock_all_auths();

    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);

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

#[test]
fn test_coverage_plans_round_trip() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    env.mock_all_auths();
    register_insurer_with_anchor(&env, &client, &admin, &insurer_wallet);

    let mut service_codes = soroban_sdk::Vec::new(&env);
    service_codes.push_back(String::from_str(&env, "CPT99213"));
    let mut plans = soroban_sdk::Vec::new(&env);
    plans.push_back(CoveragePlan {
        plan_id: 1,
        plan_name: String::from_str(&env, "PPO Gold"),
        service_codes,
        is_active: true,
        effective_from: 0,
        effective_until: None,
    });

    client.set_coverage_plans(&insurer_wallet, &plans);
    let loaded = client.get_coverage_plans(&insurer_wallet);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.get(0).unwrap().plan_name, String::from_str(&env, "PPO Gold"));
}

#[test]
fn test_set_coverage_plans_rejects_revoked_insurer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();
    client.set_trusted_issuer(&admin, &issuer, &true);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Coverage info"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 3),
    );

    // Revoke the insurer's credential
    client.revoke_credential(&admin, &insurer_wallet, &symbol_short!("fraud"));

    // Attempt to set coverage plans should fail
    let mut plans = soroban_sdk::Vec::new(&env);
    plans.push_back(CoveragePlan {
        plan_id: 1,
        plan_name: String::from_str(&env, "PPO Gold"),
        service_codes: soroban_sdk::Vec::new(&env),
        is_active: true,
        effective_from: 0,
        effective_until: None,
    });

    let result = client.try_set_coverage_plans(&insurer_wallet, &plans);
    assert!(matches!(result, Err(Ok(Error::NotAuthorized))));
}

#[test]
fn test_set_coverage_plans_rejects_expired_insurer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 100);
    client.set_trusted_issuer(&admin, &issuer, &true);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Coverage info"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &150_u64,
        &dummy_hash(&env, 3),
    );

    // Advance time past expiration
    env.ledger().with_mut(|li| li.timestamp = 151);

    // Attempt to set coverage plans should fail
    let mut plans = soroban_sdk::Vec::new(&env);
    plans.push_back(CoveragePlan {
        plan_id: 1,
        plan_name: String::from_str(&env, "PPO Gold"),
        service_codes: soroban_sdk::Vec::new(&env),
        is_active: true,
        effective_from: 0,
        effective_until: None,
    });

    let result = client.try_set_coverage_plans(&insurer_wallet, &plans);
    assert!(matches!(result, Err(Ok(Error::NotAuthorized))));
}

#[test]
fn test_add_coverage_plan_rejects_revoked_insurer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();
    client.set_trusted_issuer(&admin, &issuer, &true);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Coverage info"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 3),
    );

    // Revoke the insurer's credential
    client.revoke_credential(&admin, &insurer_wallet, &symbol_short!("fraud"));

    // Attempt to add coverage plan should fail
    let mut service_codes = soroban_sdk::Vec::new(&env);
    service_codes.push_back(String::from_str(&env, "CPT99213"));

    let result = client.try_add_coverage_plan(
        &insurer_wallet,
        &String::from_str(&env, "PPO Gold"),
        &service_codes,
        &true,
        &0_u64,
        &None,
    );
    assert!(matches!(result, Err(Ok(Error::NotAuthorized))));
}

#[test]
fn test_add_coverage_plan_rejects_expired_insurer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 100);
    client.set_trusted_issuer(&admin, &issuer, &true);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Coverage info"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &150_u64,
        &dummy_hash(&env, 3),
    );

    // Advance time past expiration
    env.ledger().with_mut(|li| li.timestamp = 151);

    // Attempt to add coverage plan should fail
    let mut service_codes = soroban_sdk::Vec::new(&env);
    service_codes.push_back(String::from_str(&env, "CPT99213"));

    let result = client.try_add_coverage_plan(
        &insurer_wallet,
        &String::from_str(&env, "PPO Gold"),
        &service_codes,
        &true,
        &0_u64,
        &None,
    );
    assert!(matches!(result, Err(Ok(Error::NotAuthorized))));
}

#[test]
fn test_add_claims_reviewers_batch_rejects_revoked_insurer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    env.mock_all_auths();
    client.set_trusted_issuer(&admin, &issuer, &true);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Coverage info"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 3),
    );

    // Revoke the insurer's credential
    client.revoke_credential(&admin, &insurer_wallet, &symbol_short!("fraud"));

    // Attempt to add batch of reviewers should fail
    let mut reviewers = soroban_sdk::Vec::new(&env);
    reviewers.push_back(reviewer1);
    reviewers.push_back(reviewer2);

    let result = client.try_add_claims_reviewers_batch(&insurer_wallet, &reviewers);
    assert!(matches!(result, Err(Ok(Error::NotAuthorized))));
}

#[test]
fn test_add_claims_reviewers_batch_rejects_expired_insurer() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);

    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 100);
    client.set_trusted_issuer(&admin, &issuer, &true);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "HealthGuard Insurance"),
        &String::from_str(&env, "INS-2026-12345"),
        &String::from_str(&env, "Coverage info"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &150_u64,
        &dummy_hash(&env, 3),
    );

    // Advance time past expiration
    env.ledger().with_mut(|li| li.timestamp = 151);

    // Attempt to add batch of reviewers should fail
    let mut reviewers = soroban_sdk::Vec::new(&env);
    reviewers.push_back(reviewer1);
    reviewers.push_back(reviewer2);

    let result = client.try_add_claims_reviewers_batch(&insurer_wallet, &reviewers);
    assert!(matches!(result, Err(Ok(Error::NotAuthorized))));
}

#[test]
fn test_untrusted_issuer_cannot_register_insurer() {
    let env = Env::default();
    let (_admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);
    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    let attacker = Address::generate(&env);

    assert!(matches!(
        client.try_set_trusted_issuer(&attacker, &issuer, &true),
        Err(Ok(Error::NotAuthorized))
    ));

    let result = client.try_register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "Untrusted Insurer"),
        &String::from_str(&env, "LIC-UNTRUSTED"),
        &String::from_str(&env, "metadata"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 3),
    );

    assert!(matches!(result, Err(Ok(Error::NotAuthorized))));
    assert!(!client.is_insurer_active(&insurer_wallet));
}

#[test]
fn test_credential_expiry_is_capped_at_two_years() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);
    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    client.set_trusted_issuer(&admin, &issuer, &true);

    let result = client.try_register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "Long-lived Insurer"),
        &String::from_str(&env, "LIC-LONG"),
        &String::from_str(&env, "metadata"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME + 1),
        &dummy_hash(&env, 3),
    );

    assert!(matches!(result, Err(Ok(Error::InvalidCredentialExpiry))));
}

#[test]
fn test_only_admin_can_revoke_and_renewal_reactivates_credential() {
    let env = Env::default();
    let (admin, contract_id) = create_registry(&env);
    let client = InsurerRegistryClient::new(&env, &contract_id);
    let insurer_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    client.set_trusted_issuer(&admin, &issuer, &true);
    env.ledger().with_mut(|li| li.timestamp = 100);

    client.register_insurer(
        &insurer_wallet,
        &String::from_str(&env, "Renewable Insurer"),
        &String::from_str(&env, "LIC-RENEW"),
        &String::from_str(&env, "metadata"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &150_u64,
        &dummy_hash(&env, 3),
    );

    assert!(matches!(
        client.try_revoke_credential(&issuer, &insurer_wallet, &symbol_short!("fraud")),
        Err(Ok(Error::NotAuthorized))
    ));
    client.revoke_credential(&admin, &insurer_wallet, &symbol_short!("fraud"));
    assert!(!client.is_insurer_active(&insurer_wallet));

    env.ledger().with_mut(|li| li.timestamp = 151);
    client.renew_credential(
        &insurer_wallet,
        &issuer,
        &dummy_hash(&env, 4),
        &dummy_hash(&env, 5),
        &(env.ledger().timestamp() + MAX_CREDENTIAL_LIFETIME),
        &dummy_hash(&env, 6),
    );
    assert!(client.is_insurer_active(&insurer_wallet));
}
