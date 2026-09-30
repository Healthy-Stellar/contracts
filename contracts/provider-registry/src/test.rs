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
        .try_get_record(&admin, &admin, &String::from_str(&env, "MISSING"))
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

// ── admin rotation ────────────────────────────────────────────────────────────

#[test]
fn test_admin_rotation_success() {
    let (env, admin, client) = setup();
    let new_admin = Address::generate(&env);

    client.propose_admin_rotation(&admin, &new_admin);
    client.accept_admin_rotation(&new_admin);

    // New admin can perform admin-only actions; old admin cannot.
    let provider = Address::generate(&env);
    let issuer = Address::generate(&env);
    client.register_provider(
        &new_admin,
        &provider,
        &String::from_str(&env, "Dr. New"),
        &String::from_str(&env, "General"),
        &String::from_str(&env, "LIC-NEW"),
        &dummy_hash(&env, 1),
        &issuer,
        &dummy_hash(&env, 2),
        &u64::MAX,
        &dummy_hash(&env, 3),
    );
    assert!(client.is_provider(&provider));

    let err = client
        .try_revoke_provider(&admin, &provider)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn test_propose_rotation_double_propose_returns_error() {
    let (env, admin, client) = setup();
    let first = Address::generate(&env);
    let second = Address::generate(&env);

    client.propose_admin_rotation(&admin, &first);

    let err = client
        .try_propose_admin_rotation(&admin, &second)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::RotationPending);
}

#[test]
fn test_accept_rotation_wrong_address_returns_error() {
    let (env, admin, client) = setup();
    let pending = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);

    let err = client
        .try_accept_admin_rotation(&stranger)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotPendingAdmin);
    // Pending state must still be intact after a failed accept.
    client.accept_admin_rotation(&pending);
}

#[test]
fn test_accept_rotation_without_proposal_returns_error() {
    let (env, _, client) = setup();
    let stranger = Address::generate(&env);

    let err = client
        .try_accept_admin_rotation(&stranger)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NoRotationPending);
}

#[test]
fn test_accept_rotation_after_expiry_returns_error_and_reproposal_succeeds() {
    let (env, admin, client) = setup();
    let pending = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);

    // Advance the ledger timestamp past the 24-hour window.
    env.ledger().with_mut(|l| {
        l.timestamp += ADMIN_ROTATION_WINDOW + 1;
    });

    let err = client
        .try_accept_admin_rotation(&pending)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::RotationExpired);

    // A new proposal must replace the expired pending entry even though the
    // failed accept rolled back its attempted cleanup.
    let replacement = Address::generate(&env);
    client.propose_admin_rotation(&admin, &replacement);
    client.accept_admin_rotation(&replacement);
}

#[test]
fn test_expired_rotation_can_be_replaced_without_accept_attempt() {
    let (env, admin, client) = setup();
    let typo = Address::generate(&env);
    let replacement = Address::generate(&env);

    client.propose_admin_rotation(&admin, &typo);
    env.ledger().with_mut(|l| {
        l.timestamp += ADMIN_ROTATION_WINDOW + 1;
    });

    client.propose_admin_rotation(&admin, &replacement);
    client.accept_admin_rotation(&replacement);
}

#[test]
fn test_admin_can_cancel_rotation() {
    let (env, admin, client) = setup();
    let pending = Address::generate(&env);
    let replacement = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);
    client.cancel_admin_rotation(&admin);
    client.propose_admin_rotation(&admin, &replacement);
    client.accept_admin_rotation(&replacement);
}

#[test]
fn test_non_admin_cannot_cancel_rotation() {
    let (env, admin, client) = setup();
    let pending = Address::generate(&env);
    let impostor = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);
    let err = client
        .try_cancel_admin_rotation(&impostor)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);
    client.accept_admin_rotation(&pending);
}

#[test]
fn test_accept_rotation_at_exact_expiry_boundary_succeeds() {
    let (env, admin, client) = setup();
    let pending = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);

    // Exactly at the expiry timestamp the rotation is still valid
    // (contract uses `> expiry`, so equality is within the window).
    env.ledger().with_mut(|l| {
        l.timestamp += ADMIN_ROTATION_WINDOW;
    });

    client.accept_admin_rotation(&pending);

    // Confirm the new admin is active.
    let provider = Address::generate(&env);
    register_provider_with_anchor(&env, &client, &pending, &provider);
    assert!(client.is_provider(&provider));
}

#[test]
fn test_propose_rotation_non_admin_returns_error() {
    let (env, _, client) = setup();
    let stranger = Address::generate(&env);
    let new_admin = Address::generate(&env);

    let err = client
        .try_propose_admin_rotation(&stranger, &new_admin)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);
}

// ── reactivate_provider ───────────────────────────────────────────────────────

#[test]
fn test_reactivate_provider_restores_active_and_clears_revoked_at() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    register_provider_with_anchor(&env, &client, &admin, &provider);
    client.revoke_provider(&admin, &provider);

    // Confirm provider is inactive and has a revoked_at timestamp.
    let profile = client.get_provider_profile(&provider);
    assert!(!profile.active);
    assert!(profile.credential.revoked_at.is_some());
    assert!(!client.is_provider(&provider));

    client.reactivate_provider(&admin, &provider);

    let profile = client.get_provider_profile(&provider);
    assert!(profile.active);
    assert!(profile.credential.revoked_at.is_none());
    assert!(client.is_provider(&provider));
}

#[test]
fn test_reactivate_provider_non_admin_returns_error() {
    let (env, admin, client) = setup();
    let non_admin = Address::generate(&env);
    let provider = Address::generate(&env);

    register_provider_with_anchor(&env, &client, &admin, &provider);
    client.revoke_provider(&admin, &provider);

    let err = client
        .try_reactivate_provider(&non_admin, &provider)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn test_reactivate_unknown_provider_returns_error() {
    let (env, admin, client) = setup();
    let nobody = Address::generate(&env);

    let err = client
        .try_reactivate_provider(&admin, &nobody)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::RecordNotFound);
}

#[test]
fn test_reactivate_already_active_provider_is_idempotent() {
    // reactivate_provider on an already-active provider should succeed and
    // leave the profile active with no revoked_at (no-op semantics).
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    register_provider_with_anchor(&env, &client, &admin, &provider);
    assert!(client.is_provider(&provider));

    client.reactivate_provider(&admin, &provider);

    let profile = client.get_provider_profile(&provider);
    assert!(profile.active);
    assert!(profile.credential.revoked_at.is_none());
    assert!(client.is_provider(&provider));
}
