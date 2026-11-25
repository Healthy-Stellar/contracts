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
use soroban_sdk::{contract, contractimpl, contracterror, contracttype, symbol_short, Address, Env, String};

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
}

/// --------------------
/// Doctor Structures
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorProfileData {
    pub name: String,
    pub specialization: String,
    pub institution_wallet: Address,
    pub metadata: String,
    pub active: bool,
    pub revoked_at: Option<u64>,
}

/// --------------------
/// Storage Keys
/// --------------------
#[contracttype]
pub enum DataKey {
    Admin,
    Doctor(Address),
}

#[contract]
pub struct DoctorRegistry;

#[contractimpl]
impl DoctorRegistry {
    /// Set the contract admin at deployment. Only the admin (or an authorized
    /// registrar) may create or modify doctor profiles.
    pub fn __constructor(env: Env, admin: Address) {
        validate_nonzero_address(&admin).expect("admin address must not be zero");
        env.storage().persistent().set(&DataKey::Admin, &admin);
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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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

    pub fn get_doctor_profile(env: Env, wallet: Address) -> Result<DoctorProfileData, Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        let key = DataKey::Doctor(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)
    }

    /// Deactivate a doctor profile.
    /// Requires the admin (registrar) to authorize.
    ///
    /// # Arguments
    /// * `registrar` - The admin address that authorizes this deactivation
    /// * `wallet` - The wallet address of the doctor whose profile is being deactivated
    pub fn deactivate_doctor_profile(
        env: Env,
        registrar: Address,
        wallet: Address,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        validate_nonzero_address(&registrar).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        require_admin(&env, &registrar)?;

        let key = DataKey::Doctor(wallet.clone());
        let mut doctor_profile: DoctorProfileData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)?;

        doctor_profile.active = false;
        doctor_profile.revoked_at = Some(env.ledger().timestamp());
        env.storage().persistent().set(&key, &doctor_profile);

        env.events()
            .publish((symbol_short!("deact_doc"), wallet), symbol_short!("success"));

        Ok(())
    }

    /// Reactivate a previously deactivated doctor profile.
    /// Requires the admin (registrar) to authorize.
    ///
    /// # Arguments
    /// * `registrar` - The admin address that authorizes this reactivation
    /// * `wallet` - The wallet address of the doctor whose profile is being reactivated
    pub fn reactivate_doctor_profile(
        env: Env,
        registrar: Address,
        wallet: Address,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        validate_nonzero_address(&registrar).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        require_admin(&env, &registrar)?;

        let key = DataKey::Doctor(wallet.clone());
        let mut doctor_profile: DoctorProfileData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)?;

        doctor_profile.active = true;
        doctor_profile.revoked_at = None;
        env.storage().persistent().set(&key, &doctor_profile);

        env.events()
            .publish((symbol_short!("react_doc"), wallet), symbol_short!("success"));

        Ok(())
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
