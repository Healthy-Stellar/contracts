#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
};

mod test;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    RateLimitExceeded = 1,
    InvalidScore = 2,
    AlreadyRated = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitConfig {
    pub max_records: u32,
    pub window_seconds: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRateWindow {
    pub count: u32,
    pub window_start: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderReputation {
    pub total_ratings: u64,
    pub total_score: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Provider(Address),
    Record(String),
    RateLimitConfig,
    ProviderRate(Address),
    ProviderReputation(Address),
    ProviderRatingByPatient(Address, Address), // (provider, patient)
}

#[contract]
pub struct ProviderRegistry;

#[contractimpl]
impl ProviderRegistry {
    /// Initialize the contract with an admin address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    /// Configure rolling per-provider rate limit for `add_record`. Admin only.
    /// Use `max_records = 0` or `window_seconds = 0` to disable limiting.
    pub fn set_rate_limit(env: Env, admin: Address, max_records: u32, window_seconds: u64) {
        Self::assert_admin(&env, &admin);
        env.storage().instance().set(
            &DataKey::RateLimitConfig,
            &RateLimitConfig {
                max_records,
                window_seconds,
            },
        );
    }

    /// Whitelist a provider address. Admin only.
    pub fn register_provider(env: Env, admin: Address, provider: Address) {
        Self::assert_admin(&env, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::Provider(provider.clone()), &true);
        env.events()
            .publish((symbol_short!("reg_prov"), provider), symbol_short!("ok"));
    }

    /// Remove a provider from the whitelist. Admin only.
    pub fn revoke_provider(env: Env, admin: Address, provider: Address) {
        Self::assert_admin(&env, &admin);
        env.storage()
            .persistent()
            .remove(&DataKey::Provider(provider.clone()));
        env.events()
            .publish((symbol_short!("rev_prov"), provider), symbol_short!("ok"));
    }

    /// Returns true if the address is a whitelisted provider.
    pub fn is_provider(env: Env, provider: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Provider(provider))
            .unwrap_or(false)
    }

    /// Add a medical record. Caller must be a whitelisted provider.
    pub fn add_record(
        env: Env,
        provider: Address,
        record_id: String,
        data: String,
    ) -> Result<(), ContractError> {
        provider.require_auth();
        if !Self::is_provider(env.clone(), provider.clone()) {
            panic!("Unauthorized: not a whitelisted provider");
        }
        Self::consume_provider_rate_slot(&env, &provider)?;
        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id.clone()), &data);
        env.events().publish(
            (symbol_short!("add_rec"), provider, record_id),
            symbol_short!("ok"),
        );
        Ok(())
    }

    /// Retrieve a medical record by ID.
    pub fn get_record(env: Env, record_id: String) -> String {
        env.storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .expect("Record not found")
    }

    /// Rate a provider with score 1..=5.
    /// A patient can only rate the same provider once.
    pub fn rate_provider(
        env: Env,
        patient: Address,
        provider: Address,
        score: u32,
    ) -> Result<(), ContractError> {
        patient.require_auth();

        if score < 1 || score > 5 {
            return Err(ContractError::InvalidScore);
        }
        if !Self::is_provider(env.clone(), provider.clone()) {
            panic!("Provider not found");
        }

        let patient_rating_key = DataKey::ProviderRatingByPatient(provider.clone(), patient);
        if env.storage().persistent().has(&patient_rating_key) {
            return Err(ContractError::AlreadyRated);
        }

        let reputation_key = DataKey::ProviderReputation(provider.clone());
        let mut reputation: ProviderReputation = env
            .storage()
            .persistent()
            .get(&reputation_key)
            .unwrap_or(ProviderReputation {
                total_ratings: 0,
                total_score: 0,
            });

        reputation.total_ratings += 1;
        reputation.total_score += score as u64;

        env.storage()
            .persistent()
            .set(&patient_rating_key, &true);
        env.storage().persistent().set(&reputation_key, &reputation);
        env.events().publish(
            (symbol_short!("rate"), provider),
            (reputation.total_ratings, score),
        );
        Ok(())
    }

    /// Returns (total_ratings, average_score_scaled_by_100).
    pub fn get_provider_reputation(env: Env, provider: Address) -> (u64, u64) {
        let reputation_key = DataKey::ProviderReputation(provider);
        let reputation: ProviderReputation = env
            .storage()
            .persistent()
            .get(&reputation_key)
            .unwrap_or(ProviderReputation {
                total_ratings: 0,
                total_score: 0,
            });

        if reputation.total_ratings == 0 {
            return (0, 0);
        }
        let average_scaled = (reputation.total_score * 100) / reputation.total_ratings;
        (reputation.total_ratings, average_scaled)
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    fn assert_admin(env: &Env, caller: &Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        if *caller != admin {
            panic!("Unauthorized: admin only");
        }
    }

    /// Per-provider counter with window start; resets when ledger time passes the window.
    fn consume_provider_rate_slot(env: &Env, provider: &Address) -> Result<(), ContractError> {
        let config_opt: Option<RateLimitConfig> =
            env.storage().instance().get(&DataKey::RateLimitConfig);
        let Some(config) = config_opt else {
            return Ok(());
        };
        if config.max_records == 0 || config.window_seconds == 0 {
            return Ok(());
        }

        let now = env.ledger().timestamp();
        let key = DataKey::ProviderRate(provider.clone());
        let mut state: ProviderRateWindow =
            env.storage()
                .persistent()
                .get(&key)
                .unwrap_or(ProviderRateWindow {
                    count: 0,
                    window_start: 0,
                });

        let window_end = state.window_start.saturating_add(config.window_seconds);
        if state.window_start == 0 || now >= window_end {
            state.count = 0;
            state.window_start = now;
        }

        if state.count >= config.max_records {
            return Err(ContractError::RateLimitExceeded);
        }

        state.count += 1;
        env.storage().persistent().set(&key, &state);
        Ok(())
    }
}
