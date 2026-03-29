//! Core trait and placeholder for ZK medical-eligibility verification.
//!
//! See the [crate-level documentation](crate) and `INTERFACE.md` for the full
//! expected proof-system boundary, security notes, and Soroban integration guidance.

use soroban_sdk::{Address, Bytes, Vec};

/// Semantic version label for this **Rust API** (not the future circuit version).
/// Bump when `ZKProofVerifier` method signatures or `public_inputs` encoding contract changes.
pub const RUST_INTERFACE_VERSION: &str = "1.0.0-stub";

/// Public inputs to the eligibility relation, as opaque byte blobs agreed with the circuit.
///
/// In Soroban, this is [`Vec<Bytes>`](Vec): each element is one public input field (or a
/// packed struct serialized off-chain). A production circuit should fix **ordering** and
/// **width** of each element and document them next to the verifying key artifact.
pub type PublicInputs = Vec<Bytes>;

/// Verifies a zero-knowledge proof that a patient satisfies eligibility for a
/// medical benefit **without** requiring full medical record disclosure on-chain.
///
/// # Parameters
///
/// - **`patient`**: On-chain identity ([`Address`]) the proof must bind to (directly or via
///   a commitment in `public_inputs`) per your threat model.
/// - **`proof`**: Opaque bytes for your proof system (Groth16, Plonk, STARK, …).
/// - **`public_inputs`**: See [`PublicInputs`].
///
/// # Returns
///
/// `true` only when cryptographic verification succeeds under a deployed VK and the
/// statement matches `public_inputs`. Implementations **must** return `false` for any
/// parse error, wrong VK, or failed equation — never “soft-accept.”
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
}
