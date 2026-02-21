#![cfg(test)]

use crate::contract::{ReferralContract, ReferralContractClient};
use crate::types::Error;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, BytesN, Env, String, Symbol, Vec,
};

#[test]
fn test_referral_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, ReferralContract);
    let client = ReferralContractClient::new(&env, &contract_id);

    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);
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

    let contract_id = env.register_contract(None, ReferralContract);
    let client = ReferralContractClient::new(&env, &contract_id);

    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

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

    let contract_id = env.register_contract(None, ReferralContract);
    let client = ReferralContractClient::new(&env, &contract_id);

    let referring_provider = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let referred_to = Address::generate(&env);

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
