#![no_std]
#![allow(deprecated)]

//! Cross-contract wrapper for cached ZK eligibility checks.

pub mod interface;

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

pub use interface::{
    verify_eligibility_proof, PlaceholderZkProofVerifier, PublicInputs, RUST_INTERFACE_VERSION,
    ZKProofVerifier,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    ZkEligibilityContract,
}

#[contract]
pub struct ZkEligibilityVerifier;

#[contractimpl]
impl ZkEligibilityVerifier {
    pub fn initialize(env: Env, zk_eligibility_contract: Address) {
        env.storage()
            .persistent()
            .set(&DataKey::ZkEligibilityContract, &zk_eligibility_contract);
    }

    pub fn check_eligibility(env: Env, subject: Address) -> bool {
        let zk_contract: Address = env
            .storage()
            .persistent()
            .get(&DataKey::ZkEligibilityContract)
            .expect("not initialized");
        let client = zk_eligibility::ZkEligibilityClient::new(&env, &zk_contract);
        client.is_eligible(&subject)
    }
}
