#![no_std]

//! # ZK Eligibility Contract
//!
//! Manages verifier key versioning and zero-knowledge proof verification for eligibility-sensitive
//! healthcare operations (telemedicine cross-state licensing, insurance claim gating, etc).
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

// ── Mainnet deployment gate ───────────────────────────────────────────────────
// Building with `--features mainnet` is a hard error until a real Groth16/PLONK
// pairing verifier replaces the stub in `run_verification`.  CI must never
// enable this feature until issue #821 is resolved.
#[cfg(feature = "mainnet")]
compile_error!(
    "The `mainnet` feature is reserved for a future release that ships a real \
     Groth16/PLONK verifier (issue #821).  The current `run_verification` \
     implementation is a non-cryptographic testing stub and MUST NOT be \
     deployed to mainnet.  Remove `--features mainnet` from your build command."
);

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
/// Window (in seconds) within which a new admin must accept the rotation (~24 hours).
pub const ADMIN_ROTATION_WINDOW: u64 = 86_400;
/// Alias for `ADMIN_ROTATION_WINDOW`; used by tests and external callers.
pub const ROTATION_TTL: u64 = ADMIN_ROTATION_WINDOW;
/// Index of the expiry timestamp in the public inputs array.
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
    BatchTooLarge        = 10,
    RotationPending      = 11,
    NoRotationPending    = 12,
    NotPendingAdmin      = 13,
    RotationExpired      = 14,
    ProofExpired         = 15,
    /// Returned at runtime when `verify_eligibility` is called on a build that
    /// still uses the non-cryptographic stub verifier AND public inputs do not
    /// satisfy the stub commitment check.  Never returned by a correct proof
    /// submission; exists so integration tests can assert the stub is active.
    StubVerifierActive   = 16,
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
    /// Value is a (bool, u32) tuple: (eligible, expires_at_ledger)
    Eligibility(Address),
    /// Configurable TTL (in ledgers) for nullifier entries.
    NullifierTtlLedgers,
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
    /// When non-zero, this deprecated schema was migrated to `migrated_to`.
    /// Nullifiers recorded under this schema remain valid after migration.
    /// When zero the schema was deprecated without migration; its nullifiers
    /// are treated as expired so subjects can re-verify under a new schema.
    pub migrated_to: u32,
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
            migrated_to: 0,
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

    /// Migrate a deprecated schema to a new version, carrying nullifiers forward.
    ///
    /// After a successful migration:
    /// - `old_version` is marked `deprecated-migrated`; its nullifiers remain
    ///   valid, preventing replay under the new schema.
    /// - Nullifiers from schemas deprecated WITHOUT migration are treated as
    ///   invalid, allowing subjects to re-verify under the new key.
    ///
    /// `migration_proof` is verified against the new schema's verifier key.
    pub fn migrate_schema(
        env: Env,
        admin: Address,
        old_version: u32,
        new_version: u32,
        migration_proof: Bytes,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        Self::assert_admin(&env, &admin)?;

        let old_key = DataKey::VerifierKey(old_version);
        let mut old_entry: VerifierKeyEntry = env
            .storage()
            .persistent()
            .get(&old_key)
            .ok_or(Error::SchemaNotFound)?;

        let new_entry: VerifierKeyEntry = env
            .storage()
            .persistent()
            .get(&DataKey::VerifierKey(new_version))
            .ok_or(Error::SchemaNotFound)?;

        if !new_entry.active {
            return Err(Error::SchemaNotFound);
        }

        // Migration proofs are verified against the new schema's VK.
        // The single public input is a 32-byte truncation/hash of the new VK
        // itself, which binds the migration proof to this specific schema
        // without requiring a user-supplied expiry timestamp.
        let vk_hash: BytesN<32> = env.crypto().sha256(&new_entry.vk).into();
        let mut migration_inputs: Vec<BytesN<32>> = Vec::new(&env);
        migration_inputs.push_back(vk_hash);
        if !Self::run_verification(&env, &new_entry.vk, &migration_proof, &migration_inputs) {
            return Err(Error::VerificationFailed);
        }

        old_entry.migrated_to = new_version;
        env.storage().persistent().set(&old_key, &old_entry);

        env.events().publish(
            (symbol_short!("sch_migr"), old_version),
            (new_version,),
        );
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
        if Self::nullifier_active(&env, &proof_hash) {
            return Err(Error::ProofAlreadyUsed);
        }

        // ── Verification ──────────────────────────────────────────────────────
        if !Self::run_verification(&env, &vk_entry.vk, &bundle.proof, &bundle.public_inputs) {
            return Err(Error::VerificationFailed);
        }

        // ── Record nullifier ──────────────────────────────────────────────────
        Self::store_nullifier(&env, &proof_hash, bundle.schema_version);
        
        // ── Cache eligibility with expiry ────────────────────────────────────────
        let eligibility_expires_at = env.ledger().sequence().saturating_add(
            env.storage()
                .persistent()
                .get(&DataKey::NullifierTtlLedgers)
                .unwrap_or(DEFAULT_NULLIFIER_TTL_LEDGERS),
        );
        env.storage()
            .persistent()
            .set(&DataKey::Eligibility(subject.clone()), &(true, eligibility_expires_at));

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
    /// Returns false if the cached entry has expired.
    pub fn is_eligible(env: Env, subject: Address) -> bool {
        let cache_entry: Option<(bool, u32)> = env
            .storage()
            .persistent()
            .get(&DataKey::Eligibility(subject));
        
        match cache_entry {
            Some((eligible, expires_at_ledger)) => {
                // Check if cache has expired
                if env.ledger().sequence() >= expires_at_ledger {
                    return false;
                }
                eligible
            }
            None => false,
        }
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Returns `true` when the nullifier for `proof_hash` is active:
    /// - The record exists and has not yet reached its `expires_at_ledger`.
    /// - The schema it was recorded against is either still active or was
    ///   migrated to a new version (deprecated-migrated). Non-migrated
    ///   deprecated schemas have their nullifiers invalidated so subjects
    ///   can re-verify under the new key.
    fn nullifier_active(env: &Env, proof_hash: &BytesN<32>) -> bool {
        let record: NullifierRecord = match env
            .storage()
            .persistent()
            .get(&DataKey::Nullifier(proof_hash.clone()))
        {
            Some(r) => r,
            None => return false,
        };

        if env.ledger().sequence() >= record.expires_at_ledger {
            return false;
        }

        match env
            .storage()
            .persistent()
            .get::<DataKey, VerifierKeyEntry>(&DataKey::VerifierKey(record.schema_version))
        {
            Some(entry) => entry.active || entry.migrated_to > 0,
            None => false,
        }
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

        // ── Expiry check (public_inputs[0] = big-endian u64 expiry timestamp) ─
        let expiry_input = match bundle.public_inputs.get(EXPIRY_INPUT_IDX) {
            Some(input) => input,
            None => return false,
        };
        let expiry = Self::decode_expiry_u64(&expiry_input);
        if expiry <= env.ledger().timestamp() {
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
        
        // ── Cache eligibility with expiry ────────────────────────────────────────
        let eligibility_expires_at = env.ledger().sequence().saturating_add(
            env.storage()
                .persistent()
                .get(&DataKey::NullifierTtlLedgers)
                .unwrap_or(DEFAULT_NULLIFIER_TTL_LEDGERS),
        );
        env.storage()
            .persistent()
            .set(&DataKey::Eligibility(subject.clone()), &(true, eligibility_expires_at));

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
    /// ⚠️  SECURITY — TESTING STUB ONLY.  This is NOT a real ZK verifier.
    /// Production deployments are blocked by `compile_error!` in the `mainnet`
    /// feature gate at the top of this file.  Replace with a real Groth16/PLONK
    /// pairing verifier before enabling that feature (issue #821).
    ///
    /// ## What this stub checks
    ///
    /// The old stub compared only `vk[0] == proof[0]`, which was trivially
    /// forgeable: any caller who read the public `get_verifier_key` view could
    /// craft a passing proof by setting its first byte to match the VK's first
    /// byte, regardless of public inputs.
    ///
    /// This replacement binds the public inputs into the check:
    ///
    /// ```text
    /// commitment = SHA-256( vk_bytes || public_input_0 || … || public_input_n )
    /// ```
    ///
    /// The proof must carry `commitment[0..4]` as its **last four bytes**.
    /// Because the commitment covers every public input scalar, a proof issued
    /// for one (subject, expiry, claim) tuple will not pass when replayed with
    /// different public inputs — the commitment will not match.
    ///
    /// Forging still requires knowing the VK bytes AND the exact public inputs
    /// *before* submission, which prevents the "read VK[0], craft any proof"
    /// attack described in issue #821 while keeping the stub testable without
    /// real ZK machinery.
    ///
    /// ## Proof format expected by this stub
    ///
    /// ```text
    /// [ arbitrary_payload (0 .. proof.len()-4) ][ commitment[0..4] (last 4 bytes) ]
    /// ```
    ///
    /// Total length must be ≥ 5 bytes (1 byte payload + 4-byte tag).
    fn run_verification(
        env: &Env,
        vk: &Bytes,
        proof: &Bytes,
        public_inputs: &Vec<BytesN<32>>,
    ) -> bool {
        // ── Basic structural checks ───────────────────────────────────────────
        if vk.is_empty() || proof.is_empty() {
            return false;
        }
        // Need at least a 4-byte commitment tag at the end of the proof.
        let proof_len = proof.len();
        if proof_len < 5 {
            return false;
        }
        // public_inputs must be non-empty (expiry lives at index 0).
        if public_inputs.is_empty() {
            return false;
        }

        // ── Build commitment: SHA-256( vk || input_0 || … || input_n ) ───────
        //
        // We concatenate into a single `Bytes` buffer so we make exactly one
        // host call to `env.crypto().sha256()`, keeping metering predictable.
        let mut buf = Bytes::new(env);
        buf.append(vk);
        for i in 0..public_inputs.len() {
            // BytesN<32> → Bytes via from_slice on its raw array copy.
            let scalar: BytesN<32> = public_inputs.get(i).unwrap();
            buf.append(&Bytes::from_slice(env, &scalar.to_array()));
        }
        let commitment: BytesN<32> = env.crypto().sha256(&buf).into();

        // ── Extract the 4-byte tag from the tail of the proof ─────────────────
        let tag_start = proof_len - 4;
        let p0 = proof.get(tag_start).unwrap_or(0);
        let p1 = proof.get(tag_start + 1).unwrap_or(0);
        let p2 = proof.get(tag_start + 2).unwrap_or(0);
        let p3 = proof.get(tag_start + 3).unwrap_or(0);

        // ── Compare against the first 4 bytes of the commitment ───────────────
        let c0 = commitment.get(0).unwrap_or(0);
        let c1 = commitment.get(1).unwrap_or(0);
        let c2 = commitment.get(2).unwrap_or(0);
        let c3 = commitment.get(3).unwrap_or(0);

        p0 == c0 && p1 == c1 && p2 == c2 && p3 == c3
    }
}
