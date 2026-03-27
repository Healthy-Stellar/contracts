#![no_std]
#![allow(deprecated)]

//! # Zero-knowledge eligibility verification (interface scaffolding)
//!
//! This crate defines a **stable Rust trait boundary** between on-chain medical-benefit
//! logic and a **future** zero-knowledge (ZK) proof system. The goal is to let a
//! **patient** demonstrate *eligibility* for a benefit (e.g. coverage tier, prior
//! authorization precondition, clinical program enrollment) **without** posting
//! identifiable health data or full medical records on-chain.
//!
//! ## Problem being addressed
//!
//! On a public ledger, storing plaintext diagnoses, medications, or visit history
//! to decide eligibility is usually unacceptable under privacy regulation and
//! clinical trust models. A ZK proof allows a prover (often the patient or their
//! wallet, assisted by an off-chain prover service) to convince the contract that
//! a statement of the form below is true:
//!
//! > “There exists a private witness *w* (e.g. structured clinical facts) such that
//! > a public predicate *P(public_inputs, w)* holds, and *w* was bound correctly to
//! > this `patient` identity and benefit policy.”
//!
//! The verifier on-chain only sees **`proof`**, **`public_inputs`**, and
//! **`patient`** (and any other arguments you add in a real integration)—not *w*.
//!
//! ## Expected ZK system interface (conceptual)
//!
//! A production deployment will pair this trait with an off-chain **circuit**
//! (e.g. R1CS / Plonkish arithmetization) and an on-chain **verifying key** (VK)
//! or VK commitment. Typical responsibilities split as follows:
//!
//! | Layer | Responsibility |
//! |------|----------------|
//! | **Circuit / relation** | Encodes eligibility rules as constraints over private witness and public inputs (e.g. hash of policy id, benefit id, epoch, Merkle roots of allow-lists). |
//! | **Prover** | Takes witness + public inputs, outputs `proof` bytes and the same `public_inputs` the verifier checks. |
//! | **Verifier (this trait)** | Parses `proof`, loads VK, runs pairing / polynomial checks (depending on proof system), returns `true` iff valid. |
//! | **Contract** | Calls the verifier, then updates benefit state (e.g. mint entitlement, set flag) only when verification succeeds. |
//!
//! **`public_inputs`** are values that **both** prover and verifier agree on and
//! that appear in the proof’s public statement. Examples (serialized as [`Bytes`]
//! elements in this stub):
//!
//! - Benefit program id, policy version, or contract-specific `benefit_id`
//! - Time window or ledger-bound **nullifier** domain separator (anti-replay)
//! - Commitments to **allowed** diagnosis / procedure code sets (Merkle roots)
//! - A **nullifier** or rate-limit tag derived from patient secret + epoch (if
//!   designed into the circuit to prevent double-claiming)
//!
//! **`proof`** is an opaque byte encoding defined by your proof system (Groth16,
//! Plonk, STARKs, etc.). On Soroban you must keep proof size and verification cost
//! within protocol limits—document concrete max sizes and gas for your chosen system.
//!
//! ## Soroban-specific design notes
//!
//! - **No `Env` on the trait today**: the stub does not need it. A real verifier
//!   implementation will likely take `&Env` (or store one during construction) to
//!   access **host crypto** primitives, metering, and storage for a cached VK hash.
//! - **Storage of verifying keys**: prefer storing a **hash** of the VK on-chain
//!   and pinning the full VK off-chain (IPFS / institutional registry), or store a
//!   compact VK representation if small enough. Version VK with `policy_version` in
//!   `public_inputs` to allow upgrades.
//! - **Determinism**: verification must be deterministic; avoid floating point or
//!   nondeterministic parsing in the verifier.
//! - **Failure modes**: return `false` for any malformed proof; reserve `panic!`
//!   only for invariant bugs, not for “invalid proof” (unless your contract
//!   standardizes panics for denial).
//!
//! ## Security expectations (non-exhaustive)
//!
//! - The proof must **bind** to `patient` if individual claims must not be replayed
//!   across addresses—usually by including `patient` (or a commitment derived from
//!   patient-held secrets) inside the circuit’s public inputs or transcript.
//! - **Soundness**: under the proof system’s assumptions, a polynomial-time adversary
//!   cannot forge `proof` for false `public_inputs`.
//! - **Privacy**: raw PHI should not appear in `public_inputs` unless intentionally
//!   disclosed; prefer commitments and hashes.
//!
//! ## This crate’s status
//!
//! [`PlaceholderZkProofVerifier`] implements [`ZKProofVerifier`] and **always returns
//! `false`**. Replace it with a real implementation once the circuit, VK layout, and
//! proof serialization are fixed.

use soroban_sdk::{Address, Bytes, Vec};

/// Verifies a zero-knowledge proof that a patient satisfies eligibility for a
/// medical benefit **without** requiring full medical record disclosure on-chain.
///
/// # Parameters
///
/// - **`patient`**: On-chain identity (Stellar / Soroban [`Address`]) that the proof
///   must be bound to in your circuit design (or that the caller uses for
///   authorization after verification).
/// - **`proof`**: Opaque proof bytes from your chosen ZK scheme. Layout is *not*
///   specified here; document it alongside the circuit artifact.
/// - **`public_inputs`**: Vector of public circuit inputs, each as [`Bytes`].
///   Typical encodings: fixed-width field elements, `U256` big-endian, or short
///   structured blobs agreed off-chain.
///
/// # Returns
///
/// - `true` if and only if the proof is valid under the deployed verifying key and
///   matches `public_inputs` (and any internal binding checks).
/// - `false` for invalid, malformed, or non-canonical proofs, **or** for this
///   placeholder implementation.
///
/// # Future work
///
/// Real implementations should document: proof system name, curve, VK id, maximum
/// lengths for `proof` and `public_inputs`, and versioning of the byte format.
pub trait ZKProofVerifier {
    /// Returns whether the supplied proof attests to eligibility for the patient.
    ///
    /// Stub implementations should return `false` until cryptographic verification
    /// is wired in.
    fn verify_eligibility_proof(
        &self,
        patient: Address,
        proof: Bytes,
        public_inputs: Vec<Bytes>,
    ) -> bool;
}

/// A compile-time placeholder verifier used until a real ZK verifier is integrated.
///
/// Always returns `false` so contracts cannot accidentally treat unimplemented
/// verification as successful.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlaceholderZkProofVerifier;

impl ZKProofVerifier for PlaceholderZkProofVerifier {
    fn verify_eligibility_proof(
        &self,
        patient: Address,
        proof: Bytes,
        public_inputs: Vec<Bytes>,
    ) -> bool {
        // Explicitly consume arguments so the signature stays stable and the
        // compiler does not warn under #![no_std] when building as a dependency.
        let _ = (patient, proof, public_inputs);
        false
    }
}
