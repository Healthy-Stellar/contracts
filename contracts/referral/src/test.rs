#![cfg(test)]
#![allow(deprecated)]

use crate::contract::{ReferralContract, ReferralContractClient};
use crate::types::Error;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec, vec};
use provider_registry::{ProviderRegistry, ProviderRegistryClient};

/// Registers a `ProviderRegistry` contract, initializes it with `admin`, and
/// registers `referral_contract_id` against it, returning the registry
/// client so individual tests can register whichever provider addresses
/// they need via `register_provider`.
fn setup_provider_registry<'a>(
    env: &'a Env,
    admin: &Address,
    referral_contract_id: &Address,
) -> ProviderRegistryClient<'a> {
    let provider_registry_id = env.register(ProviderRegistry, ());
    let pr_client = ProviderRegistryClient::new(env, &provider_registry_id);
    pr_client.initialize(admin);

    let referral_client = ReferralContractClient::new(env, referral_contract_id);
    referral_client.initialize(admin, &provider_registry_id);

    pr_client
}

/// Registers `provider` as an active, non-expired provider in the given
/// `ProviderRegistry`, matching the shape expected by `register_provider`.
fn register_provider(env: &Env, pr_client: &ProviderRegistryClient, admin: &Address, provider: &Address) {
    pr_client.register_provider(
        admin,
        provider,
        &String::from_str(env, "Dr. Test"),
        &String::from_str(env, "General"),
        &String::from_str(env, "LIC000"),
        &BytesN::from_array(env, &[9; 32]),
        admin,
        &BytesN::from_array(env, &[9; 32]),
        &(env.ledger().timestamp() + 86400),
        &BytesN::from_array(env, &[9; 32]),
    );
}

#[test]
fn test_referral_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

    let pr_client = setup_provider_registry(&env, &admin, &contract_id);
    register_provider(&env, &pr_client, &admin, &referring_provider);
    register_provider(&env, &pr_client, &admin, &referred_to);

    let specialty = Symbol::new(&env, "Cardio");
    let reason_hash = BytesN::from_array(&env, &[7; 32]);
    let priority = Symbol::new(&env, "Urgent");
    let clinical_summary_hash = BytesN::from_array(&env, &[1; 32]);
    let mut requested_services = Vec::new(&env);
    requested_services.push_back(String::from_str(&env, "ECG"));

    // 1. Create Referral
    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &specialty,
        &reason_hash,
        &priority,
        &clinical_summary_hash,
        &requested_services,
    );
    assert_eq!(referral_id, 1);

    // 2. Accept Referral
    let estimated_appointment_date = Some(1234567890);
    client.accept_referral(&referral_id, &referred_to, &estimated_appointment_date);

    // 3. Share care summary
    let summary_type = Symbol::new(&env, "LabResults");
    let summary_hash = BytesN::from_array(&env, &[2; 32]);
    client.share_care_summary(&referral_id, &referred_to, &summary_type, &summary_hash);

    // 4. Request care summary
    let mut information_needed = Vec::new();
    information_needed.push_back(String::from_str(&env, "Previous ECGs"));
    client.request_care_summary(&referral_id, &referring_provider, &information_needed);

    // 5. Complete Referral
    let consultation_summary_hash = BytesN::from_array(&env, &[3; 32]);
    let recommendations_hash = BytesN::from_array(&env, &[4; 32]);
    let followup_required = true;
    client.complete_referral(
        &referral_id,
        &referred_to,
        &consultation_summary_hash,
        &recommendations_hash,
        &followup_required,
    );

    // Error case: Try to accept a completed referral (InvalidStatusTransition)
    let res = client.try_accept_referral(&referral_id, &referred_to, &estimated_appointment_date);
    assert!(res.is_err());
}

#[test]
fn test_decline_and_update_status() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

    let pr_client = setup_provider_registry(&env, &admin, &contract_id);
    register_provider(&env, &pr_client, &admin, &referring_provider);
    register_provider(&env, &pr_client, &admin, &referred_to);

    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &BytesN::from_array(&env, &[7; 32]),
        &Symbol::new(&env, "Routine"),
        &BytesN::from_array(&env, &[1; 32]),
        &Vec::new(&env),
    );

    // Decline Referral
    let decline_reason = String::from_str(&env, "Not taking new patients");
    client.decline_referral(&referral_id, &referred_to, &decline_reason, &None);

    // Update Status
    let referral_id2 = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &BytesN::from_array(&env, &[7; 32]),
        &Symbol::new(&env, "Routine"),
        &BytesN::from_array(&env, &[1; 32]),
        &Vec::new(&env),
    );

    // Must accept before it can be scheduled (Pending -> Accepted -> Scheduled)
    client.update_referral_status(
        &referral_id2,
        &referred_to,
        &Symbol::new(&env, "Accepted"),
        &None,
    );
    client.update_referral_status(
        &referral_id2,
        &referred_to,
        &Symbol::new(&env, "Scheduled"),
        &None,
    );
}

#[test]
fn test_auth_failures() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

    let pr_client = setup_provider_registry(&env, &admin, &contract_id);
    register_provider(&env, &pr_client, &admin, &referring_provider);
    register_provider(&env, &pr_client, &admin, &referred_to);

    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &BytesN::from_array(&env, &[7; 32]),
        &Symbol::new(&env, "Routine"),
        &BytesN::from_array(&env, &[1; 32]),
        &Vec::new(&env),
    );

    // Try to accept with wrong provider
    let wrong_provider = Address::generate(&env);
    let res = client.try_accept_referral(&referral_id, &wrong_provider, &None);
    assert!(res.is_err()); // NotAuthorized
}

#[test]
fn test_create_referral_rejects_unregistered_referring_provider() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

    let pr_client = setup_provider_registry(&env, &admin, &contract_id);
    // Only the target provider is registered; the referring provider is not.
    register_provider(&env, &pr_client, &admin, &referred_to);

    let res = client.try_create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &BytesN::from_array(&env, &[7; 32]),
        &Symbol::new(&env, "Routine"),
        &BytesN::from_array(&env, &[1; 32]),
        &Vec::new(&env),
    );
    assert_eq!(res, Err(Ok(Error::ProviderNotRegistered)));
}

#[test]
fn test_initialize_rejects_second_call() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let provider_registry_id = env.register(ProviderRegistry, ());

    client.initialize(&admin, &provider_registry_id);

    // A second initialize must be rejected with AlreadyInitialized.
    let attacker_registry = env.register(ProviderRegistry, ());
    let res = client.try_initialize(&admin, &attacker_registry);
    assert_eq!(res, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_set_provider_registry_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let provider_registry_id = env.register(ProviderRegistry, ());
    client.initialize(&admin, &provider_registry_id);

    let attacker = Address::generate(&env);
    let attacker_registry = env.register(ProviderRegistry, ());
    let res = client.try_set_provider_registry(&attacker, &attacker_registry);
    assert!(res.is_err());
}
