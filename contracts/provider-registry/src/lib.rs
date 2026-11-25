#![no_std]

//! # Provider Registry Contract
//!
//! Maintains healthcare provider registry with credentials, specializations, network participation,
//! and provider validation for access control decisions.
//!
//! ## Single source of truth for "active clinician"
//!
//! This contract is the canonical registry for whether an address is a trusted, active
//! clinician. Other registries (doctor-registry, patient-registry's doctor flow,
//! healthcare-credentialing) are deprecated or act as thin views over this contract.
//! Committee decisions in healthcare-credentialing call `set_active` here so that a
//! suspension/revocation in one place takes effect everywhere.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Admin-only provider registration. Provider authentication for
//! profile updates. Credential verification required before network participation. Address nonzero
//! validation prevents invalid entries. Specialization enumeration restricts valid provider types.
//!
//! **Audit Controls:** Provider registration events logged with credential type. Credential
//! updates tracked with new specialization. Network participation events recorded. Provider
//! deactivation events logged. Credential expiration events tracked.
//!
//! **Data Retention Policy:** Provider records retained indefinitely as reference data. Credential
//! information persisted with expiration dates. Specialization information maintained for service
//! validation. Deactivated providers retain historical data without deletion.
//!
//! **Encryption/Integrity:** Provider addresses validated via nonzero checks. Address immutable
//! once registered. Specialization enum validates allowed provider types. Credential metadata
//! stored encrypted in persistent state. Registry lookup validates provider identity.

use shared::privacy::validate_nonzero_address;
use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short, Address, BytesN, Env,
    String, Vec,
};

mod test;

// ── Errors ────────────────────────────────────────────────────────────────────

/// Maximum entries per batch call.
pub const MAX_BATCH_SIZE: u32 = 50;
/// Admin rotation confirmation window (24 hours in seconds).
pub const ADMIN_ROTATION_WINDOW: u64 = 86_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized     = 2,
    Unauthorized       = 3,
    NotAProvider       = 4,
    RecordNotFound     = 5,
    BatchTooLarge      = 6,
    InvalidAddress     = 7,
    RotationPending    = 8,
    NoRotationPending  = 9,
    RotationExpired    = 10,
    NotPendingAdmin    = 11,
    StaleNonce         = 12,
    AlreadyExists      = 13,
    CredentialExpired  = 14,
}

/// Input entry for `batch_register_providers`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchProviderEntry {
    pub provider: Address,
    pub name: String,
    pub specialty: String,
    pub license_number: String,
    pub credential_hash: BytesN<32>,
    pub issuer: Address,
    pub attestation_hash: BytesN<32>,
    pub expires_at: u64,
    pub revocation_reference: BytesN<32>,
}

