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
    referral_client.initialize(&provider_registry_id);

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
    register_provider(&env, &pr_client, &admin, &referred_to);

    let specialty = Symbol::new(&env, "Cardio");
    let reason = String::from_str(&env, "Heart palpitations");
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
        &reason,
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
    let mut information_needed = Vec::new(&env);
    information_needed.push_back(String::from_str(&env, "Previous ECGs"));
    client.request_care_summary(&referral_id, &referring_provider, &information_needed);

    // 5. Complete Referral
    let consultation_summary_hash = BytesN::from_array(&env, &[3; 32]);
    let recommendations = String::from_str(&env, "Rest and medication");
    let followup_required = true;
    client.complete_referral(
        &referral_id,
        &referred_to,
        &consultation_summary_hash,
        &recommendations,
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
    register_provider(&env, &pr_client, &admin, &referred_to);

    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &String::from_str(&env, "Knee pain"),
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
        &String::from_str(&env, "Knee pain"),
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
    register_provider(&env, &pr_client, &admin, &referred_to);

    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &String::from_str(&env, "Knee pain"),
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
fn test_provider_registration_verification() {
    let env = Env::default();
    env.mock_all_auths();

    // Register ProviderRegistry and initialize it
    let provider_registry_id = env.register_contract(None, ProviderRegistry);
    let pr_client = ProviderRegistryClient::new(&env, &provider_registry_id);
    let admin = Address::generate(&env);
    pr_client.initialize(&admin);

    // Register Referral contract and initialize it with ProviderRegistry
    let referral_id = env.register_contract(None, ReferralContract);
    let client = ReferralContractClient::new(&env, &referral_id);
    client.initialize(&provider_registry_id);

    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let unregistered_provider = Address::generate(&env);
    let registered_provider = Address::generate(&env);

    // Register one provider but not the other
    pr_client.register_provider(
        &admin,
        &registered_provider,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "Cardiology"),
        &String::from_str(&env, "LIC123"),
        &BytesN::from_array(&env, &[1; 32]),
        &admin,
        &BytesN::from_array(&env, &[2; 32]),
        &(env.ledger().timestamp() + 86400),
        &BytesN::from_array(&env, &[3; 32]),
    );

    let specialty = Symbol::new(&env, "Cardio");
    let reason = String::from_str(&env, "Heart palpitations");
    let priority = Symbol::new(&env, "Urgent");
    let clinical_summary_hash = BytesN::from_array(&env, &[1; 32]);
    let requested_services = Vec::new(&env);

    // Try to create referral to unregistered provider
    let res = client.try_create_referral(
        &referring_provider,
        &patient_id,
        &unregistered_provider,
        &specialty,
        &reason,
        &priority,
        &clinical_summary_hash,
        &requested_services,
    );
    assert!(res.is_err()); // ProviderNotRegistered

    // Create referral to registered provider should succeed
    let result = client.try_create_referral(
        &referring_provider,
        &patient_id,
        &registered_provider,
        &specialty,
        &reason,
        &priority,
        &clinical_summary_hash,
        &requested_services,
    );
    assert!(result.is_ok());
    let referral_id_created = result.unwrap().unwrap();
    assert_eq!(referral_id_created, 1);
}

#[test]
fn test_update_referral_status_rejects_pending_to_completed() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

    let pr_client = setup_provider_registry(&env, &admin, &contract_id);
    register_provider(&env, &pr_client, &admin, &referred_to);

    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &String::from_str(&env, "Knee pain"),
        &Symbol::new(&env, "Routine"),
        &BytesN::from_array(&env, &[1; 32]),
        &Vec::new(&env),
    );

    // A freshly created referral is Pending. Jumping straight to Completed
    // must be rejected, since acceptance (and clinical work) is skipped.
    let err = client
        .try_update_referral_status(
            &referral_id,
            &referred_to,
            &Symbol::new(&env, "Completed"),
            &None,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidStatusTransition);
}

#[test]
fn test_update_referral_status_rejects_completed_to_pending() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

    let pr_client = setup_provider_registry(&env, &admin, &contract_id);
    register_provider(&env, &pr_client, &admin, &referred_to);

    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &String::from_str(&env, "Knee pain"),
        &Symbol::new(&env, "Routine"),
        &BytesN::from_array(&env, &[1; 32]),
        &Vec::new(&env),
    );

    // Legally progress the referral all the way to Completed.
    client.update_referral_status(&referral_id, &referred_to, &Symbol::new(&env, "Accepted"), &None);
    client.update_referral_status(&referral_id, &referred_to, &Symbol::new(&env, "Completed"), &None);

    // Reverting a Completed referral back to Pending must be rejected,
    // otherwise the audit trail is corrupted.
    let err = client
        .try_update_referral_status(
            &referral_id,
            &referred_to,
            &Symbol::new(&env, "Pending"),
            &None,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidStatusTransition);
}

#[test]
fn test_update_referral_status_allows_legal_transitions() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

    let pr_client = setup_provider_registry(&env, &admin, &contract_id);
    register_provider(&env, &pr_client, &admin, &referred_to);

    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &referred_to,
        &Symbol::new(&env, "Ortho"),
        &String::from_str(&env, "Knee pain"),
        &Symbol::new(&env, "Routine"),
        &BytesN::from_array(&env, &[1; 32]),
        &Vec::new(&env),
    );

    // Pending -> Accepted is a legal transition and should succeed.
    client.update_referral_status(
        &referral_id,
        &referred_to,
        &Symbol::new(&env, "Accepted"),
        &None,
    );

    // Accepted -> Scheduled is also a legal transition and should succeed.
    client.update_referral_status(
        &referral_id,
        &referred_to,
        &Symbol::new(&env, "Scheduled"),
        &None,
    );
}

#[test]
fn test_referring_provider_cannot_apply_receiving_provider_statuses() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReferralContract, ());
    let client = ReferralContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let receiving_provider = Address::generate(&env);

    let pr_client = setup_provider_registry(&env, &admin, &contract_id);
    register_provider(&env, &pr_client, &admin, &receiving_provider);

    let referral_id = client.create_referral(
        &referring_provider,
        &patient_id,
        &receiving_provider,
        &Symbol::new(&env, "Ortho"),
        &String::from_str(&env, "Knee pain"),
        &Symbol::new(&env, "Routine"),
        &BytesN::from_array(&env, &[1; 32]),
        &Vec::new(&env),
    );

    for status in ["Accepted", "Declined"] {
        let err = client
            .try_update_referral_status(
                &referral_id,
                &referring_provider,
                &Symbol::new(&env, status),
                &None,
            )
            .unwrap_err()
            .unwrap();
        assert_eq!(err, Error::NotAuthorized);
    }

    client.update_referral_status(
        &referral_id,
        &receiving_provider,
        &Symbol::new(&env, "Accepted"),
        &None,
    );

    let err = client
        .try_update_referral_status(
            &referral_id,
            &referring_provider,
            &Symbol::new(&env, "Completed"),
            &None,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotAuthorized);
}
