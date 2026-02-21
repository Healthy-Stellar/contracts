#![cfg(test)]

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String, Symbol, Vec};

use crate::{AllergyManagement, AllergyManagementClient, AllergyStatus, RecordAllergyRequest};

fn create_test_env() -> (Env, Address, Address, Address, AllergyManagementClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    let contract_id = env.register(AllergyManagement, ());
    let client = AllergyManagementClient::new(&env, &contract_id);

    client.initialize(&admin);

    (env, admin, patient, provider, client)
}

fn create_allergy_request(
    env: &Env,
    allergen: &str,
    allergen_type: Symbol,
    reactions: Vec<String>,
    severity: Symbol,
    onset_date: Option<u64>,
    verified: bool,
) -> RecordAllergyRequest {
    RecordAllergyRequest {
        allergen: String::from_str(env, allergen),
        allergen_type,
        reaction_type: reactions,
        severity,
        onset_date,
        verified,
    }
}

#[test]
fn test_initialize() {
    let (env, _admin, _, _, _client) = create_test_env();
    
    // Verify initialization succeeded (no panic)
    assert!(env.auths().len() > 0);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_double_initialize() {
    let (_, admin, _, _, client) = create_test_env();
    
    // Try to initialize again
    client.initialize(&admin);
}

#[test]
fn test_record_allergy() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));
    reactions.push_back(String::from_str(&env, "hives"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("moderate"),
        Some(1000u64),
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    assert_eq!(allergy_id, 0);

    let allergy = client.get_allergy(&allergy_id, &provider);
    assert_eq!(allergy.patient_id, patient);
    assert_eq!(allergy.allergen, String::from_str(&env, "Penicillin"));
    assert_eq!(allergy.severity, symbol_short!("moderate"));
    assert_eq!(allergy.status, AllergyStatus::Active);
}

#[test]
fn test_record_multiple_allergies() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "anaphylaxis"));

    let request1 = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions.clone(),
        symbol_short!("severe"),
        None,
        true,
    );

    let request2 = create_allergy_request(
        &env,
        "Peanuts",
        symbol_short!("food"),
        reactions,
        symbol_short!("critical"),
        None,
        true,
    );

    let id1 = client.record_allergy(&patient, &provider, &request1);
    let id2 = client.record_allergy(&patient, &provider, &request2);

    assert_eq!(id1, 0);
    assert_eq!(id2, 1);

    let active = client.get_active_allergies(&patient, &provider);
    assert_eq!(active.len(), 2);
}

#[test]
#[should_panic]
fn test_duplicate_allergy_prevention() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    client.record_allergy(&patient, &provider, &request);
    // Try to record duplicate - should panic
    client.record_allergy(&patient, &provider, &request);
}

#[test]
fn test_update_allergy_severity() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    let reason = String::from_str(&env, "Patient experienced more severe reaction");
    client.update_allergy_severity(&allergy_id, &provider, &symbol_short!("severe"), &reason);

    let allergy = client.get_allergy(&allergy_id, &provider);
    assert_eq!(allergy.severity, symbol_short!("severe"));
    assert_eq!(allergy.severity_history.len(), 1);
}

#[test]
#[should_panic]
fn test_update_severity_invalid() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    // Try invalid severity - should panic
    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &symbol_short!("invalid"),
        &String::from_str(&env, "test"),
    );
}

#[test]
fn test_resolve_allergy() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    let resolution_date = env.ledger().timestamp();
    let reason = String::from_str(&env, "Tolerance developed after desensitization");
    client.resolve_allergy(&allergy_id, &provider, &resolution_date, &reason);

    let allergy = client.get_allergy(&allergy_id, &provider);
    assert_eq!(allergy.status, AllergyStatus::Resolved);
    assert_eq!(allergy.resolution_date, Some(resolution_date));

    let active = client.get_active_allergies(&patient, &provider);
    assert_eq!(active.len(), 0);
}

#[test]
#[should_panic]
fn test_resolve_already_resolved() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    client.resolve_allergy(
        &allergy_id,
        &provider,
        &env.ledger().timestamp(),
        &String::from_str(&env, "Test"),
    );

    // Try to resolve again - should panic
    client.resolve_allergy(
        &allergy_id,
        &provider,
        &env.ledger().timestamp(),
        &String::from_str(&env, "Test again"),
    );
}

#[test]
fn test_check_drug_allergy_interaction() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "anaphylaxis"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("severe"),
        None,
        true,
    );

    client.record_allergy(&patient, &provider, &request);

    let interactions = client
        .check_drug_allergy_interaction(&patient, &String::from_str(&env, "Penicillin"));

    assert_eq!(interactions.len(), 1);
    let interaction = interactions.get(0).unwrap();
    assert_eq!(interaction.allergen, String::from_str(&env, "Penicillin"));
    assert_eq!(interaction.severity, symbol_short!("severe"));
}

