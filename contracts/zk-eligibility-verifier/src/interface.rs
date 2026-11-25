//! Core trait and placeholder for ZK medical-eligibility verification.
//!
//! See the [crate-level documentation](crate) and `INTERFACE.md` for the full
//! expected proof-system boundary, security notes, and Soroban integration guidance.

use soroban_sdk::{Address, Bytes, Env, Vec};

/// Semantic version label for this **Rust API** (not the future circuit version).
/// Bump when `ZKProofVerifier` method signatures or `public_inputs` encoding contract changes.
pub const RUST_INTERFACE_VERSION: &str = "1.1.0-stub";

/// Public inputs to the eligibility relation, as opaque byte blobs agreed with the circuit.
///
/// In Soroban, this is [`Vec<Bytes>`](Vec): each element is one public input field (or a
/// packed struct serialized off-chain). A production circuit should fix **ordering** and
/// **width** of each element and document them next to the verifying key artifact.
///
/// # Required layout
///
/// The circuit **must** expose, at minimum, the following public inputs (in this order):
///
/// 1. **Subject binding** — `H(subject_address)` (or the subject's strkey bytes). The
///    verifier checks this against the `patient`/`subject` argument so a proof cannot be
///    replayed for a different address (front-running / proof selling).
/// 2. **Circuit nullifier** — derived inside the circuit from the holder's secret and a
///    context/epoch. Stored on-chain to prevent witness reuse. This is *not* a hash of the
///    proof bytes: Groth16 proofs are malleable, so byte hashes are not sound nullifiers.
///
/// Implementations **may** additionally bind:
///
/// 3. **Contract address** — `env.current_contract_address()` bytes, to prevent
///    cross-deployment replay.
/// 4. **Network passphrase hash** — to prevent cross-network replay.
///
/// The concrete encoding (width, endianness, hash function) is fixed by the circuit and
/// documented next to the verifying key artifact.
pub type PublicInputs = Vec<Bytes>;

/// Index of the subject-binding public input within [`PublicInputs`].
pub const PUBLIC_INPUT_SUBJECT_HASH: u32 = 0;

/// Index of the circuit-level nullifier public input within [`PublicInputs`].
pub const PUBLIC_INPUT_NULLIFIER: u32 = 1;

/// Minimum number of public inputs required by the eligibility relation.
pub const MIN_PUBLIC_INPUTS: u32 = 2;

/// Verifies a zero-knowledge proof that a patient satisfies eligibility for a
/// medical benefit **without** requiring full medical record disclosure on-chain.
///
/// # Parameters
///
/// - **`patient`**: On-chain identity ([`Address`]) the proof must bind to. Implementations
///   **must** check this against the subject-binding public input
///   ([`PUBLIC_INPUT_SUBJECT_HASH`]) before verifying, so a valid proof cannot be replayed
///   for a different address.
/// - **`proof`**: Opaque bytes for your proof system (Groth16, Plonk, STARK, …).
/// - **`public_inputs`**: See [`PublicInputs`] for the required layout, including the
///   subject binding and the circuit-level nullifier.
///
/// # Returns
///
/// `true` only when cryptographic verification succeeds under a deployed VK and the
/// statement matches `public_inputs` **and** the subject binding matches `patient`.
/// Implementations **must** return `false` for any parse error, wrong VK, mismatched
/// subject, or failed equation — never “soft-accept.”
///
/// The [`PlaceholderZkProofVerifier`] always returns `false`.
pub trait ZKProofVerifier {
    /// Stub implementations should return `false` until real verification is implemented.
    fn verify_eligibility_proof(
        &self,
        patient: Address,
        proof: Bytes,
        public_inputs: PublicInputs,
    ) -> bool;

    /// Returns the circuit-level nullifier from `public_inputs`, if present.
    ///
    /// Callers should store this value (rather than a hash of `proof`) to prevent witness
    /// reuse. Returns `None` when the input is missing or malformed.
    fn nullifier(&self, public_inputs: &PublicInputs) -> Option<Bytes> {
        public_inputs.get(PUBLIC_INPUT_NULLIFIER)
    }
}

/// Dispatches to [`ZKProofVerifier::verify_eligibility_proof`] for any verifier instance.
///
/// Intended for call sites that prefer a function value over method syntax (e.g. when
/// passing `&dyn ZKProofVerifier` is awkward in `no_std`); equivalent to
/// `verifier.verify_eligibility_proof(...)`.
#[inline]
pub fn verify_eligibility_proof(
    verifier: &impl ZKProofVerifier,
    patient: Address,
    proof: Bytes,
    public_inputs: PublicInputs,
) -> bool {
    verifier.verify_eligibility_proof(patient, proof, public_inputs)
}

/// Returns the circuit-level nullifier from `public_inputs`, if present.
///
/// Equivalent to `verifier.nullifier(&public_inputs)`; provided for symmetry with
/// [`verify_eligibility_proof`].
#[inline]
pub fn nullifier(verifier: &impl ZKProofVerifier, public_inputs: &PublicInputs) -> Option<Bytes> {
    verifier.nullifier(public_inputs)
}

/// Computes the subject-binding public input for `subject` under the given `env`.
///
/// This is a convenience helper for off-chain proof generation and for on-chain checks.
/// The exact hash function and encoding **must** match the circuit; this helper uses
/// `sha256` over the subject's strkey bytes as the default convention.
pub fn subject_binding(env: &Env, subject: &Address) -> Bytes {
    let _ = env;
    let strkey = subject.to_string();
    let mut buf = Bytes::new(env);
    let len = strkey.len();
    let mut i = 0;
    while i < len {
        buf.push_back(strkey.as_byte(i));
        i += 1;
    }
    env.crypto().sha256(&buf).to_bytes()
}

/// Placeholder verifier: **always returns `false`** so unimplemented verification cannot
/// authorize benefits by accident.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlaceholderZkProofVerifier;

impl ZKProofVerifier for PlaceholderZkProofVerifier {
    fn verify_eligibility_proof(
        &self,
        patient: Address,
        proof: Bytes,
        public_inputs: PublicInputs,
    ) -> bool {
        let _ = (patient, proof, public_inputs);
        false
    }

    fn nullifier(&self, public_inputs: &PublicInputs) -> Option<Bytes> {
        public_inputs.get(PUBLIC_INPUT_NULLIFIER)
    }
}