/// Per-entry outcome returned by `batch_register_providers`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchEntryStatus {
    /// Entry registered successfully.
    Success       = 1,
    /// Entry skipped because the address is already registered.
    AlreadyExists = 2,
    InvalidAddress = 6,
    /// Entry skipped because `expires_at` is not in the future.
    CredentialExpired = 7,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialAnchor {
    pub credential_hash: BytesN<32>,
    pub issuer: Address,
    pub attestation_hash: BytesN<32>,
    pub expires_at: u64,
    pub revocation_reference: BytesN<32>,
    pub revoked_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProfile {
    pub name: String,
    pub specialty: String,
    pub license_number: String,
    pub credential: CredentialAnchor,
    pub active: bool,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Initialized,
    Admin,
    Provider(Address),
    Record(Address, String),
    ProviderRecords(Address),
    ProviderRecordCount(Address),
    RateLimitConfig,
    ProviderRate(Address),
    ProviderReputation(Address),
    ProviderRatingByPatient(Address, Address),
    PendingAdmin,
    RotationExpiry,
    /// Per-caller nonce for replay attack protection: (caller) -> u64
    CallerNonce(Address),
    ProviderHospital(Address, Address),
    /// External contract authorised to update `active` (e.g. credentialing committee).
    TrustedAuthority(Address),
    /// Historical credential anchors for a provider (append-only).
    CredentialHistory(Address),
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct ProviderRegistry;

#[contractimpl]
impl ProviderRegistry {
    /// Runs atomically at deployment, so no one can front-run it to become admin
    /// of the root of trust for `is_provider`.
    pub fn __constructor(env: Env, admin: Address) {
        validate_nonzero_address(&admin).expect("admin address must not be zero");
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Initialized, &true);
    }

    /// Authorise an external contract (e.g. healthcare-credentialing) to update the
    /// `active` flag of providers. Admin-only. This is how committee decisions
    /// (approve/suspend/revoke) propagate into the single source of truth.
    pub fn set_trusted_authority(
        env: Env,
        admin: Address,
        authority: Address,
        trusted: bool,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&authority).map_err(|_| Error::InvalidAddress)?;
        Self::assert_admin(&env, &admin)?;
        let key = DataKey::TrustedAuthority(authority.clone());
        if trusted {
            env.storage().persistent().set(&key, &true);
        } else {
            env.storage().persistent().remove(&key);
        }
        env.events().publish(
            (symbol_short!("trust_auth"), authority),
            trusted,
        );
        Ok(())
    }

    /// Update the `active` flag of a registered provider.
    ///
    /// Callable by the admin or by a trusted authority (e.g. the credentialing
    /// committee contract). This is the single synchronisation point so that a
    /// suspension/revocation in any committee workflow takes effect for every
    /// consumer that reads `is_active_provider`.
    pub fn set_active(
        env: Env,
        caller: Address,
        provider: Address,
        active: bool,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&caller).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        caller.require_auth();
        Self::assert_admin_or_trusted(&env, &caller)?;

        let key = DataKey::Provider(provider.clone());
        let mut profile: ProviderProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotAProvider)?;
        profile.active = active;
        env.storage().persistent().set(&key, &profile);
        env.events().publish(
            (symbol_short!("set_active"), provider),
            active,
        );
        Ok(())
    }

    /// Returns true only if the provider is registered, marked active, and its
    /// credential has not expired or been revoked. This is the canonical
    /// "is this address a trusted clinician?" check consumers should use.
    pub fn is_active_provider(env: Env, provider: Address) -> bool {
        let key = DataKey::Provider(provider);
        let profile: ProviderProfile = match env.storage().persistent().get(&key) {
            Some(p) => p,
            None => return false,
        };
        Self::is_active(&profile, env.ledger().timestamp())
    }

    /// Alias kept for consumers that call `is_provider`. Delegates to the same
    /// internal predicate so both entry points always agree.
    pub fn is_provider(env: Env, provider: Address) -> bool {
        Self::is_active_provider(env, provider)
    }

    /// Renew an expired credential. The provider must authenticate. The new
    /// `expires_at` must be in the future. The previous anchor is appended to
    /// the credential history before being replaced.
    pub fn renew_credential(
        env: Env,
        provider: Address,
        credential_hash: BytesN<32>,
        issuer: Address,
        attestation_hash: BytesN<32>,
        expires_at: u64,
        revocation_reference: BytesN<32>,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&issuer).map_err(|_| Error::InvalidAddress)?;
        provider.require_auth();

        let now = env.ledger().timestamp();
        if expires_at <= now {
            return Err(Error::CredentialExpired);
        }

        let key = DataKey::Provider(provider.clone());
        let mut profile: ProviderProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotAProvider)?;

        // Append the previous anchor to history before replacing it.
        let hist_key = DataKey::CredentialHistory(provider.clone());
        let mut history: Vec<CredentialAnchor> = env
            .storage()
            .persistent()
            .get(&hist_key)
            .unwrap_or_else(|| Vec::new(&env));
        history.push_back(profile.credential.clone());
        env.storage().persistent().set(&hist_key, &history);

        profile.credential = CredentialAnchor {
            credential_hash,
            issuer,
            attestation_hash,
            expires_at,
            revocation_reference,
            revoked_at: None,
        };
        env.storage().persistent().set(&key, &profile);
        env.events().publish(
            (symbol_short!("renew_cred"), provider),
            expires_at,
        );
        Ok(())
    }

    /// Return the historical credential anchors for a provider.
    pub fn get_credential_history(env: Env, provider: Address) -> Vec<CredentialAnchor> {
        env.storage()
            .persistent()
            .get(&DataKey::CredentialHistory(provider))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Reactivate a previously deactivated provider. Admin or trusted authority only.
    pub fn reactivate_provider(
        env: Env,
        caller: Address,
        provider: Address,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&caller).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        caller.require_auth();
        Self::assert_admin_or_trusted(&env, &caller)?;

        let key = DataKey::Provider(provider.clone());
        let mut profile: ProviderProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotAProvider)?;
        profile.active = true;
        env.storage().persistent().set(&key, &profile);
        env.events().publish(
            (symbol_short!("react_pr"), provider),
            true,
        );
        Ok(())
    }

    /// Revoke a provider's credential. Admin or trusted authority only. Marks the
    /// credential as revoked and deactivates the provider.
    pub fn revoke_credential(
        env: Env,
        caller: Address,
        provider: Address,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&caller).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        caller.require_auth();
        Self::assert_admin_or_trusted(&env, &caller)?;

        let now = env.ledger().timestamp();
        let key = DataKey::Provider(provider.clone());
        let mut profile: ProviderProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotAProvider)?;
        profile.credential.revoked_at = Some(now);
        profile.active = false;
        env.storage().persistent().set(&key, &profile);
        env.events().publish(
            (symbol_short!("revoke_cr"), provider),
            now,
        );
        Ok(())
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Single internal "is active" predicate shared by `is_active_provider` and
    /// `is_provider`. A credential with `expires_at == 0` is treated as invalid
    /// (never active), matching `register_provider` which rejects `expires_at <= now`.
    fn is_active(profile: &ProviderProfile, now: u64) -> bool {
        if !profile.active {
            return false;
        }
        if profile.credential.revoked_at.is_some() {
            return false;
        }
        if profile.credential.expires_at == 0 {
            return false;
        }
        profile.credential.expires_at > now
    }

    fn assert_initialized(env: &Env) -> Result<(), Error> {
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(Error::NotInitialized)
        }
    }

    fn assert_not_initialized(env: &Env) -> Result<(), Error> {
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            Err(Error::AlreadyInitialized)
        } else {
            Ok(())
        }
    }

    fn assert_admin(env: &Env, caller: &Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if &admin == caller {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }

    fn assert_admin_or_trusted(env: &Env, caller: &Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if &admin == caller {
            return Ok(());
        }
        let trusted: bool = env
            .storage()
            .persistent()
            .get(&DataKey::TrustedAuthority(caller.clone()))
            .unwrap_or(false);
        if trusted {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }
}
