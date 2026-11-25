#![cfg(test)]
use super::*;
use provider_registry::{ProviderRegistry, ProviderRegistryClient};
use soroban_sdk::{
    Address, BytesN, Env, String, Symbol,
    testutils::{Address as _, Events as _, Ledger},
};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Deploy a provider registry and the clinical-guideline contract, wire them
/// together and register one provider.
///
/// Returns `(env, client, admin, registered_provider)`.
fn setup() -> (
    Env,
    ClinicalGuidelineContractClient<'static>,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let registry_id = env.register(ProviderRegistry, ());
    let registry = ProviderRegistryClient::new(&env, &registry_id);
    registry.initialize(&admin);

    let contract_id = env.register(ClinicalGuidelineContract, ());
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    client.set_provider_registry(&registry_id);

    let provider = Address::generate(&env);
    registry.register_provider(
        &admin,
        &provider,
        &String::from_str(&env, "Dr. Guideline"),
        &String::from_str(&env, "Internal Medicine"),
        &String::from_str(&env, "LIC-001"),
        &BytesN::from_array(&env, &[1u8; 32]),
        &admin,
        &BytesN::from_array(&env, &[2u8; 32]),
        &(env.ledger().timestamp() + 86_400),
        &BytesN::from_array(&env, &[3u8; 32]),
    );

    (env, client, admin, provider)
}

fn register_guideline(
    env: &Env,
    client: &ClinicalGuidelineContractClient<'static>,
    admin: &Address,
) -> (String, BytesN<32>, Symbol) {
    let guideline_id = String::from_str(env, "G123");
    let criteria_hash = BytesN::from_array(env, &[0u8; 32]);
    let evidence_level = Symbol::new(env, "Level_A");
    client.register_clinical_guideline(
        admin,
        &guideline_id,
        &String::from_str(env, "Flu"),
        &criteria_hash,
        &BytesN::from_array(env, &[1u8; 32]),
        &evidence_level,
    );
    (guideline_id, criteria_hash, evidence_level)
}

fn create_reminder(
    env: &Env,
    client: &ClinicalGuidelineContractClient<'static>,
    patient: &Address,
    provider: &Address,
) -> u64 {
    client.create_reminder(
        patient,
        provider,
        &Symbol::new(env, "MEDS"),
        &1_000_000,
        &Symbol::new(env, "HIGH"),
    )
}

// ── guidelines ───────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_unauthorized_registration() {
    let env = Env::default();
    let contract_id = env.register(ClinicalGuidelineContract, ());
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.register_clinical_guideline(
        &admin,
        &String::from_str(&env, "FAIL"),
        &String::from_str(&env, "NA"),
        &BytesN::from_array(&env, &[0u8; 32]),
        &BytesN::from_array(&env, &[0u8; 32]),
        &Symbol::new(&env, "B"),
    );
}

// ── reminders ────────────────────────────────────────────────────────────────

#[test]
fn test_reminders() {
    let (env, client, _admin, provider) = setup();
    let patient = Address::generate(&env);

    env.ledger().with_mut(|li| {
        li.timestamp = 12345;
    });

    let first = create_reminder(&env, &client, &patient, &provider);
    let second = create_reminder(&env, &client, &patient, &provider);
    assert_eq!(first, 1);
    assert_eq!(second, 2);

    let reminder = client.get_reminder(&patient, &patient, &first);
    assert_eq!(reminder.reminder_id, first);
    assert_eq!(reminder.patient_id, patient);
    assert_eq!(reminder.due_date, 1_000_000);
    assert_eq!(reminder.created_at, 12345);
}

// ── #851: create_reminder must enforce the provider-registry check ───────────

#[test]
fn test_create_reminder_rejects_unregistered_provider() {
    let (env, client, _admin, _provider) = setup();

    let res = client.try_create_reminder(
        &Address::generate(&env),
        &Address::generate(&env),
        &Symbol::new(&env, "MEDS"),
        &1_000_000,
        &Symbol::new(&env, "HIGH"),
    );
    assert_eq!(
        res.err().expect("expected NotAuthorized"),
        Ok(Error::NotAuthorized)
    );
}

#[test]
fn test_create_reminder_rejects_when_registry_not_configured() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(ClinicalGuidelineContract, (&admin,));
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

    client.register_clinical_guideline(
        &admin,
        &String::from_str(&env, "FAIL"),
        &String::from_str(&env, "NA"),
        &BytesN::from_array(&env, &[0u8; 32]),
        &BytesN::from_array(&env, &[0u8; 32]),
        &Symbol::new(&env, "B"),
    );
}

// ── #852: get_reminder requires authentication and access control ────────────

#[test]
fn test_register_guideline_emits_event() {
    let env = Env::default();
    let admin = Address::generate(&env);
    env.mock_all_auths();
    let contract_id = env.register(ClinicalGuidelineContract, (&admin,));
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

#[test]
fn test_get_reminder_rejects_unrelated_caller() {
    let (env, client, _admin, provider) = setup();
    let patient = Address::generate(&env);
    let id = create_reminder(&env, &client, &patient, &provider);

    let stranger = Address::generate(&env);
    let res = client.try_get_reminder(&stranger, &patient, &id);
    assert_eq!(
        res.err().expect("expected NotAuthorized"),
        Ok(Error::NotAuthorized)
    );
}

#[test]
fn test_update_guideline_emits_event() {
    let env = Env::default();
    let admin = Address::generate(&env);
    env.mock_all_auths();
    let contract_id = env.register(ClinicalGuidelineContract, (&admin,));
    let client = ClinicalGuidelineContractClient::new(&env, &contract_id);

#[test]
fn test_register_guideline_emits_event() {
    let (env, client, admin, _provider) = setup();
    register_guideline(&env, &client, &admin);

    let events = env.events().all();
    // Should have at least one event from register_clinical_guideline
    assert!(!events.is_empty());
}

#[test]
fn test_update_guideline_emits_event() {
    let (env, client, admin, _provider) = setup();
    let (guideline_id, criteria_hash, evidence_level) = register_guideline(&env, &client, &admin);

    client.update_clinical_guideline(
        &admin,
        &guideline_id,
        &String::from_str(&env, "Influenza"),
        &criteria_hash,
        &BytesN::from_array(&env, &[1u8; 32]),
        &evidence_level,
    );

    let events = env.events().all();
    // Should have at least one event from update_clinical_guideline
    assert!(!events.is_empty());
}

#[test]
fn test_create_reminder_emits_event() {
    let (env, client, _admin, provider) = setup();
    create_reminder(&env, &client, &Address::generate(&env), &provider);

    let events = env.events().all();
    // Should have at least one event from create_reminder
    assert!(!events.is_empty());
}
