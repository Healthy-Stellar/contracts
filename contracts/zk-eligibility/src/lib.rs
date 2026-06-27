#![no_std]

//! ZK Eligibility Contract
//!
//! Manages verifier key versioning and on-chain proof verification for
//! eligibility-sensitive operations (e.g. telemedicine cross-state licensing,
//! insurance claim gating).
//!
//! ## Design
//! - Admin registers versioned verifier keys (VK). Each VK is bound to a
//!   schema version so proof/public-input formats can evolve without breaking
//!   existing proofs.
//! - Callers submit a (proof, public_inputs, schema_version) tuple.
//!   The contract looks up the active VK for that version and runs
//!   verification.
//! - Verification cost is bounded: public_inputs length is capped at
//!   MAX_PUBLIC_INPUTS and proof length at MAX_PROOF_BYTES.
//! - A successful verification is recorded on-chain (nullifier pattern) so
//!   the same proof cannot be replayed within the TTL window.
//! - Nullifiers expire after `nullifier_ttl_ledgers` ledgers; expired
//!   nullifiers allow re-verification with the same proof.
//! - Integration point: other contracts call `verify_eligibility` and receive
//!   a typed `Ok(())` / `Err(Error)` they can gate their own logic on.

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short, Address, Bytes, BytesN,
    Env, Vec,
};

mod test;

// ── Bounds ────────────────────────────────────────────────────────────────────

/// Maximum number of 32-byte public input scalars accepted per proof.
pub const MAX_PUBLIC_INPUTS: u32 = 16;
/// Maximum proof byte length accepted (Groth16 ~192 bytes; give headroom).
pub const MAX_PROOF_BYTES: u32 = 512;
/// Maximum subjects/bundles accepted in a single batch call.
pub const MAX_BATCH_SIZE: u32 = 10;
/// Default nullifier TTL in ledgers when not explicitly configured (~1 day at 5s/ledger).
pub const DEFAULT_NULLIFIER_TTL_LEDGERS: u32 = 17_280;

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized   = 1,
    NotInitialized       = 2,
    Unauthorized         = 3,
    SchemaNotFound       = 4,
    SchemaAlreadyExists  = 5,
    ProofTooLarge        = 6,
    TooManyPublicInputs  = 7,
    ProofAlreadyUsed     = 8,
    VerificationFailed   = 9,
    BatchTooLarge        = 10,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Initialized,
    Admin,
    /// Verifier key for a given schema version.
    VerifierKey(u32),
    /// Nullifier: proof hash → NullifierRecord (schema version + expiry ledger).
    Nullifier(BytesN<32>),
    /// Cached subject eligibility after a successful proof.
    Eligibility(Address),
    /// Configurable TTL (in ledgers) for nullifier entries.
    NullifierTtlLedgers,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A versioned verifier key entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifierKeyEntry {
    /// Raw verifier key bytes (circuit-specific, opaque to the contract).
    pub vk: Bytes,
    /// Schema version this key is valid for.
    pub schema_version: u32,
    /// Whether this key is still active (admin can deprecate old versions).
    pub active: bool,
}

/// Nullifier record stored on successful proof verification.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NullifierRecord {
    /// Schema version the proof was verified against.
    pub schema_version: u32,
    /// Ledger sequence number at which this nullifier expires.
    pub expires_at_ledger: u32,
}

/// Proof submission bundle.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofBundle {
    /// Raw proof bytes.
    pub proof: Bytes,
    /// Public inputs as a vector of 32-byte scalars.
    pub public_inputs: Vec<BytesN<32>>,
    /// Schema version the proof was generated against.
    pub schema_version: u32,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct ZkEligibility;

