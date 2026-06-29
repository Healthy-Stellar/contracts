#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _, Address, BytesN, Env, String,
};

fn dummy_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn register_provider_with_anchor(
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

fn setup() -> (Env, Address, ProviderRegistryClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(ProviderRegistry, ());
    let client = ProviderRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin);
    (env, admin, client)
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn test_double_initialize_returns_error() {
    let (_, admin, client) = setup();
    let err = client.try_initialize(&admin).unwrap_err().unwrap();
    assert_eq!(err, Error::AlreadyInitialized);
}

#[test]
fn test_mutable_call_before_init_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(ProviderRegistry, ());
    let client = ProviderRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let issuer = Address::generate(&env);
    let err = client
        .try_register_provider(
            &admin,
            &provider,
            &String::from_str(&env, "Dr. Smith"),
            &String::from_str(&env, "General"),
            &String::from_str(&env, "LIC-001"),
            &dummy_hash(&env, 1),
            &issuer,
            &dummy_hash(&env, 2),
            &u64::MAX,
            &dummy_hash(&env, 3),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotInitialized);
}

// ── register / revoke / is_provider ──────────────────────────────────────────

#[test]
fn test_register_and_is_provider() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    assert!(!client.is_provider(&provider));
    register_provider_with_anchor(&env, &client, &admin, &provider);
    assert!(client.is_provider(&provider));
}

#[test]
fn test_register_provider_exposes_profile() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    register_provider_with_anchor(&env, &client, &admin, &provider);

    let profile = client.get_provider_profile(&provider);
    assert_eq!(profile.credential.credential_hash, dummy_hash(&env, 1));
    assert_eq!(profile.credential.attestation_hash, dummy_hash(&env, 2));
    assert_eq!(profile.credential.revocation_reference, dummy_hash(&env, 3));
    assert!(profile.active);
}

#[test]
fn test_revoke_provider_preserves_profile_but_disables_membership() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);
    client.revoke_provider(&admin, &provider);
    assert!(!client.is_provider(&provider));

    let profile = client.get_provider_profile(&provider);
    assert!(!profile.active);
    assert!(profile.credential.revoked_at.is_some());
}

#[test]
fn test_register_provider_non_admin_returns_error() {
    let (env, _, client) = setup();
    let non_admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let issuer = Address::generate(&env);
    let err = client
        .try_register_provider(
            &non_admin,
            &provider,
            &String::from_str(&env, "Dr. Smith"),
            &String::from_str(&env, "General"),
            &String::from_str(&env, "LIC-001"),
            &dummy_hash(&env, 1),
            &issuer,
            &dummy_hash(&env, 2),
            &u64::MAX,
            &dummy_hash(&env, 3),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn test_revoke_provider_non_admin_returns_error() {
    let (env, admin, client) = setup();
    let non_admin = Address::generate(&env);
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);
    let err = client.try_revoke_provider(&non_admin, &provider).unwrap_err().unwrap();
    assert_eq!(err, Error::Unauthorized);
}

// ── add_record ────────────────────────────────────────────────────────────────

#[test]
fn test_add_record_by_whitelisted_provider() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);
    client.add_record(
        &provider,
        &String::from_str(&env, "REC001"),
        &String::from_str(&env, "Patient data"),
    );
    assert_eq!(
        client.get_record(&String::from_str(&env, "REC001")),
        String::from_str(&env, "Patient data")
    );
}

#[test]
fn test_add_record_non_provider_returns_error() {
    let (env, _, client) = setup();
    let stranger = Address::generate(&env);
    let err = client
        .try_add_record(
            &stranger,
            &String::from_str(&env, "REC002"),
            &String::from_str(&env, "bad data"),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotAProvider);
}

#[test]
fn test_add_record_after_revocation_returns_error() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);
    client.revoke_provider(&admin, &provider);
    let err = client
        .try_add_record(
            &provider,
            &String::from_str(&env, "REC003"),
            &String::from_str(&env, "stale"),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotAProvider);
}

// ── get_record ────────────────────────────────────────────────────────────────

#[test]
fn test_get_missing_record_returns_error() {
    let (env, _, client) = setup();
    let err = client
        .try_get_record(&String::from_str(&env, "MISSING"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::RecordNotFound);
}

// ── Batch registration tests (#396) ──────────────────────────────────────────

#[test]
fn test_batch_register_providers_full_success() {
    let (env, admin, client) = setup();

    let mut entries = Vec::new(&env);
    for i in 0..3u8 {
        entries.push_back(BatchProviderEntry {
            provider: Address::generate(&env),
            name: String::from_str(&env, "Dr. Batch"),
            specialty: String::from_str(&env, "General"),
            license_number: String::from_str(&env, "LIC-BATCH"),
            credential_hash: dummy_hash(&env, i + 1),
            issuer: Address::generate(&env),
            attestation_hash: dummy_hash(&env, i + 10),
            expires_at: u64::MAX,
            revocation_reference: dummy_hash(&env, i + 20),
        });
    }

    let results = client.batch_register_providers(&admin, &entries);
    assert_eq!(results.len(), 3);
    for i in 0..3u32 {
        assert!(matches!(results.get(i).unwrap(), BatchEntryStatus::Success));
    }
}

#[test]
fn test_batch_register_providers_idempotent_on_duplicate() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    let entry = BatchProviderEntry {
        provider: provider.clone(),
        name: String::from_str(&env, "Dr. Dup"),
        specialty: String::from_str(&env, "General"),
        license_number: String::from_str(&env, "LIC-DUP"),
        credential_hash: dummy_hash(&env, 1),
        issuer: Address::generate(&env),
        attestation_hash: dummy_hash(&env, 2),
        expires_at: u64::MAX,
        revocation_reference: dummy_hash(&env, 3),
    };

    let mut entries = Vec::new(&env);
    entries.push_back(entry.clone());
    entries.push_back(entry);

    let results = client.batch_register_providers(&admin, &entries);
    assert_eq!(results.len(), 2);
    assert!(matches!(results.get(0).unwrap(), BatchEntryStatus::Success));
    assert!(matches!(results.get(1).unwrap(), BatchEntryStatus::AlreadyExists));
}

#[test]
fn test_batch_register_providers_over_limit_fails() {
    let (env, admin, client) = setup();

    let mut entries = Vec::new(&env);
    for _ in 0..51 {
        entries.push_back(BatchProviderEntry {
            provider: Address::generate(&env),
            name: String::from_str(&env, "Dr. Over"),
            specialty: String::from_str(&env, "General"),
            license_number: String::from_str(&env, "LIC"),
            credential_hash: dummy_hash(&env, 1),
            issuer: Address::generate(&env),
            attestation_hash: dummy_hash(&env, 2),
            expires_at: u64::MAX,
            revocation_reference: dummy_hash(&env, 3),
        });
    }

    let err = client
        .try_batch_register_providers(&admin, &entries)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::BatchTooLarge);
}
