#![no_std]

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
    Record(String),
    ProviderRecords(Address),
    ProviderRecordCount(Address),
    RateLimitConfig,
    ProviderRate(Address),
    ProviderReputation(Address),
    ProviderRatingByPatient(Address, Address),
    PendingAdmin,
    RotationExpiry,
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
        env.storage()
            .persistent()
            .set(&DataKey::Provider(provider.clone()), &profile);
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
            env.events().publish(
                (symbol_short!("reg_prov"), entry.provider.clone()),
                symbol_short!("ok"),
            );
            results.push_back(BatchEntryStatus::Success);
        }

        Ok(results)
    }

    pub fn revoke_provider(env: Env, admin: Address, provider: Address) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        Self::assert_admin(&env, &admin)?;
        let key = DataKey::Provider(provider.clone());
        let mut profile: ProviderProfile = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::RecordNotFound)?;

        profile.active = false;
        profile.credential.revoked_at = Some(env.ledger().timestamp());
        env.storage().persistent().set(&key, &profile);

        env.events()
            .publish((symbol_short!("rev_prov"), provider), symbol_short!("ok"));
        Ok(())
    }

    pub fn is_provider(env: Env, provider: Address) -> bool {
        Self::is_provider_active(&env, &provider)
    }

    fn is_provider_active(env: &Env, provider: &Address) -> bool {
        if let Some(profile) = env.storage().persistent().get::<DataKey, ProviderProfile>(&DataKey::Provider(provider.clone())) {
            profile.active && profile.credential.revoked_at.is_none() && profile.credential.expires_at > env.ledger().timestamp()
        } else {
            false
        }
    }

    pub fn get_provider_profile(
        env: Env,
        provider: Address,
    ) -> Result<ProviderProfile, Error> {
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        env.storage()
            .persistent()
            .get(&DataKey::Provider(provider))
            .ok_or(Error::RecordNotFound)
    }

    pub fn add_record(
        env: Env,
        provider: Address,
        record_id: String,
        data: String,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        provider.require_auth();
        if !Self::is_provider(env.clone(), provider.clone()) {
            return Err(Error::NotAProvider);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id.clone()), &data);
        env.events()
            .publish((symbol_short!("add_rec"), provider, record_id), symbol_short!("ok"));
        Ok(())
    }

    pub fn get_record(env: Env, record_id: String) -> Result<String, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)
    }

    /// Propose transferring admin to `new_admin`. Must be confirmed within 24 hours.
    pub fn propose_admin_rotation(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        Self::assert_admin(&env, &admin)?;
        if env.storage().persistent().has(&DataKey::PendingAdmin) {
            return Err(Error::RotationPending);
        }
        let expiry = env.ledger().timestamp() + ADMIN_ROTATION_WINDOW;
        env.storage().persistent().set(&DataKey::PendingAdmin, &new_admin);
        env.storage().persistent().set(&DataKey::RotationExpiry, &expiry);
        Ok(())
    }

    /// New admin confirms the rotation proposed by the current admin.
    pub fn accept_admin_rotation(env: Env, new_admin: Address) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        new_admin.require_auth();
        let pending: Address = env
            .storage()
            .persistent()
            .get(&DataKey::PendingAdmin)
            .ok_or(Error::NoRotationPending)?;
        if new_admin != pending {
            return Err(Error::NotPendingAdmin);
        }
        let expiry: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RotationExpiry)
            .unwrap_or(0);
        if env.ledger().timestamp() > expiry {
            env.storage().persistent().remove(&DataKey::PendingAdmin);
            env.storage().persistent().remove(&DataKey::RotationExpiry);
            return Err(Error::RotationExpired);
        }
        env.storage().persistent().set(&DataKey::Admin, &new_admin);
        env.storage().persistent().remove(&DataKey::PendingAdmin);
        env.storage().persistent().remove(&DataKey::RotationExpiry);
        Ok(())
    }

    // ── guards ────────────────────────────────────────────────────────────────

    fn assert_initialized(env: &Env) -> Result<(), Error> {
        if !env.storage().persistent().has(&DataKey::Initialized) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn assert_not_initialized(env: &Env) -> Result<(), Error> {
        if env.storage().persistent().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }
        Ok(())
    }

    fn assert_admin(env: &Env, caller: &Address) -> Result<(), Error> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if *caller != admin {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }
}
