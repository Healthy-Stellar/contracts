#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _, testutils::Ledger as _, Address, BytesN, Env, String,
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
fn test_provider_hospital_affiliation_requires_active_provider_and_matches_hospital() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    let hospital = Address::generate(&env);
    let other_hospital = Address::generate(&env);

    register_provider_with_anchor(&env, &client, &admin, &provider);
    client.set_provider_hospital(&admin, &provider, &hospital, &true);

    assert!(client.is_provider_affiliated(&provider, &hospital));
    assert!(!client.is_provider_affiliated(&provider, &other_hospital));

    client.revoke_provider(&admin, &provider);
    assert!(!client.is_provider_affiliated(&provider, &hospital));
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

// ── renew_credential ─────────────────────────────────────────────────────────

#[test]
fn test_expired_credential_locks_out_then_renew_restores() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    let issuer = Address::generate(&env);

    // Register with a credential that expires at ledger timestamp 1000.
    client.register_provider(
        &admin,
        &provider,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "General"),
        &String::from_str(&env, "LIC-001"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &1000u64,
        &dummy_hash(&env, 3),
    );
    assert!(client.is_provider(&provider));

    // Advance past expiry: provider is locked out.
    env.ledger().set_timestamp(2000);
    assert!(!client.is_provider(&provider));

    // Renew the credential with a future expiry.
    client.renew_credential(
        &admin,
        &provider,
        &dummy_hash(&env, 10),
        &dummy_hash(&env, 11),
        &5000u64,
    );
    assert!(client.is_provider(&provider));

    let profile = client.get_provider_profile(&provider);
    assert_eq!(profile.credential.credential_hash, dummy_hash(&env, 10));
    assert_eq!(profile.credential.attestation_hash, dummy_hash(&env, 11));
    assert_eq!(profile.credential.expires_at, 5000u64);
}

#[test]
fn test_renew_credential_rejects_past_expiry() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);

    env.ledger().set_timestamp(2000);
    let err = client
        .try_renew_credential(
            &admin,
            &provider,
            &dummy_hash(&env, 10),
            &dummy_hash(&env, 11),
            &2000u64,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidExpiry);
}

#[test]
fn test_renew_credential_non_admin_returns_error() {
    let (env, admin, client) = setup();
    let non_admin = Address::generate(&env);
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);

    let err = client
        .try_renew_credential(
            &non_admin,
            &provider,
            &dummy_hash(&env, 10),
            &dummy_hash(&env, 11),
            &u64::MAX,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn test_register_provider_rejects_expired_credential() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    let issuer = Address::generate(&env);

    env.ledger().set_timestamp(2000);
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
            &1000u64,
            &dummy_hash(&env, 3),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidExpiry);
}

// ── add_record ────────────────────────────────────────────────────────────────

#[test]
fn test_add_record_by_whitelisted_provider() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);
    let nonce = client.get_caller_nonce(&provider);
    client.add_record(
        &provider,
        &String::from_str(&env, "REC001"),
        &String::from_str(&env, "Patient data"),
        &(nonce + 1),
    );
    assert_eq!(
        client.get_record(&provider, &provider, &String::from_str(&env, "REC001")),
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
            &1u64,
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
            &1u64,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotAProvider);
}

#[test]
fn test_add_record_stale_nonce_rejected() {
    let (env, admin, client) = setup();
    env.mock_all_auths();
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);

    let nonce = client.get_caller_nonce(&provider);
    // First call with valid nonce succeeds
    client.add_record(
        &provider,
        &String::from_str(&env, "REC001"),
        &String::from_str(&env, "data"),
        &(nonce + 1),
    );
    // Second call with the same nonce must be rejected (stale)
    let err = client
        .try_add_record(
            &provider,
            &String::from_str(&env, "REC002"),
            &String::from_str(&env, "replay"),
            &(nonce + 1),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::StaleNonce);
}

#[test]
fn test_add_record_zero_nonce_accepted_as_initial() {
    let (env, admin, client) = setup();
    env.mock_all_auths();
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &admin, &provider);

    // Initial nonce is 0; passing 0 should fail (must be > 0), so pass 1
    let nonce = client.get_caller_nonce(&provider);
    assert_eq!(nonce, 0);
    client.add_record(
        &provider,
        &String::from_str(&env, "REC001"),
        &String::from_str(&env, "initial data"),
        &(nonce + 1),
    );
    // Verify the nonce was incremented
    assert_eq!(client.get_caller_nonce(&provider), 1);
}

// ── get_record ────────────────────────────────────────────────────────────────

#[test]
fn test_get_missing_record_returns_error() {
    let (env, admin, client) = setup();
    let err = client
        .try_get_record(
            &admin,
            &admin,
            &String::from_str(&env, "MISSING"),
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::RecordNotFound);
}
