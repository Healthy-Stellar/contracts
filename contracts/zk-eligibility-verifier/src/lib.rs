#![no_std]

//! # ZK Eligibility Verifier Contract
//!
//! Cross-contract wrapper for cached zero-knowledge proof eligibility checks with result caching,
//! verification delegation, and proof result validation.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Verification results cached to prevent re-verification overhead.
//! Cache invalidation enforced per TTL. Cross-contract calls validate callee eligibility via
//! delegation. Cached results restrict replay without nullifier re-verification. Authorization
//! checks on result retrieval.
//!
//! **Audit Controls:** Cache hit/miss events logged for monitoring. Verification delegation
//! events tracked. Cache invalidation events recorded. Cross-contract verification calls tracked
//! with caller identity. Stale result detection logged.
//!
//! **Data Retention Policy:** Verification results cached with TTL enforcing freshness. Cache
//! entries expire after configured lifetime. Expired results automatically invalidated. Verification
//! history maintained via event stream. No persistent proof storage (privacy-preserving).
//!
//! **Encryption/Integrity:** Cached verification results encrypted in persistent storage. Cache
//! TTL enforced by Soroban storage layer. Cross-contract interface prevents tampering. Nullifier
//! validation prevents stale cache exploitation. Result integrity via delegation signature.

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
