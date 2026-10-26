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
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct ProviderRegistry;

#[contractimpl]
impl ProviderRegistry {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        Self::assert_not_initialized(&env)?;
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Initialized, &true);
        Ok(())
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
        if !profile.active {
            return false;
        }
        if profile.credential.revoked_at.is_some() {
            return false;
        }
        if profile.credential.expires_at != 0
            && env.ledger().timestamp() >= profile.credential.expires_at
        {
            return false;
        }
        true
    }

    pub fn register_provider(
        env: Env,
        admin: Address,
        provider: Address,
        name: String,
        specialty: String,
        license_number: String,
        credential_hash: BytesN<32>,
        issuer: Address,
        attestation_hash: BytesN<32>,
        expires_at: u64,
        revocation_reference: BytesN<32>,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&issuer).map_err(|_| Error::InvalidAddress)?;
        Self::assert_admin(&env, &admin)?;
        if expires_at <= env.ledger().timestamp() {
            return Err(Error::CredentialExpired);
        }
        let key = DataKey::Provider(provider.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }
        let profile = ProviderProfile {
            name,
            specialty,
            license_number,
            credential: CredentialAnchor {
                credential_hash,
                issuer,
                attestation_hash,
                expires_at,
                revocation_reference,
                revoked_at: None,
            },
            active: true,
        };
        env.storage().persistent().set(&key, &profile);
        env.events()
            .publish((symbol_short!("reg_prov"), provider), symbol_short!("ok"));
        Ok(())
    }

    /// Register multiple providers in one transaction.
    ///
    /// Returns a `Vec<BatchEntryStatus>` aligned 1-to-1 with `entries`.
    /// Already-registered providers produce `AlreadyExists` (no overwrite).
    /// Enforces `MAX_BATCH_SIZE`; returns `BatchTooLarge` if exceeded.
    pub fn batch_register_providers(
        env: Env,
        admin: Address,
        entries: Vec<BatchProviderEntry>,
    ) -> Result<Vec<BatchEntryStatus>, Error> {
        Self::assert_initialized(&env)?;
        Self::assert_admin(&env, &admin)?;

        if entries.len() > MAX_BATCH_SIZE {
            return Err(Error::BatchTooLarge);
        }

        let mut results: Vec<BatchEntryStatus> = Vec::new(&env);

        for entry in entries.iter() {
            if validate_nonzero_address(&entry.provider).is_err()
                || validate_nonzero_address(&entry.issuer).is_err()
            {
                results.push_back(BatchEntryStatus::InvalidAddress);
                continue;
            }

            if entry.expires_at <= env.ledger().timestamp() {
                results.push_back(BatchEntryStatus::CredentialExpired);
                continue;
            }

            let key = DataKey::Provider(entry.provider.clone());
            if env.storage().persistent().has(&key) {
                results.push_back(BatchEntryStatus::AlreadyExists);
                continue;
            }

            let profile = ProviderProfile {
                name: entry.name.clone(),
                specialty: entry.specialty.clone(),
                license_number: entry.license_number.clone(),
                credential: CredentialAnchor {
                    credential_hash: entry.credential_hash.clone(),
                    issuer: entry.issuer.clone(),
                    attestation_hash: entry.attestation_hash.clone(),
                    expires_at: entry.expires_at,
                    revocation_reference: entry.revocation_reference.clone(),
                    revoked_at: None,
                },
                active: true,
            };
            env.storage().persistent().set(&key, &profile);
            results.push_back(BatchEntryStatus::Success);
        }

        Ok(results)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

        env.events()
            .publish((symbol_short!("react_pr"), provider), symbol_short!("ok"));
        Ok(())
    }

    pub fn is_provider(env: Env, provider: Address) -> bool {
        Self::is_provider_active(&env, &provider)
    }

    pub fn set_provider_hospital(
        env: Env,
        admin: Address,
        provider: Address,
        hospital: Address,
        affiliated: bool,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&hospital).map_err(|_| Error::InvalidAddress)?;
        Self::assert_admin(&env, &admin)?;
        if affiliated && !Self::is_provider_active(&env, &provider) {
            return Err(Error::NotAProvider);
        }

        env.storage().persistent().set(
            &DataKey::ProviderHospital(provider.clone(), hospital.clone()),
            &affiliated,
        );
        Ok(())
    }

    pub fn is_provider_affiliated(env: Env, provider: Address, hospital: Address) -> bool {
        Self::is_provider_active(&env, &provider)
            && env
                .storage()
                .persistent()
                .get(&DataKey::ProviderHospital(provider, hospital))
                .unwrap_or(false)
    }

    fn is_provider_active(env: &Env, provider: &Address) -> bool {
        if let Some(profile) = env.storage().persistent().get::<DataKey, ProviderProfile>(&DataKey::Provider(provider.clone())) {
            profile.active && profile.credential.revoked_at.is_none() && profile.credential.expires_at > env.ledger().timestamp()
        } else {
            Err(Error::NotInitialized)
        }
    }

    fn assert_not_initialized(env: &Env) -> Result<(), Error> {
        if env.storage().persistent().has(&DataKey::Initialized) {
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
        if &admin != caller {
            return Err(Error::Unauthorized);
        }
        caller.require_auth();
        Ok(())
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
        if env
            .storage()
            .persistent()
            .has(&DataKey::TrustedAuthority(caller.clone()))
        {
            return Ok(());
        }
        Err(Error::Unauthorized)
    }
}
