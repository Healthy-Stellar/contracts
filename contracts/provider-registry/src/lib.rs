#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

mod test;

#[contracttype]
pub enum DataKey {
    Admin,
    Provider(Address),
    Record(String),
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
    pub fn add_record(env: Env, provider: Address, record_id: String, data: String) {
        provider.require_auth();
        if !Self::is_provider(env.clone(), provider.clone()) {
            panic!("Unauthorized: not a whitelisted provider");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id.clone()), &data);
        env.events()
            .publish((symbol_short!("add_rec"), provider, record_id), symbol_short!("ok"));
    }

    /// Retrieve a medical record by ID.
    pub fn get_record(env: Env, record_id: String) -> String {
        env.storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .expect("Record not found")
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
}
