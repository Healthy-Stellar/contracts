use soroban_sdk::{Address, Env, String, Symbol, Vec};

use crate::{AllergyRecord, DataKey, Error};

/// Get the next allergy ID and increment counter
pub fn get_next_allergy_id(env: &Env) -> u64 {
    let current_id = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::AllergyCounter)
        .unwrap_or(0);
    
    env.storage()
        .instance()
        .set(&DataKey::AllergyCounter, &(current_id + 1));
    
    current_id
}

/// Save an allergy record
pub fn save_allergy(env: &Env, allergy: &AllergyRecord) {
    let key = DataKey::Allergy(allergy.allergy_id);
    env.storage().persistent().set(&key, allergy);
}

/// Get an allergy record by ID
pub fn get_allergy(env: &Env, allergy_id: u64) -> Result<AllergyRecord, Error> {
    let key = DataKey::Allergy(allergy_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(Error::AllergyNotFound)
}

/// Add an allergy ID to a patient's allergy list
pub fn add_patient_allergy(env: &Env, patient_id: &Address, allergy_id: u64) {
    let key = DataKey::PatientAllergies(patient_id.clone());
    let mut allergies: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));
    
    allergies.push_back(allergy_id);
    env.storage().persistent().set(&key, &allergies);
}

/// Get all allergy IDs for a patient
pub fn get_patient_allergies(env: &Env, patient_id: &Address) -> Vec<u64> {
    let key = DataKey::PatientAllergies(patient_id.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

/// Check if a patient already has a specific allergy recorded
pub fn check_duplicate_allergy(
    env: &Env,
    patient_id: &Address,
    allergen: &String,
    allergen_type: &Symbol,
) -> bool {
    let allergy_ids = get_patient_allergies(env, patient_id);
    
    for allergy_id in allergy_ids.iter() {
        if let Ok(allergy) = get_allergy(env, allergy_id) {
            if allergy.allergen == *allergen 
                && allergy.allergen_type == *allergen_type 
                && allergy.status == crate::AllergyStatus::Active {
                return true;
            }
        }
    }
    
    false
}

/// Grant access to a provider to view patient allergies
pub fn grant_access(env: &Env, patient_id: &Address, provider_id: &Address) {
    let key = DataKey::AccessControl(patient_id.clone(), provider_id.clone());
    env.storage().persistent().set(&key, &true);
}

/// Revoke access from a provider
pub fn revoke_access(env: &Env, patient_id: &Address, provider_id: &Address) {
    let key = DataKey::AccessControl(patient_id.clone(), provider_id.clone());
    env.storage().persistent().remove(&key);
}

/// Check if a requester has access to patient allergies
pub fn check_access_permission(env: &Env, patient_id: &Address, requester: &Address) -> bool {
    // Patient always has access to their own data
    if patient_id == requester {
        return true;
    }
    
    // Check if admin
    if let Some(admin) = env.storage().instance().get::<DataKey, Address>(&DataKey::Admin) {
        if &admin == requester {
            return true;
        }
    }
    
    // Check explicit access grant
    let key = DataKey::AccessControl(patient_id.clone(), requester.clone());
    env.storage().persistent().has(&key)
}

/// Store cross-sensitivity relationship between allergens
pub fn add_cross_sensitivity(env: &Env, allergen1: &String, allergen2: &String) {
    let key1 = DataKey::CrossSensitivity(allergen1.clone(), allergen2.clone());
    let key2 = DataKey::CrossSensitivity(allergen2.clone(), allergen1.clone());
    
    env.storage().persistent().set(&key1, &true);
    env.storage().persistent().set(&key2, &true);
}

/// Check if two allergens have cross-sensitivity
pub fn has_cross_sensitivity(env: &Env, allergen1: &String, allergen2: &String) -> bool {
    let key = DataKey::CrossSensitivity(allergen1.clone(), allergen2.clone());
    env.storage().persistent().has(&key)
}
