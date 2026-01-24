#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

/// --------------------
/// Hospital Structures
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HospitalData {
    pub name: String,
    pub location: String,
    pub metadata: String, // Services, departments, accreditation info
}

/// --------------------
/// Storage Keys
/// --------------------
#[contracttype]
pub enum DataKey {
    Hospital(Address),
}

#[contract]
pub struct HospitalRegistry;

#[contractimpl]
impl HospitalRegistry {
    /// Register a new hospital with basic information
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the hospital
    /// * `name` - The name of the hospital
    /// * `location` - The physical location/address of the hospital
    /// * `metadata` - Additional information (services, departments, etc.)
    pub fn register_hospital(
        env: Env,
        wallet: Address,
        name: String,
        location: String,
        metadata: String,
    ) {
        wallet.require_auth();

        let key = DataKey::Hospital(wallet.clone());
        if env.storage().persistent().has(&key) {
            panic!("Hospital already registered");
        }

        let hospital = HospitalData {
            name,
            location,
            metadata,
        };

        env.storage().persistent().set(&key, &hospital);

        env.events().publish(
            (symbol_short!("reg_hosp"), wallet),
            symbol_short!("success"),
        );
    }

    /// Update hospital metadata
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the hospital
    /// * `metadata` - Updated metadata information
    pub fn update_hospital(env: Env, wallet: Address, metadata: String) {
        wallet.require_auth();

        let key = DataKey::Hospital(wallet.clone());
        let mut hospital: HospitalData = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital not found");

        hospital.metadata = metadata;
        env.storage().persistent().set(&key, &hospital);

        env.events().publish(
            (symbol_short!("upd_hosp"), wallet),
            symbol_short!("success"),
        );
    }

    /// Retrieve hospital data by wallet address
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the hospital
    ///
    /// # Returns
    /// The HospitalData for the given wallet address
    pub fn get_hospital(env: Env, wallet: Address) -> HospitalData {
        let key = DataKey::Hospital(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .expect("Hospital not found")
    }
}

mod test;