#[test]
fn test_check_drug_no_interaction() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    client.record_allergy(&patient, &provider, &request);

    let interactions = client
        .check_drug_allergy_interaction(&patient, &String::from_str(&env, "Aspirin"));

    assert_eq!(interactions.len(), 0);
}

#[test]
fn test_get_active_allergies() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request1 = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions.clone(),
        symbol_short!("mild"),
        None,
        true,
    );

    let request2 = create_allergy_request(
        &env,
        "Peanuts",
        symbol_short!("food"),
        reactions,
        symbol_short!("severe"),
        None,
        true,
    );

    client.record_allergy(&patient, &provider, &request1);
    let id2 = client.record_allergy(&patient, &provider, &request2);

    client.resolve_allergy(
        &id2,
        &provider,
        &env.ledger().timestamp(),
        &String::from_str(&env, "Test"),
    );

    let active = client.get_active_allergies(&patient, &provider);
    assert_eq!(active.len(), 1);
    let allergy = active.get(0).unwrap();
    assert_eq!(allergy.allergen, String::from_str(&env, "Penicillin"));
}

#[test]
fn test_access_control() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    // Authorized access should work
    let _allergy = client.get_allergy(&allergy_id, &provider);
}

#[test]
#[should_panic]
fn test_unauthorized_access() {
    let (env, _, patient, provider, client) = create_test_env();
    let unauthorized = Address::generate(&env);

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    // Unauthorized access should panic
    client.get_allergy(&allergy_id, &unauthorized);
}

#[test]
fn test_patient_self_access() {
    let (env, _, patient, provider, client) = create_test_env();

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    client.grant_access(&patient, &provider);
    let allergy_id = client.record_allergy(&patient, &provider, &request);

    // Patient can access their own data
    let _allergy = client.get_allergy(&allergy_id, &patient);
}

#[test]
fn test_revoke_access() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    // Access should work
    let _allergy = client.get_allergy(&allergy_id, &provider);

    // Revoke access
    client.revoke_access(&patient, &provider);
}

#[test]
#[should_panic]
fn test_revoked_access_fails() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    // Revoke access
    client.revoke_access(&patient, &provider);

    // This should panic
    client.get_allergy(&allergy_id, &provider);
}

#[test]
#[should_panic]
fn test_invalid_allergen_type() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("invalid"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    // Should panic with invalid allergen type
    client.record_allergy(&patient, &provider, &request);
}

#[test]
fn test_get_all_allergies() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request1 = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions.clone(),
        symbol_short!("mild"),
        None,
        true,
    );

    let request2 = create_allergy_request(
        &env,
        "Peanuts",
        symbol_short!("food"),
        reactions,
        symbol_short!("severe"),
        None,
        true,
    );

    client.record_allergy(&patient, &provider, &request1);
    let id2 = client.record_allergy(&patient, &provider, &request2);

    client.resolve_allergy(
        &id2,
        &provider,
        &env.ledger().timestamp(),
        &String::from_str(&env, "Test"),
    );

    let all = client.get_all_allergies(&patient, &provider);
    assert_eq!(all.len(), 2);

    let active = client.get_active_allergies(&patient, &provider);
    assert_eq!(active.len(), 1);
}

#[test]
fn test_multiple_severity_updates() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let request = create_allergy_request(
        &env,
        "Penicillin",
        symbol_short!("med"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &symbol_short!("moderate"),
        &String::from_str(&env, "Worsening symptoms"),
    );

    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &symbol_short!("severe"),
        &String::from_str(&env, "Anaphylactic reaction"),
    );

    let allergy = client.get_allergy(&allergy_id, &provider);
    assert_eq!(allergy.severity, symbol_short!("severe"));
    assert_eq!(allergy.severity_history.len(), 2);
}

#[test]
fn test_environmental_allergy() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "sneezing"));
    reactions.push_back(String::from_str(&env, "watery eyes"));

    let request = create_allergy_request(
        &env,
        "Pollen",
        symbol_short!("env"),
        reactions,
        symbol_short!("mild"),
        None,
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    let allergy = client.get_allergy(&allergy_id, &provider);
    assert_eq!(allergy.allergen_type, symbol_short!("env"));
    assert_eq!(allergy.reaction_type.len(), 2);
}

#[test]
fn test_food_allergy() {
    let (env, _, patient, provider, client) = create_test_env();

    client.grant_access(&patient, &provider);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "anaphylaxis"));

    let request = create_allergy_request(
        &env,
        "Shellfish",
        symbol_short!("food"),
        reactions,
        symbol_short!("critical"),
        Some(500u64),
        true,
    );

    let allergy_id = client.record_allergy(&patient, &provider, &request);

    let allergy = client.get_allergy(&allergy_id, &provider);
    assert_eq!(allergy.allergen_type, symbol_short!("food"));
    assert_eq!(allergy.severity, symbol_short!("critical"));
    assert_eq!(allergy.onset_date, Some(500u64));
}
