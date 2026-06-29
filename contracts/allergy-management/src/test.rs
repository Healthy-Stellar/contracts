#![cfg(test)]

use soroban_sdk::{symbol_short, testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke}, Address, BytesN, Env, IntoVal, String, Symbol, Vec};
use shared::test_utils::{dummy_hash};

use crate::{
    AllergyManagement, AllergyManagementClient, AllergyStatus, Error, RecordAllergyRequest,
};
use provider_registry::{ProviderRegistry, ProviderRegistryClient};

fn register_provider_in_registry(
    env: &Env,
    client: &ProviderRegistryClient<'_>,
    admin: &Address,
    provider: &Address,
) {
    let issuer = Address::generate(env);
    client.register_provider(
        admin,
        provider,
        &String::from_str(env, "Dr. Smith"),
        &String::from_str(env, "General"),
        &String::from_str(env, "LIC-001"),
        &dummy_hash(env, 1),
        &issuer,
        &dummy_hash(env, 2),
        &u64::MAX,
        &dummy_hash(env, 3),
    );
}

fn create_test_env() -> (
    Env,
    Address,
    Address,
    Address,
    AllergyManagementClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();
    // Set a baseline timestamp so onset_date values like 1000 or 500 are in the past.
    env.ledger().set_timestamp(10_000);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);

    // Set up provider-registry contract
    let provider_registry_id = env.register(ProviderRegistry, ());
    let provider_registry_client = ProviderRegistryClient::new(&env, &provider_registry_id);
    provider_registry_client.initialize(&admin);
    register_provider_in_registry(&env, &provider_registry_client, &admin, &provider);

    // Generate dummy addresses for other registries
    let patient_registry = Address::generate(&env);
    let hospital_registry = Address::generate(&env);
    let insurer_registry = Address::generate(&env);

    let contract_id = env.register(AllergyManagement, ());
    let client = AllergyManagementClient::new(&env, &contract_id);

    let patient_registry = Address::generate(&env);
    let provider_registry = Address::generate(&env);
    let hospital_registry = Address::generate(&env);
    let insurer_registry = Address::generate(&env);
    client.initialize(
        &admin,
        &patient_registry,
        &provider_registry,
        &hospital_registry,
        &insurer_registry,
    );

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
    assert!(!env.auths().is_empty());
}

#[test]
fn test_double_initialize() {
    let (env, admin, _, _, client) = create_test_env();

    let p_reg = Address::generate(&env);
    let prov_reg = Address::generate(&env);
    let hosp_reg = Address::generate(&env);
    let ins_reg = Address::generate(&env);

    // Try to initialize again
    let result = client.try_initialize(&admin, &p_reg, &prov_reg, &hosp_reg, &ins_reg);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
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

    let interactions =
        client.check_drug_allergy_interaction(&patient, &String::from_str(&env, "Penicillin"));

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

    let interactions =
        client.check_drug_allergy_interaction(&patient, &String::from_str(&env, "Aspirin"));

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

// ─── Issue #327 ── Guardian authorization scope ───────────────────────────

#[test]
fn test_guardian_a_cannot_write_allergy_for_patient_b() {
    let (env, _, patient_a, guardian_a, client) = create_test_env();
    let patient_b = Address::generate(&env);
    let guardian_b = Address::generate(&env);

    // Guardian A is authorized only for Patient A.
    client.grant_access(&patient_a, &guardian_a);
    // Guardian B is authorized only for Patient B.
    client.grant_access(&patient_b, &guardian_b);

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

    // Guardian A must not be able to record an allergy for Patient B.
    let result = client.try_record_allergy(&patient_b, &guardian_a, &request);
    assert!(
        result.is_err(),
        "Guardian A must not be able to record allergy for Patient B"
    );
}

#[test]
fn test_guardian_a_cannot_update_severity_for_patient_b_allergy() {
    let (env, _, patient_a, guardian_a, client) = create_test_env();
    let patient_b = Address::generate(&env);
    let guardian_b = Address::generate(&env);

    client.grant_access(&patient_a, &guardian_a);
    client.grant_access(&patient_b, &guardian_b);

    // Guardian B records an allergy for Patient B.
    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "hives"));
    let request_b = create_allergy_request(
        &env,
        "Peanuts",
        symbol_short!("food"),
        reactions,
        symbol_short!("severe"),
        None,
        true,
    );
    let allergy_id = client.record_allergy(&patient_b, &guardian_b, &request_b);

    // Guardian A must not be able to update the severity of Patient B's allergy.
    let result = client.try_update_allergy_severity(
        &allergy_id,
        &guardian_a,
        &symbol_short!("mild"),
        &String::from_str(&env, "unauthorized update attempt"),
    );
    assert!(
        result.is_err(),
        "Guardian A must not be able to update allergy severity for Patient B"
    );
}

#[test]
fn test_guardian_a_cannot_resolve_patient_b_allergy() {
    let (env, _, patient_a, guardian_a, client) = create_test_env();
    let patient_b = Address::generate(&env);
    let guardian_b = Address::generate(&env);

    client.grant_access(&patient_a, &guardian_a);
    client.grant_access(&patient_b, &guardian_b);

    // Guardian B records an allergy for Patient B.
    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "swelling"));
    let request_b = create_allergy_request(
        &env,
        "Aspirin",
        symbol_short!("med"),
        reactions,
        symbol_short!("moderate"),
        None,
        true,
    );
    let allergy_id = client.record_allergy(&patient_b, &guardian_b, &request_b);

    // Guardian A must not be able to resolve Patient B's allergy.
    let result = client.try_resolve_allergy(
        &allergy_id,
        &guardian_a,
        &env.ledger().timestamp(),
        &String::from_str(&env, "unauthorized resolution attempt"),
    );
    assert!(
        result.is_err(),
        "Guardian A must not be able to resolve allergy for Patient B"
    );
}

// ── deregister_patient tests ──────────────────────────────────────────────────

#[test]
fn test_deregister_patient_marks_allergies_deleted() {
    let (env, admin, patient, provider, client) = create_test_env();

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

    // Allergy is Active before deregistration
    client.grant_access(&patient, &provider);
    let allergy = client.get_allergy(&allergy_id, &patient);
    assert_eq!(allergy.status, AllergyStatus::Active);

    client.deregister_patient(&patient);

    // Allergy is now Deleted
    let allergy_after = client.get_allergy(&allergy_id, &patient);
    assert_eq!(allergy_after.status, AllergyStatus::Deleted);
}

#[test]
fn test_deregister_patient_clears_allergy_index() {
    let (env, admin, patient, provider, client) = create_test_env();

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "hives"));

    let request = create_allergy_request(
        &env,
        "Aspirin",
        symbol_short!("med"),
        reactions,
        symbol_short!("moderate"),
        None,
        true,
    );

    client.record_allergy(&patient, &provider, &request);

    // Active allergies exist before deregistration
    client.grant_access(&patient, &provider);
    assert_eq!(client.get_active_allergies(&patient, &patient).len(), 1);

    client.deregister_patient(&patient);

    // Index removed — get_active_allergies returns empty
    assert_eq!(client.get_active_allergies(&patient, &patient).len(), 0);
}

#[test]
fn test_deregister_patient_non_admin_rejected() {
    let (env, _admin, patient, _provider, client) = create_test_env();
    let attacker = Address::generate(&env);

    let result = client
        .mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &attacker,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &client.address,
                fn_name: "deregister_patient",
                args: (&patient,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_deregister_patient(&patient);

    assert!(result.is_err());
}
