#![cfg(test)]
#![allow(deprecated)]

use crate::contract::{
    ProviderRegistryClient, TelemedicineContract,
    TelemedicineContractClient,
};
use crate::types::PrescriptionRequest;
use soroban_sdk::{
    contract, contractimpl, testutils::{Address as _, Ledger as _},
    Address, BytesN, Env, String, Symbol, Vec,
};

// Mock provider-registry for cross-contract testing
#[contract]
pub struct MockProviderRegistry;

#[contractimpl]
impl MockProviderRegistry {
    pub fn is_provider(env: Env, provider: Address) -> bool {
        let key = (Symbol::new(&env, "Revoked"), provider.clone());
        !env.storage().persistent().has(&key)
    }

    pub fn set_revoked(env: Env, provider: Address) {
        let key = (Symbol::new(&env, "Revoked"), provider.clone());
        env.storage().persistent().set(&key, &true);
    }
}

#[test]
fn test_telemedicine_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TelemedicineContract, ());
    let client = TelemedicineContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let visit_time = 1700000000;
    let visit_type = Symbol::new(&env, "Consult");
    let platform = Symbol::new(&env, "ZoomHD");

    // 1. Schedule Visit
    let visit_id = client.schedule_virtual_visit(
        &patient_id,
        &provider_id,
        &visit_time,
        &visit_type,
        &30,
        &platform,
        &true,
        &true,
    );
    assert_eq!(visit_id, 1);

    // Register provider license in NY so eligibility passes.
    client.register_provider_license(
        &provider_id,
        &String::from_str(&env, "NY"),
        &String::from_str(&env, "LIC-NY-001"),
        &0_u64,
    );

    // 2. Verify Eligibility
    let eligibility = client.verify_telemedicine_eligibility(
        &patient_id,
        &provider_id,
        &String::from_str(&env, "NY"),
        &String::from_str(&env, "NY"),
    );
    assert!(eligibility.is_eligible);

    // 3. Start Session
    let session_start_time = 1700000010;
    let token = client.start_virtual_session(
        &visit_id,
        &provider_id,
        &session_start_time,
        &String::from_str(&env, "NY"),
        &String::from_str(&env, "NY"),
    );
    assert_ne!(token, BytesN::from_array(&env, &[0; 32]));
    client.validate_session_token(&visit_id, &provider_id, &token);

    let replay = client.try_validate_session_token(&visit_id, &provider_id, &token);
    assert_eq!(replay, Err(Ok(crate::types::Error::SessionAlreadyUsed)));

    // 4. Record technical issue
    client.record_technical_issue(
        &visit_id,
        &patient_id,
        &Symbol::new(&env, "Audio"),
        &String::from_str(&env, "Could not hear provider"),
        &Some(String::from_str(&env, "Reconnected")),
    );

    // 5. Prescribe during visit
    let rx_request = PrescriptionRequest {
        medication_name: String::from_str(&env, "Amoxicillin"),
        dosage: String::from_str(&env, "500mg"),
        frequency: String::from_str(&env, "BID"),
        duration_days: 10,
    };
    let rx_id = client.prescribe_during_visit(&visit_id, &provider_id, &patient_id, &rx_request);
    assert_eq!(rx_id, 0);

    // 6. Record documentation
    let note_hash = BytesN::from_array(&env, &[1; 32]);
    let mut diagnosis_codes = Vec::new(&env);
    diagnosis_codes.push_back(String::from_str(&env, "J01.90"));

    client.record_visit_documentation(
        &visit_id,
        &provider_id,
        &note_hash,
        &diagnosis_codes,
        &String::from_str(&env, "Acute sinusitis"),
        &String::from_str(&env, "Prescribed antibiotics"),
    );

    // 7. End session
    client.end_virtual_session(&visit_id, &provider_id, &(session_start_time + 1200), &20);

    // Error case: End already completed session
    let res =
        client.try_end_virtual_session(&visit_id, &provider_id, &(session_start_time + 1200), &20);
    assert!(res.is_err());
}

#[test]
fn test_auth_and_eligibility_failures() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TelemedicineContract, ());
    let client = TelemedicineContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    // Test ineligible state
    let eligibility = client.verify_telemedicine_eligibility(
        &patient_id,
        &provider_id,
        &String::from_str(&env, "NY"),
        &String::from_str(&env, "CA"),
    );
    assert!(!eligibility.is_eligible);

    // Schedule visit
    let visit_id = client.schedule_virtual_visit(
        &patient_id,
        &provider_id,
        &1700000000,
        &Symbol::new(&env, "Consult"),
        &30,
        &Symbol::new(&env, "ZoomHD"),
        &true,
        &false,
    );

    // Try starting session with wrong provider
    let wrong_provider = Address::generate(&env);
    let res = client.try_start_virtual_session(
        &visit_id,
        &wrong_provider,
        &1700000010,
        &String::from_str(&env, "NY"),
        &String::from_str(&env, "NY"),
    );
    assert!(res.is_err());

    // Try prescribing to wrong patient
    let wrong_patient = Address::generate(&env);
    let rx_request = PrescriptionRequest {
        medication_name: String::from_str(&env, "Amoxicillin"),
        dosage: String::from_str(&env, "500mg"),
        frequency: String::from_str(&env, "BID"),
        duration_days: 10,
    };
    let rx_res =
        client.try_prescribe_during_visit(&visit_id, &provider_id, &wrong_patient, &rx_request);
    assert!(rx_res.is_err());
}

