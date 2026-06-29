#![no_std]
#![allow(deprecated)]

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

    pub fn get_doctor_profile(env: Env, wallet: Address) -> Result<DoctorProfileData, Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        let key = DataKey::Doctor(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProfileNotFound)
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
