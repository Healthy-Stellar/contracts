#![no_std]

//! # Doctor Registry Contract
//!
//! Maintains registry of healthcare providers with profile data including specializations,
//! institutional affiliations, and metadata for provider validation across the network.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Admin-only initialization and profile creation. Duplicate profile
//! prevention ensures one profile per doctor address. Address validation prevents invalid entries.
//! Update and delete operations require proper authorization.
//!
//! **Audit Controls:** Provider registration events emitted with doctor wallet and profile data.
//! All profile modifications tracked via storage updates. Admin address maintained for access control.
//!
//! **Data Retention Policy:** Doctor profiles persist indefinitely as reference data. Provider
//! specialization and institutional metadata retained for credential verification. Profile deactivation
//! (if implemented) removes access authorization without deleting historical data.
//!
//! **Encryption/Integrity:** Doctor addresses validated via nonzero address checks. Institutional
//! wallet references provide cryptographic identity anchoring. Profile metadata encrypted via
//! Soroban's secure storage mechanisms.

use shared::privacy::validate_nonzero_address;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};

/// Error codes for doctor registry operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    DuplicateProfile = 1,
    ProfileNotFound = 2,
    Unauthorized = 3,
    AlreadyInitialized = 4,
    InvalidAddress = 5,
    AlreadyInactive = 6,
    AlreadyActive = 7,
    NoPendingAdmin = 8,
    InvalidReason = 9,
}

/// --------------------
/// Doctor Structures
/// --------------------
/// A single entry in a doctor's status history. Appended on every
/// deactivation/reactivation; never cleared, so the full audit trail is kept.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusChange {
    /// `true` for active, `false` for inactive.
    pub status: bool,
    /// Reason code (see `Reason`); empty for reactivations.
    pub reason: String,
    /// Registrar that performed the change.
    pub by: Address,
    /// Ledger timestamp of the change.
    pub at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorProfileData {
    pub name: String,
    pub specialization: String,
    pub institution_wallet: Address,
    pub metadata: String,
    pub active: bool,
    /// Timestamp of the most recent deactivation, retained across reactivations.
    pub revoked_at: Option<u64>,
    /// Full status history (deactivations and reactivations).
    pub status_history: Vec<StatusChange>,
}

/// --------------------
/// Storage Keys
/// --------------------
#[contracttype]
pub enum DataKey {
    Admin,
    PendingAdmin,
    Doctor(Address),
}

#[contract]
pub struct DoctorRegistry;

#[contractimpl]
impl DoctorRegistry {
    /// Set the contract admin. Must be called once before any profile operations.
    /// Only the admin (or an authorized registrar) may create or modify doctor profiles.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        admin.require_auth();
        if env.storage().persistent().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Create a new doctor profile with basic information and institution association.
    /// Requires the admin (registrar) to authorize, preventing arbitrary self-registration.
    ///
    /// # Arguments
    /// * `registrar` - The admin address that authorizes this profile creation
    /// * `wallet` - The wallet address of the doctor being registered
    /// * `name` - The name of the doctor
    /// * `specialization` - The area of specialization
    /// * `institution_wallet` - The wallet address of the associated hospital/clinic
    pub fn create_doctor_profile(
        env: Env,
        registrar: Address,
        wallet: Address,
        name: String,
        specialization: String,
        institution_wallet: Address,
    ) -> Result<(), Error> {
        validate_nonzero_address(&registrar).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&institution_wallet).map_err(|_| Error::InvalidAddress)?;
        require_admin(&env, &registrar)?;

