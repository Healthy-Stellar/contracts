#![no_std]

//! # ZK Eligibility Verifier Contract
//!
//! Cross-contract wrapper for zero-knowledge proof eligibility verification.
//! Acts as a pass-through to `zk-eligibility` contract for eligibility checks.
//!
//! This wrapper allows other contracts to uniformly delegate eligibility verification
//! and can be extended with caching/TTL logic in future versions (see issue #818).
//! Currently, verification results are cached by the underlying `zk-eligibility`
//! contract, which stores them with TTL enforcement (see `zk_eligibility::is_eligible`).

pub mod interface;

use soroban_sdk::{contract, contractclient, contracterror, contractimpl, contracttype, Address, Env};

pub use interface::{
    verify_eligibility_proof, PlaceholderZkProofVerifier, PublicInputs, RUST_INTERFACE_VERSION,
    ZKProofVerifier,
};

/// Number of public inputs the eligibility circuit expects, in order:
/// `[subject_hash, nullifier, contract_address_hash, network_passphrase_hash]`.
pub const PUBLIC_INPUT_COUNT: u32 = 4;

/// Minimal client for the `zk-eligibility` contract.
///
/// Declared locally (instead of depending on the full `zk-eligibility` contract
/// crate) so the verifier only references the single method it needs. Depending
/// on the contract crate caused both crates' `#[contractimpl]` blocks to emit an
/// `initialize` export symbol, which clashed when linking the WASM and left the
/// verifier's own `initialize` unexported while leaking zk-eligibility's full
/// API onto the verifier's storage (#990).
#[contractclient(name = "ZkEligibilityClient")]
pub trait ZkEligibilityInterface {
    fn is_eligible(env: Env, subject: Address) -> bool;

    /// Submit a proof bound to `subject` and a circuit-level `nullifier`.
    ///
    /// The underlying contract must:
    /// - require `subject.require_auth()`,
    /// - check `public_inputs[0] == H(subject)`,
    /// - check `public_inputs[1] == nullifier` and reject a reused nullifier,
    /// - check the domain-binding inputs against its own address / network.
    fn verify_eligibility(
        env: Env,
        subject: Address,
        public_inputs: PublicInputs,
        nullifier: soroban_sdk::BytesN<32>,
        proof: soroban_sdk::Bytes,
    ) -> bool;
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    ZkEligibilityContract,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 2,
    Unauthorized = 3,
}

#[contract]
pub struct ZkEligibilityVerifier;

#[contractimpl]
impl ZkEligibilityVerifier {
    /// Runs atomically as part of contract creation, so the deployer binds the
    /// admin and verifier addresses and no front-running `initialize` call is possible.
    pub fn __constructor(
        env: Env,
        admin: Address,
        zk_eligibility_contract: Address,
    ) {
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::ZkEligibilityContract, &zk_eligibility_contract);
    }

    /// Submit a proof for `subject`, forwarding the circuit public inputs
    /// (subject hash, nullifier, domain binding) to the zk-eligibility contract.
    ///
    /// The caller must supply the full `public_inputs` bundle so the underlying
    /// contract can verify the subject binding and the circuit-level nullifier.
    /// Returns `true` only if the underlying contract accepts the proof.
    pub fn verify_eligibility(
        env: Env,
        subject: Address,
        public_inputs: PublicInputs,
        nullifier: soroban_sdk::BytesN<32>,
        proof: soroban_sdk::Bytes,
    ) -> bool {
        let zk_contract: Address = match env
            .storage()
            .persistent()
            .get(&DataKey::ZkEligibilityContract)
        {
            Some(addr) => addr,
            None => return false, // Not initialized; cannot verify
        };
        // Fail closed: any error from the target (missing, archived, trapping,
        // interface-incompatible, or proof rejected) answers "not verified".
        let client = ZkEligibilityClient::new(&env, &zk_contract);
        match client.try_verify_eligibility(&subject, &public_inputs, &nullifier, &proof) {
            Ok(Ok(verified)) => verified,
            _ => false,
        }
    }

    /// Check eligibility by delegating to the configured zk-eligibility contract.
    pub fn check_eligibility(env: Env, subject: Address) -> bool {
        let zk_contract: Address = match env
            .storage()
            .persistent()
            .get(&DataKey::ZkEligibilityContract)
        {
            Some(addr) => addr,
            None => return false, // Not initialized; cannot verify
        };
        // Fail closed: a missing, archived, trapping or interface-incompatible
        // target must answer "not eligible" instead of aborting the caller (#951).
        let client = ZkEligibilityClient::new(&env, &zk_contract);
        match client.try_is_eligible(&subject) {
            Ok(Ok(eligible)) => eligible,
            _ => false,
        }
    }

    /// Update the zk-eligibility contract address. Admin only.
    /// Used when the underlying verifier contract is redeployed.
    pub fn set_zk_eligibility_contract(
        env: Env,
        admin: Address,
        zk_eligibility_contract: Address,
    ) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        env.storage()
            .persistent()
            .set(&DataKey::ZkEligibilityContract, &zk_eligibility_contract);
        Ok(())
    }
}

#[cfg(test)]
mod test;
