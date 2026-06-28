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
//!   the same proof cannot be replayed.
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
/// Admin rotation confirmation window (24 hours in seconds).
pub const ADMIN_ROTATION_WINDOW: u64 = 86_400;
/// Index of the expiry timestamp within `public_inputs` (big-endian u64 in first 8 bytes).
pub const EXPIRY_INPUT_IDX: u32 = 0;

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
    ProofExpired         = 10,
    RotationPending      = 11,
    NoRotationPending    = 12,
    RotationExpired      = 13,
    NotPendingAdmin      = 14,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Initialized,
    Admin,
    /// Verifier key for a given schema version.
    VerifierKey(u32),
    /// Nullifier: proof hash → bool (prevents replay).
    Nullifier(BytesN<32>),
    /// Cached subject eligibility after a successful proof.
    Eligibility(Address),
    PendingAdmin,
    RotationExpiry,
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
    /// replayed. Returns `Ok(())` which callers use to gate their own logic.
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

        // ── Expiry check (public_inputs[0] = big-endian u64 expiry timestamp) ─
        let expiry_input = bundle
            .public_inputs
            .get(EXPIRY_INPUT_IDX)
            .ok_or(Error::ProofExpired)?;
        let expiry = Self::decode_expiry_u64(&expiry_input);
        if expiry <= env.ledger().timestamp() {
            return Err(Error::ProofExpired);
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
        let nullifier_key = DataKey::Nullifier(proof_hash.clone());
        if env.storage().persistent().has(&nullifier_key) {
            return Err(Error::ProofAlreadyUsed);
        }

        // ── Verification ──────────────────────────────────────────────────────
        // On Soroban there is no native pairing-based ZK verifier built into
        // the host. The canonical production approach is to use a Soroban host
        // function once it is available, or to call an external verifier
        // contract whose address is stored in the VK entry.
        //
        // Here we implement the verification gate that all production code
        // paths must pass through. The actual cryptographic check is delegated
        // to `run_verification` which can be swapped for a real verifier
        // without changing any caller code.
        if !Self::run_verification(&env, &vk_entry.vk, &bundle.proof, &bundle.public_inputs) {
            return Err(Error::VerificationFailed);
        }

        // ── Record nullifier ──────────────────────────────────────────────────
        env.storage().persistent().set(&nullifier_key, &true);
        env.storage()
            .persistent()
            .set(&DataKey::Eligibility(subject.clone()), &true);

        env.events().publish(
            (symbol_short!("zk_ok"), subject, bundle.schema_version),
            proof_hash,
        );
        Ok(())
    }

    /// Read a verifier key entry (public view).
    pub fn get_verifier_key(env: Env, schema_version: u32) -> Result<VerifierKeyEntry, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::VerifierKey(schema_version))
            .ok_or(Error::SchemaNotFound)
    }

    /// Check whether a proof (identified by its hash) has already been used.
    pub fn is_nullified(env: Env, proof_hash: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Nullifier(proof_hash))
    }

    /// Check whether a subject has a cached successful eligibility proof.
    pub fn is_eligible(env: Env, subject: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Eligibility(subject))
            .unwrap_or(false)
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

    /// Propose transferring admin to `new_admin`. Must be confirmed within 24 hours.
    pub fn propose_admin_rotation(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        Self::assert_admin(&env, &admin)?;
        if env.storage().persistent().has(&DataKey::PendingAdmin) {
            return Err(Error::RotationPending);
        }
        let expiry = env.ledger().timestamp() + ADMIN_ROTATION_WINDOW;
        env.storage().persistent().set(&DataKey::PendingAdmin, &new_admin);
        env.storage().persistent().set(&DataKey::RotationExpiry, &expiry);
        Ok(())
    }

    /// New admin confirms the rotation proposed by the current admin.
    pub fn accept_admin_rotation(env: Env, new_admin: Address) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        new_admin.require_auth();
        let pending: Address = env
            .storage()
            .persistent()
            .get(&DataKey::PendingAdmin)
            .ok_or(Error::NoRotationPending)?;
        if new_admin != pending {
            return Err(Error::NotPendingAdmin);
        }
        let expiry: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RotationExpiry)
            .unwrap_or(0);
        if env.ledger().timestamp() > expiry {
            env.storage().persistent().remove(&DataKey::PendingAdmin);
            env.storage().persistent().remove(&DataKey::RotationExpiry);
            return Err(Error::RotationExpired);
        }
        env.storage().persistent().set(&DataKey::Admin, &new_admin);
        env.storage().persistent().remove(&DataKey::PendingAdmin);
        env.storage().persistent().remove(&DataKey::RotationExpiry);
        Ok(())
    }

    /// Decode a big-endian u64 from the first 8 bytes of a public input scalar.
    fn decode_expiry_u64(input: &BytesN<32>) -> u64 {
        let mut ts: u64 = 0;
        for i in 0..8u32 {
            ts = (ts << 8) | (input.get(i).unwrap_or(0) as u64);
        }
        ts
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
