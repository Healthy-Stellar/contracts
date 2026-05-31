#![no_std]

use soroban_sdk::{contracttype, vec, Address, Env, IntoVal, Symbol};

/// Actor types for verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActorType {
    Patient,
    Provider,
    Hospital,
    Insurer,
}

/// Cached verification result with expiration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationCache {
    pub verified: bool,
    pub expires_at: u64,
}

/// Storage key for verification cache
#[contracttype]
pub enum VerificationKey {
    Cache(ActorType, Address),
    /// Registry contract addresses stored by the host contract during initialization.
    PatientRegistry,
    ProviderRegistry,
    HospitalRegistry,
    InsurerRegistry,
}

/// Cache duration in ledger seconds (e.g., 24 hours)
pub const CACHE_DURATION: u64 = 86400;

/// Verify if an address is a registered actor of the given type.
/// Uses caching to reduce cross-contract calls.
/// Registry addresses must be stored in instance storage under the
/// `VerificationKey::{Patient,Provider,Hospital,Insurer}Registry` keys
/// before this function is called.
pub fn verify_actor(env: &Env, actor_type: ActorType, address: &Address) -> bool {
    // Check cache first
    let cache_key = VerificationKey::Cache(actor_type.clone(), address.clone());
    if let Some(cache) = env
        .storage()
        .temporary()
        .get::<VerificationKey, VerificationCache>(&cache_key)
    {
        if env.ledger().timestamp() < cache.expires_at {
            return cache.verified;
        }
    }

    // Perform verification based on actor type
    let verified = match actor_type {
        ActorType::Patient => verify_patient(env, address),
        ActorType::Provider => verify_provider(env, address),
        ActorType::Hospital => verify_hospital(env, address),
        ActorType::Insurer => verify_insurer(env, address),
    };

    // Cache the result
    let cache = VerificationCache {
        verified,
        expires_at: env.ledger().timestamp() + CACHE_DURATION,
    };
    env.storage().temporary().set(&cache_key, &cache);

    verified
}

fn verify_patient(env: &Env, address: &Address) -> bool {
    let registry: Address = match env
        .storage()
        .instance()
        .get::<VerificationKey, Address>(&VerificationKey::PatientRegistry)
    {
        Some(r) => r,
        None => return false,
    };
    let args = vec![env, address.clone().into_val(env)];
    env.invoke_contract(&registry, &Symbol::new(env, "is_patient_registered"), args)
}

fn verify_provider(env: &Env, address: &Address) -> bool {
    let registry: Address = match env
        .storage()
        .instance()
        .get::<VerificationKey, Address>(&VerificationKey::ProviderRegistry)
    {
        Some(r) => r,
        None => return false,
    };
    let args = vec![env, address.clone().into_val(env)];
    env.invoke_contract(&registry, &Symbol::new(env, "is_provider"), args)
}

fn verify_hospital(env: &Env, address: &Address) -> bool {
    let registry: Address = match env
        .storage()
        .instance()
        .get::<VerificationKey, Address>(&VerificationKey::HospitalRegistry)
    {
        Some(r) => r,
        None => return false,
    };
    let args = vec![env, address.clone().into_val(env)];
    env.invoke_contract(&registry, &Symbol::new(env, "is_hospital_active"), args)
}

fn verify_insurer(env: &Env, address: &Address) -> bool {
    let registry: Address = match env
        .storage()
        .instance()
        .get::<VerificationKey, Address>(&VerificationKey::InsurerRegistry)
    {
        Some(r) => r,
        None => return false,
    };
    let args = vec![env, address.clone().into_val(env)];
    env.invoke_contract(&registry, &Symbol::new(env, "is_insurer_active"), args)
}
