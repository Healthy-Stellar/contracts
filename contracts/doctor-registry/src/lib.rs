#![no_std]
#![allow(deprecated)]

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
