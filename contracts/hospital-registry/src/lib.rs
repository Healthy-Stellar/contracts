#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec};

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
/// Hospital Configuration Structures
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Department {
    pub name: String,
    pub head: String,
    pub contact: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    pub name: String,
    pub address: String,
    pub metadata: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EquipmentResource {
    pub name: String,
    pub quantity: u32,
    pub status: String,
    pub metadata: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyProcedure {
    pub title: String,
    pub version: String,
    pub details: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlertSetting {
    pub alert_type: String,
    pub enabled: bool,
    pub channels: Vec<String>,
    pub escalation_contact: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsuranceProviderConfig {
    pub provider_name: String,
    pub plan_codes: Vec<String>,
    pub billing_contact: String,
    pub metadata: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BillingConfig {
    pub currency: String,
    pub payment_terms: String,
    pub tax_id: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyProtocol {
    pub protocol_name: String,
    pub description: String,
    pub last_updated: u64,
    pub contact: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HospitalConfig {
    pub departments: Vec<Department>,
    pub locations: Vec<Location>,
    pub equipment: Vec<EquipmentResource>,
    pub policies: Vec<PolicyProcedure>,
    pub alerts: Vec<AlertSetting>,
    pub insurance_providers: Vec<InsuranceProviderConfig>,
    pub billing: BillingConfig,
    pub emergency_protocols: Vec<EmergencyProtocol>,
}

/// --------------------
/// Storage Keys
/// --------------------
#[contracttype]
pub enum DataKey {
    Hospital(Address),
    HospitalConfig(Address),
}

#[contract]
pub struct HospitalRegistry;

#[contractimpl]
impl HospitalRegistry {
    fn assert_hospital_exists(env: &Env, wallet: &Address) {
        let key = DataKey::Hospital(wallet.clone());
        if !env.storage().persistent().has(&key) {
            panic!("Hospital not found");
        }
    }

    fn default_config(env: &Env) -> HospitalConfig {
        HospitalConfig {
            departments: Vec::new(env),
            locations: Vec::new(env),
            equipment: Vec::new(env),
            policies: Vec::new(env),
            alerts: Vec::new(env),
            insurance_providers: Vec::new(env),
            billing: BillingConfig {
                currency: String::from_str(env, ""),
                payment_terms: String::from_str(env, ""),
                tax_id: String::from_str(env, ""),
            },
            emergency_protocols: Vec::new(env),
        }
    }

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

        let config_key = DataKey::HospitalConfig(wallet.clone());
        let config = Self::default_config(&env);
        env.storage().persistent().set(&config_key, &config);

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

    /// Set full hospital configuration in one call
    pub fn set_hospital_config(env: Env, wallet: Address, config: HospitalConfig) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        env.storage().persistent().set(&key, &config);

        env.events()
            .publish((symbol_short!("cfg_set"), wallet), symbol_short!("success"));
    }

    /// Retrieve hospital configuration
    pub fn get_hospital_config(env: Env, wallet: Address) -> HospitalConfig {
        let key = DataKey::HospitalConfig(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found")
    }

    pub fn update_departments(env: Env, wallet: Address, departments: Vec<Department>) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        let mut config: HospitalConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found");

        config.departments = departments;
        env.storage().persistent().set(&key, &config);

        env.events().publish(
            (symbol_short!("upd_dept"), wallet),
            symbol_short!("success"),
        );
    }

    pub fn update_locations(env: Env, wallet: Address, locations: Vec<Location>) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        let mut config: HospitalConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found");

        config.locations = locations;
        env.storage().persistent().set(&key, &config);

        env.events()
            .publish((symbol_short!("upd_loc"), wallet), symbol_short!("success"));
    }

    pub fn update_equipment(env: Env, wallet: Address, equipment: Vec<EquipmentResource>) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        let mut config: HospitalConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found");

        config.equipment = equipment;
        env.storage().persistent().set(&key, &config);

        env.events()
            .publish((symbol_short!("upd_eq"), wallet), symbol_short!("success"));
    }

    pub fn update_policies(env: Env, wallet: Address, policies: Vec<PolicyProcedure>) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        let mut config: HospitalConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found");

        config.policies = policies;
        env.storage().persistent().set(&key, &config);

        env.events()
            .publish((symbol_short!("upd_pol"), wallet), symbol_short!("success"));
    }

    pub fn update_alerts(env: Env, wallet: Address, alerts: Vec<AlertSetting>) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        let mut config: HospitalConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found");

        config.alerts = alerts;
        env.storage().persistent().set(&key, &config);

        env.events().publish(
            (symbol_short!("upd_alrt"), wallet),
            symbol_short!("success"),
        );
    }

    pub fn update_insurance_providers(
        env: Env,
        wallet: Address,
        insurance_providers: Vec<InsuranceProviderConfig>,
    ) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        let mut config: HospitalConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found");

        config.insurance_providers = insurance_providers;
        env.storage().persistent().set(&key, &config);

        env.events()
            .publish((symbol_short!("upd_ins"), wallet), symbol_short!("success"));
    }

    pub fn update_billing(env: Env, wallet: Address, billing: BillingConfig) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        let mut config: HospitalConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found");

        config.billing = billing;
        env.storage().persistent().set(&key, &config);

        env.events().publish(
            (symbol_short!("upd_bill"), wallet),
            symbol_short!("success"),
        );
    }

    pub fn update_emergency_protocols(
        env: Env,
        wallet: Address,
        protocols: Vec<EmergencyProtocol>,
    ) {
        wallet.require_auth();
        Self::assert_hospital_exists(&env, &wallet);

        let key = DataKey::HospitalConfig(wallet.clone());
        let mut config: HospitalConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Hospital config not found");

        config.emergency_protocols = protocols;
        env.storage().persistent().set(&key, &config);

        env.events()
            .publish((symbol_short!("upd_emg"), wallet), symbol_short!("success"));
    }
}

mod test;