#[test]
fn test_session_tokens_are_unique_bound_expiring_and_non_replayable() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TelemedicineContract, ());
    let client = TelemedicineContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let other_provider = Address::generate(&env);

    env.ledger().with_mut(|li| {
        li.timestamp = 1_700_000_000;
    });

    // Register provider license in NY so eligibility passes.
    client.register_provider_license(
        &provider_id,
        &String::from_str(&env, "NY"),
        &String::from_str(&env, "LIC-NY-001"),
        &0_u64,
    );

    let visit_one = client.schedule_virtual_visit(
        &patient_id,
        &provider_id,
        &1_700_000_100,
        &Symbol::new(&env, "Consult"),
        &30,
        &Symbol::new(&env, "ZoomHD"),
        &true,
        &true,
    );
    let visit_two = client.schedule_virtual_visit(
        &patient_id,
        &provider_id,
        &1_700_000_200,
        &Symbol::new(&env, "Follow"),
        &30,
        &Symbol::new(&env, "ZoomHD"),
        &true,
        &false,
    );

    let token_one = client.start_virtual_session(
        &visit_one,
        &provider_id,
        &1_700_000_100,
        &String::from_str(&env, "NY"),
        &String::from_str(&env, "NY"),
    );
    let token_two = client.start_virtual_session(
        &visit_two,
        &provider_id,
        &1_700_000_200,
        &String::from_str(&env, "NY"),
        &String::from_str(&env, "NY"),
    );
    assert_ne!(token_one, token_two);

    let wrong_caller = client.try_validate_session_token(&visit_one, &other_provider, &token_one);
    assert_eq!(
        wrong_caller,
        Err(Ok(crate::types::Error::InvalidSessionToken))
    );

    client.validate_session_token(&visit_one, &provider_id, &token_one);
    let replay = client.try_validate_session_token(&visit_one, &provider_id, &token_one);
    assert_eq!(replay, Err(Ok(crate::types::Error::SessionAlreadyUsed)));

    env.ledger().with_mut(|li| {
        li.timestamp = 1_700_004_000;
    });
    let expired = client.try_validate_session_token(&visit_two, &provider_id, &token_two);
    assert_eq!(expired, Err(Ok(crate::types::Error::SessionExpired)));
}

#[test]
fn test_schedule_visit_with_active_provider_in_registry() {
    let env = Env::default();
    env.mock_all_auths();

    let telemedicine_id = env.register(TelemedicineContract, ());
    let registry_id = env.register(MockProviderRegistry, ());
    let telemedicine = TelemedicineContractClient::new(&env, &telemedicine_id);
    let _registry = MockProviderRegistryClient::new(&env, &registry_id);

    telemedicine.initialize(&registry_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    // Provider is active by default (not revoked)
    let visit_id = telemedicine.schedule_virtual_visit(
        &patient_id,
        &provider_id,
        &1700000000,
        &Symbol::new(&env, "Consult"),
        &30,
        &Symbol::new(&env, "ZoomHD"),
        &true,
        &true,
    );
    assert_eq!(visit_id, 1);
}

#[test]
fn test_schedule_visit_rejected_when_provider_not_in_registry() {
    let env = Env::default();
    env.mock_all_auths();

    let telemedicine_id = env.register(TelemedicineContract, ());
    let registry_id = env.register(MockProviderRegistry, ());
    let telemedicine = TelemedicineContractClient::new(&env, &telemedicine_id);
    let registry = MockProviderRegistryClient::new(&env, &registry_id);

    telemedicine.initialize(&registry_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    // Revoke the provider
    registry.set_revoked(&provider_id);

    let result = telemedicine.try_schedule_virtual_visit(
        &patient_id,
        &provider_id,
        &1700000000,
        &Symbol::new(&env, "Consult"),
        &30,
        &Symbol::new(&env, "ZoomHD"),
        &true,
        &true,
    );
    assert_eq!(result, Err(Ok(crate::types::Error::ProviderNotRegistered)));
}

#[test]
fn test_schedule_visit_without_registry_configured() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TelemedicineContract, ());
    let client = TelemedicineContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    // Without registry configured, scheduling should still work
    let visit_id = client.schedule_virtual_visit(
        &patient_id,
        &provider_id,
        &1700000000,
        &Symbol::new(&env, "Consult"),
        &30,
        &Symbol::new(&env, "ZoomHD"),
        &true,
        &true,
    );
    assert_eq!(visit_id, 1);
}
