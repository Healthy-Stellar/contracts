#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

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
    Doctor(Address),
}

#[contract]
pub struct DoctorRegistry;

#[contractimpl]
impl DoctorRegistry {
    /// Create a new doctor profile with basic information and institution association
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the doctor
    /// * `name` - The name of the doctor
    /// * `specialization` - The area of specialization
    /// * `institution_wallet` - The wallet address of the associated hospital/clinic
    pub fn create_doctor_profile(
        env: Env,
        wallet: Address,
        name: String,
        specialization: String,
        institution_wallet: Address,
    ) {
        wallet.require_auth();

        let key = DataKey::Doctor(wallet.clone());
        if env.storage().persistent().has(&key) {
            panic!("Doctor profile already exists");
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
    }

    /// Update doctor profile specialization and metadata
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the doctor
    /// * `specialization` - Updated area of specialization
    /// * `metadata` - Additional information (credentials, certifications, etc.)
    pub fn update_doctor_profile(
        env: Env,
        wallet: Address,
        specialization: String,
        metadata: String,
    ) {
        wallet.require_auth();

        let key = DataKey::Doctor(wallet.clone());
        let mut doctor_profile: DoctorProfileData = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Doctor profile not found");

        doctor_profile.specialization = specialization;
        doctor_profile.metadata = metadata;
        env.storage().persistent().set(&key, &doctor_profile);

        env.events()
            .publish((symbol_short!("upd_doc"), wallet), symbol_short!("success"));
    }

    /// Retrieve doctor profile data by wallet address
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the doctor
    ///
    /// # Returns
    /// The DoctorProfileData for the given wallet address
    pub fn get_doctor_profile(env: Env, wallet: Address) -> DoctorProfileData {
        let key = DataKey::Doctor(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .expect("Doctor profile not found")
    }
}

mod test;
