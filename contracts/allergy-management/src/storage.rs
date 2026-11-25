use soroban_sdk::{Address, Env, String, Symbol, Vec};
use ttl_config::{extend_critical_ttl_if_exists, extend_critical_ttl};

use crate::{AllergyRecord, DataKey, EncryptedEnvelopeRef, Error};

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
    extend_critical_ttl(env, &key);
}

/// Get an allergy record by ID
pub fn get_allergy(env: &Env, allergy_id: u64) -> Result<AllergyRecord, Error> {
    let key = DataKey::Allergy(allergy_id);
    let result = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(Error::AllergyNotFound);
    
    if result.is_ok() {
        extend_critical_ttl_if_exists(env, &key);
    }
    
    result
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
    extend_critical_ttl(env, &key);
}

/// Get all allergy IDs for a patient
pub fn get_patient_allergies(env: &Env, patient_id: &Address) -> Vec<u64> {
    let key = DataKey::PatientAllergies(patient_id.clone());
    extend_critical_ttl_if_exists(env, &key);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

/// Remove all allergy records and the patient's allergy index (used during deregistration)
pub fn remove_patient_allergies(env: &Env, patient_id: &Address) {
    let key = DataKey::PatientAllergies(patient_id.clone());
    let allergy_ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));

    for allergy_id in allergy_ids.iter() {
        env.storage()
            .persistent()
            .remove(&DataKey::Allergy(allergy_id));
    }

    env.storage().persistent().remove(&key);
}

/// Check if a patient already has a specific allergy recorded.
///
/// Because the allergen is stored as an encrypted envelope, duplicate detection
/// compares the envelope reference (which is deterministic for the same
/// plaintext under the same key) rather than the plaintext allergen name.
pub fn check_duplicate_allergy(
    env: &Env,
    patient_id: &Address,
    allergen: &EncryptedEnvelopeRef,
    allergen_type: &Symbol,
) -> bool {
    let allergy_ids = get_patient_allergies(env, patient_id);

    for allergy_id in allergy_ids.iter() {
        if let Ok(allergy) = get_allergy(env, allergy_id) {
            if allergy.allergen == *allergen
                && allergy.allergen_type == *allergen_type
                && allergy.status.is_active()
            {
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
    extend_critical_ttl(env, &key);

    // Maintain per-patient provider index
    let index_key = DataKey::GrantedProviders(patient_id.clone());
    let mut providers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&index_key)
        .unwrap_or(Vec::new(env));
    let mut already = false;
    for p in providers.iter() {
        if p == *provider_id {
            already = true;
            break;
        }
    }
    if !already {
        providers.push_back(provider_id.clone());
        env.storage().persistent().set(&index_key, &providers);
        extend_critical_ttl(env, &index_key);
    }
}

/// Revoke access from a provider
pub fn revoke_access(env: &Env, patient_id: &Address, provider_id: &Address) {
    let key = DataKey::AccessControl(patient_id.clone(), provider_id.clone());
    env.storage().persistent().remove(&key);

    // Remove from per-patient provider index
    let index_key = DataKey::GrantedProviders(patient_id.clone());
    let providers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&index_key)
        .unwrap_or(Vec::new(env));
    let mut updated: Vec<Address> = Vec::new(env);
    for p in providers.iter() {
        if p != *provider_id {
            updated.push_back(p);
        }
    }
    env.storage().persistent().set(&index_key, &updated);
}

/// Remove all AccessControl grants for a patient (used during deregistration)
pub fn remove_all_access_grants(env: &Env, patient_id: &Address) {
    let index_key = DataKey::GrantedProviders(patient_id.clone());
    let providers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&index_key)
        .unwrap_or(Vec::new(env));
    for provider_id in providers.iter() {
        let key = DataKey::AccessControl(patient_id.clone(), provider_id.clone());
        env.storage().persistent().remove(&key);
    }
    env.storage().persistent().remove(&index_key);
}

/// Alias for `remove_all_access_grants` kept for the deregistration path.
pub fn clear_access_grants(env: &Env, patient_id: &Address) {
    remove_all_access_grants(env, patient_id);
}

/// Check if a requester has access to patient allergies
pub fn check_access_permission(env: &Env, patient_id: &Address, requester: &Address) -> bool {
    // Patient always has access to their own data
    if patient_id == requester {
        return true;
    }

    // Check if admin
    if let Some(admin) = env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
    {
        if &admin == requester {
            return true;
        }
    }

    // Check explicit access grant
    let key = DataKey::AccessControl(patient_id.clone(), requester.clone());
    env.storage().persistent().has(&key)
}

/// Check whether a provider is authorized to mutate a patient's allergy record.
///
/// A provider may mutate an allergy only if they are the patient themselves,
/// the admin, a provider the patient explicitly granted access to, or the
/// provider who originally recorded the allergy. This mirrors the read-path
/// access model enforced via `check_access_permission`.
pub fn check_allergy_mutation_permission(
    env: &Env,
    allergy: &AllergyRecord,
    provider_id: &Address,
) -> bool {
    if &allergy.provider_id == provider_id {
        return true;
    }

    check_access_permission(env, &allergy.patient_id, provider_id)
}

/// Store cross-sensitivity relationship between allergens
pub fn add_cross_sensitivity(env: &Env, allergen1: &String, allergen2: &String) {
    let key1 = DataKey::CrossSensitivity(allergen1.clone(), allergen2.clone());
    let key2 = DataKey::CrossSensitivity(allergen2.clone(), allergen1.clone());

    env.storage().persistent().set(&key1, &true);
    extend_critical_ttl(env, &key1);
    env.storage().persistent().set(&key2, &true);
    extend_critical_ttl(env, &key2);
}

/// Check if two allergens have cross-sensitivity
pub fn has_cross_sensitivity(env: &Env, allergen1: &String, allergen2: &String) -> bool {
    let key = DataKey::CrossSensitivity(allergen1.clone(), allergen2.clone());
    env.storage().persistent().has(&key)
}