#[contractimpl]
impl ZkEligibility {
    /// Initialize with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        Self::assert_not_initialized(&env)?;
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Initialized, &true);
        Ok(())
    }

    /// Set the nullifier TTL in ledgers. Admin only.
    pub fn set_nullifier_ttl(env: Env, admin: Address, ttl_ledgers: u32) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        Self::assert_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::NullifierTtlLedgers, &ttl_ledgers);
        Ok(())
    }

    /// Register a verifier key for a new schema version. Admin only.
    /// Each schema_version may only be registered once; rotate by deprecating
    /// the old version and registering a new one.
    pub fn register_verifier_key(
        env: Env,
        admin: Address,
        schema_version: u32,
        vk: Bytes,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        Self::assert_admin(&env, &admin)?;

        let key = DataKey::VerifierKey(schema_version);
        if env.storage().persistent().has(&key) {
            return Err(Error::SchemaAlreadyExists);
        }

        let entry = VerifierKeyEntry {
            vk,
            schema_version,
            active: true,
        };
        env.storage().persistent().set(&key, &entry);
        env.events()
            .publish((symbol_short!("vk_reg"), schema_version), symbol_short!("ok"));
        Ok(())
    }

    /// Deprecate a verifier key so no new proofs can be verified against it.
    /// Admin only. Existing nullifiers are unaffected.
    pub fn deprecate_verifier_key(
        env: Env,
        admin: Address,
        schema_version: u32,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        Self::assert_admin(&env, &admin)?;

        let key = DataKey::VerifierKey(schema_version);
        let mut entry: VerifierKeyEntry = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::SchemaNotFound)?;

        entry.active = false;
        env.storage().persistent().set(&key, &entry);
        env.events()
            .publish((symbol_short!("vk_dep"), schema_version), symbol_short!("ok"));
        Ok(())
    }

    /// Verify a ZK proof of eligibility.
    ///
    /// On success the proof nullifier is stored so the proof cannot be
    /// replayed within the TTL window. Returns `Ok(())` which callers use
    /// to gate their own logic.
    ///
    /// `subject` is the address whose eligibility is being proven; it must
    /// sign the call so the proof cannot be submitted on behalf of another
    /// party without their consent.
    pub fn verify_eligibility(
        env: Env,
        subject: Address,
        bundle: ProofBundle,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        subject.require_auth();

        // ── Bound checks ──────────────────────────────────────────────────────
        if bundle.proof.len() > MAX_PROOF_BYTES {
            return Err(Error::ProofTooLarge);
        }
        if bundle.public_inputs.len() > MAX_PUBLIC_INPUTS {
            return Err(Error::TooManyPublicInputs);
        }

        // ── Verifier key lookup ───────────────────────────────────────────────
        let vk_entry: VerifierKeyEntry = env
            .storage()
            .persistent()
            .get(&DataKey::VerifierKey(bundle.schema_version))
            .ok_or(Error::SchemaNotFound)?;

        if !vk_entry.active {
            return Err(Error::SchemaNotFound);
        }

        // ── Nullifier check ───────────────────────────────────────────────────
        let proof_hash: BytesN<32> = env.crypto().sha256(&bundle.proof).into();
        if Self::nullifier_active(&env, &proof_hash) {
            return Err(Error::ProofAlreadyUsed);
        }

        // ── Verification ──────────────────────────────────────────────────────
        if !Self::run_verification(&env, &vk_entry.vk, &bundle.proof, &bundle.public_inputs) {
            return Err(Error::VerificationFailed);
        }

        // ── Record nullifier ──────────────────────────────────────────────────
        Self::store_nullifier(&env, &proof_hash, bundle.schema_version);
        env.storage()
            .persistent()
            .set(&DataKey::Eligibility(subject.clone()), &true);

        env.events().publish(
            (symbol_short!("zk_ok"), subject, bundle.schema_version),
            proof_hash,
        );
        Ok(())
    }

    /// Verify eligibility for a batch of up to `MAX_BATCH_SIZE` subjects.
    ///
    /// Returns a `Vec<bool>` of the same length as the inputs. A failure at
    /// index N (invalid proof, expired nullifier, unknown schema, etc.) sets
    /// that entry to `false` and does not affect other indices.
    /// Batch sizes exceeding `MAX_BATCH_SIZE` return `Error::BatchTooLarge`.
    pub fn verify_eligibility_batch(
        env: Env,
        subjects: Vec<Address>,
        bundles: Vec<ProofBundle>,
    ) -> Result<Vec<bool>, Error> {
        Self::assert_initialized(&env)?;

        let len = subjects.len();
        if len > MAX_BATCH_SIZE || bundles.len() > MAX_BATCH_SIZE || len != bundles.len() {
            return Err(Error::BatchTooLarge);
        }

        let mut results: Vec<bool> = Vec::new(&env);
        for i in 0..len {
            let subject = subjects.get(i).unwrap();
            let bundle = bundles.get(i).unwrap();
            subject.require_auth();
            let ok = Self::try_verify_single(&env, &subject, &bundle);
            results.push_back(ok);
        }
        Ok(results)
    }

    /// Read a verifier key entry (public view).
    pub fn get_verifier_key(env: Env, schema_version: u32) -> Result<VerifierKeyEntry, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::VerifierKey(schema_version))
            .ok_or(Error::SchemaNotFound)
    }

    /// Check whether a proof (identified by its hash) has an active, unexpired nullifier.
    pub fn is_nullified(env: Env, proof_hash: BytesN<32>) -> bool {
        Self::nullifier_active(&env, &proof_hash)
    }

    /// Check whether a subject has a cached successful eligibility proof.
    pub fn is_eligible(env: Env, subject: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Eligibility(subject))
            .unwrap_or(false)
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    fn nullifier_active(env: &Env, proof_hash: &BytesN<32>) -> bool {
        let record: NullifierRecord = match env
            .storage()
            .persistent()
            .get(&DataKey::Nullifier(proof_hash.clone()))
        {
            Some(r) => r,
            None => return false,
        };
        env.ledger().sequence() < record.expires_at_ledger
    }

    fn store_nullifier(env: &Env, proof_hash: &BytesN<32>, schema_version: u32) {
        let ttl: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::NullifierTtlLedgers)
            .unwrap_or(DEFAULT_NULLIFIER_TTL_LEDGERS);
        let expires_at = env.ledger().sequence().saturating_add(ttl);
        env.storage().persistent().set(
            &DataKey::Nullifier(proof_hash.clone()),
            &NullifierRecord { schema_version, expires_at_ledger: expires_at },
        );
    }

    /// Inner verification logic for a single (subject, bundle) pair that
    /// returns `bool` instead of `Result` so batch calls can collect partial
    /// successes without aborting the entire transaction.
    fn try_verify_single(env: &Env, subject: &Address, bundle: &ProofBundle) -> bool {
        if bundle.proof.len() > MAX_PROOF_BYTES || bundle.public_inputs.len() > MAX_PUBLIC_INPUTS {
            return false;
        }

        let vk_entry: VerifierKeyEntry = match env
            .storage()
            .persistent()
            .get(&DataKey::VerifierKey(bundle.schema_version))
        {
            Some(e) => e,
            None => return false,
        };
        if !vk_entry.active {
            return false;
        }

        let proof_hash: BytesN<32> = env.crypto().sha256(&bundle.proof).into();
        if Self::nullifier_active(env, &proof_hash) {
            return false;
        }

        if !Self::run_verification(env, &vk_entry.vk, &bundle.proof, &bundle.public_inputs) {
            return false;
        }

        Self::store_nullifier(env, &proof_hash, bundle.schema_version);
        env.storage()
            .persistent()
            .set(&DataKey::Eligibility(subject.clone()), &true);

        env.events().publish(
            (symbol_short!("zk_ok"), subject.clone(), bundle.schema_version),
            proof_hash,
        );
        true
    }

    // ── guards ────────────────────────────────────────────────────────────────

    fn assert_initialized(env: &Env) -> Result<(), Error> {
        if !env.storage().persistent().has(&DataKey::Initialized) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn assert_not_initialized(env: &Env) -> Result<(), Error> {
        if env.storage().persistent().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }
        Ok(())
    }

    fn assert_admin(env: &Env, caller: &Address) -> Result<(), Error> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if *caller != admin {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    /// Cryptographic verification stub.
    ///
    /// Production: replace the body with a call to the host's pairing
    /// verifier or a cross-contract call to a deployed verifier contract.
    /// The function signature is stable so all callers are unaffected.
    ///
    /// The stub accepts any proof whose first byte equals the first byte of
    /// the verifier key — a deterministic, testable rule that exercises the
    /// full call path without requiring real ZK machinery.
    fn run_verification(
        _env: &Env,
        vk: &Bytes,
        proof: &Bytes,
        _public_inputs: &Vec<BytesN<32>>,
    ) -> bool {
        if vk.is_empty() || proof.is_empty() {
            return false;
        }
        vk.get(0) == proof.get(0)
    }
}
