#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

use ttl_config::{extend_critical_ttl, extend_critical_ttl_if_exists};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllergyRecord {
    pub patient: Address,
    pub allergen: String,
    pub reaction: String,
    pub severity: u32,
    pub recorded_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Allergy(Address, u32),
    AllergyCount(Address),
}

#[contract]
pub struct AllergyTrackingContract;

#[contractimpl]
impl AllergyTrackingContract {
    /// Record a new allergy for a patient.
    pub fn add_allergy(
        env: Env,
        patient: Address,
        allergen: String,
        reaction: String,
        severity: u32,
    ) -> u32 {
        patient.require_auth();

        let count_key = DataKey::AllergyCount(patient.clone());
        let index: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);

        let record = AllergyRecord {
            patient: patient.clone(),
            allergen,
            reaction,
            severity,
            recorded_at: env.ledger().timestamp(),
        };

        let record_key = DataKey::Allergy(patient.clone(), index);
        env.storage().persistent().set(&record_key, &record);
        extend_critical_ttl(&env, &record_key);

        env.storage().persistent().set(&count_key, &(index + 1));
        extend_critical_ttl(&env, &count_key);

        extend_critical_ttl(&env, &DataKey::AllergyCount(patient));

        index
    }

    /// Fetch a single allergy record for a patient.
    pub fn get_allergy(env: Env, patient: Address, index: u32) -> Option<AllergyRecord> {
        let key = DataKey::Allergy(patient, index);
        extend_critical_ttl_if_exists(&env, &key);
        env.storage().persistent().get(&key)
    }

    /// List all allergy records for a patient.
    pub fn get_allergies(env: Env, patient: Address) -> Vec<AllergyRecord> {
        let count_key = DataKey::AllergyCount(patient.clone());
        extend_critical_ttl_if_exists(&env, &count_key);
        let count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);

        let mut records = Vec::new(&env);
        for i in 0..count {
            let key = DataKey::Allergy(patient.clone(), i);
            extend_critical_ttl_if_exists(&env, &key);
            if let Some(record) = env.storage().persistent().get::<DataKey, AllergyRecord>(&key) {
                records.push_back(record);
            }
        }
        records
    }

    /// Remove an allergy record for a patient.
    pub fn remove_allergy(env: Env, patient: Address, index: u32) {
        patient.require_auth();
        let key = DataKey::Allergy(patient.clone(), index);
        env.storage().persistent().remove(&key);
        extend_critical_ttl(&env, &DataKey::AllergyCount(patient));
    }
}