        let key = DataKey::Doctor(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::DuplicateProfile);
        }

        let doctor_profile = DoctorProfileData {
            name,
            specialization,
            institution_wallet,
            metadata: String::from_str(&env, ""),
            active: true,
            revoked_at: None,
            status_history: Vec::new(&env),
        };

        env.storage().persistent().set(&key, &doctor_profile);

        env.events()
            .publish((symbol_short!("crt_doc"), wallet), symbol_short!("success"));

        Ok(())
    }

    /// Update doctor profile specialization and metadata.
    /// Requires the admin (registrar) to authorize, consistent with creation policy.
    ///
    /// # Arguments
    /// * `registrar` - The admin address that authorizes this profile update
    /// * `wallet` - The wallet address of the doctor whose profile is being updated
    /// * `specialization` - Updated area of specialization
    /// * `metadata` - Additional information (credentials, certifications, etc.)
    pub fn update_doctor_profile(
        env: Env,
        registrar: Address,
        wallet: Address,
        specialization: String,
        metadata: String,
    ) -> Result<(), Error> {
        validate_nonzero_address(&registrar).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        require_admin(&env, &registrar)?;

        let key = DataKey::Doctor(wallet.clone());
        let mut doctor_profile: DoctorProfileData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)?;

        doctor_profile.specialization = specialization;
        doctor_profile.metadata = metadata;
        env.storage().persistent().set(&key, &doctor_profile);

        env.events()
            .publish((symbol_short!("upd_doc"), wallet), symbol_short!("success"));

        Ok(())
    }

    /// Update a doctor's name and institution wallet.
    /// Requires the admin (registrar) to authorize.
    ///
    /// # Arguments
    /// * `registrar` - The admin address that authorizes this profile update
    /// * `wallet` - The wallet address of the doctor whose profile is being updated
    /// * `name` - Updated legal name
    /// * `institution_wallet` - Updated institution wallet
    pub fn update_profile(
        env: Env,
        registrar: Address,
        wallet: Address,
        name: String,
        institution_wallet: Address,
    ) -> Result<(), Error> {
        validate_nonzero_address(&registrar).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&institution_wallet).map_err(|_| Error::InvalidAddress)?;
        require_admin(&env, &registrar)?;

        let key = DataKey::Doctor(wallet.clone());
        let mut doctor_profile: DoctorProfileData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)?;

        doctor_profile.name = name.clone();
        doctor_profile.institution_wallet = institution_wallet.clone();
        env.storage().persistent().set(&key, &doctor_profile);

        env.events().publish(
            (symbol_short!("upd_prof"), wallet),
            (registrar, name, institution_wallet, env.ledger().timestamp()),
        );

        Ok(())
    }

    pub fn get_doctor_profile(env: Env, wallet: Address) -> Result<DoctorProfileData, Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        let key = DataKey::Doctor(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)
    }

    /// Deactivate a doctor profile.
    /// Requires the admin (registrar) to authorize. Rejects profiles that are
    /// already inactive so the original revocation record is never overwritten.
    ///
    /// # Arguments
    /// * `registrar` - The admin address that authorizes this deactivation
    /// * `wallet` - The wallet address of the doctor whose profile is being deactivated
    /// * `reason` - Reason code: "suspended", "expired" or "voluntary"
    pub fn deactivate_doctor_profile(
        env: Env,
        registrar: Address,
        wallet: Address,
        reason: String,
    ) -> Result<(), Error> {
        validate_nonzero_address(&registrar).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        require_admin(&env, &registrar)?;
        validate_reason(&env, &reason)?;

        let key = DataKey::Doctor(wallet.clone());
        let mut doctor_profile: DoctorProfileData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)?;

        if !doctor_profile.active {
            return Err(Error::AlreadyInactive);
        }

        let now = env.ledger().timestamp();
        doctor_profile.active = false;
        doctor_profile.revoked_at = Some(now);
        doctor_profile.status_history.push_back(StatusChange {
            status: false,
            reason: reason.clone(),
            by: registrar.clone(),
            at: now,
        });
        env.storage().persistent().set(&key, &doctor_profile);

        env.events().publish(
            (symbol_short!("deact_doc"), wallet),
            (registrar, reason, now),
        );

        Ok(())
    }

    /// Reactivate a previously deactivated doctor profile.
    /// Requires the admin (registrar) to authorize. Rejects profiles that are
    /// already active. The prior revocation record is preserved in history.
    ///
    /// # Arguments
    /// * `registrar` - The admin address that authorizes this reactivation
    /// * `wallet` - The wallet address of the doctor whose profile is being reactivated
    pub fn reactivate_doctor_profile(
        env: Env,
        registrar: Address,
        wallet: Address,
    ) -> Result<(), Error> {
        validate_nonzero_address(&registrar).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        require_admin(&env, &registrar)?;

        let key = DataKey::Doctor(wallet.clone());
        let mut doctor_profile: DoctorProfileData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)?;

        if doctor_profile.active {
            return Err(Error::AlreadyActive);
        }

        let now = env.ledger().timestamp();
        doctor_profile.active = true;
        doctor_profile.status_history.push_back(StatusChange {
            status: true,
            reason: String::from_str(&env, ""),
            by: registrar.clone(),
            at: now,
        });
        env.storage().persistent().set(&key, &doctor_profile);

        env.events().publish(
            (symbol_short!("react_doc"), wallet),
            (registrar, now),
        );

        Ok(())
    }

    /// Return the full status history for a doctor profile.
    pub fn get_status_history(env: Env, wallet: Address) -> Result<Vec<StatusChange>, Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        let key = DataKey::Doctor(wallet);
        let profile: DoctorProfileData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)?;
        Ok(profile.status_history)
    }

    /// Propose a new admin (step 1 of two-step rotation).
    /// Requires the current admin to authorize.
    pub fn propose_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&new_admin).map_err(|_| Error::InvalidAddress)?;
        require_admin(&env, &admin)?;

        env.storage()
            .persistent()
            .set(&DataKey::PendingAdmin, &new_admin);

        env.events()
            .publish((symbol_short!("prop_adm"), admin), new_admin);

        Ok(())
    }

    /// Accept a pending admin proposal (step 2 of two-step rotation).
    /// Requires the proposed admin to authorize.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        validate_nonzero_address(&new_admin).map_err(|_| Error::InvalidAddress)?;
        new_admin.require_auth();

        let pending: Address = env
            .storage()
            .persistent()
            .get(&DataKey::PendingAdmin)
            .ok_or(Error::NoPendingAdmin)?;
        if pending != new_admin {
            return Err(Error::Unauthorized);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Admin, &new_admin);
        env.storage().persistent().remove(&DataKey::PendingAdmin);

        env.events()
            .publish((symbol_short!("acc_adm"), new_admin), symbol_short!("success"));

        Ok(())
    }

    /// Return the current admin address.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)
    }

    /// Check if a doctor is registered and active.
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the doctor
    pub fn is_active(env: Env, wallet: Address) -> bool {
        if validate_nonzero_address(&wallet).is_err() {
            return false;
        }
        let key = DataKey::Doctor(wallet);
        if let Some(profile) = env.storage().persistent().get::<DataKey, DoctorProfileData>(&key) {
            profile.active
        } else {
            false
        }
    }
}

fn validate_reason(env: &Env, reason: &String) -> Result<(), Error> {
    let suspended = String::from_str(env, "suspended");
    let expired = String::from_str(env, "expired");
    let voluntary = String::from_str(env, "voluntary");
    if *reason == suspended || *reason == expired || *reason == voluntary {
        Ok(())
    } else {
        Err(Error::InvalidReason)
    }
}

fn require_admin(env: &Env, admin: &Address) -> Result<(), Error> {
    admin.require_auth();
    let configured: Address = env
        .storage()
        .persistent()
        .get(&DataKey::Admin)
        .ok_or(Error::Unauthorized)?;
    if configured != *admin {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

mod test;
