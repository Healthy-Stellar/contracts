#![no_std]
#![allow(deprecated)]

//! # Zero-knowledge eligibility verification (interface scaffolding)
//!
//! **Labels:** advanced security · core logic  
//! This crate defines a **stable Rust trait boundary** between on-chain medical-benefit
//! logic and a **future** zero-knowledge (ZK) proof system. A **patient** (or delegate)
//! should eventually prove eligibility for a benefit **without** placing a full clinical
//! record on a public ledger.
//!
//! ## Quick start
//!
//! - Implement or inject [`ZKProofVerifier`].
//! - Until crypto exists, use [`PlaceholderZkProofVerifier`] — it **always returns `false`**.
//! - Call [`verify_eligibility_proof`] or the trait method with `patient`, opaque `proof`,
//!   and `public_inputs: `[`PublicInputs`] ([`Vec<Bytes>`](soroban_sdk::Vec)).
//!
//! ## Crate layout
//!
//! - [`interface`] — [`ZKProofVerifier`], [`PlaceholderZkProofVerifier`], [`PublicInputs`].
//! - **`INTERFACE.md`** in this crate directory — extended specification for implementers.
//!
//! ## Problem being addressed
//!
//! On a public ledger, storing plaintext diagnoses, medications, or visit history to
//! decide eligibility is usually unacceptable under privacy regulation and clinical trust
//! models. A ZK proof lets a prover convince the contract that:
//!
//! > There exists private witness *w* (e.g. structured clinical facts) such that
//! > *P(public_inputs, w)* holds, and *w* is bound to this patient and policy.
//!
//! The verifier sees only **`proof`**, **`public_inputs`**, and **`patient`** (plus any
//! extra args you add in production)—not *w*.
//!
//! ## Expected ZK system interface (summary)
//!
//! | Layer | Responsibility |
//! |------|----------------|
//! | **Circuit** | Encodes eligibility as constraints over *w* and `public_inputs`. |
//! | **Prover** | Outputs `proof` bytes + the same `public_inputs` the verifier checks. |
//! | **Verifier** ([`ZKProofVerifier`]) | Parses proof, uses VK, returns `true` iff valid. |
//! | **Contract** | Gates benefit state updates on `verify_eligibility_proof == true`. |
//!
//! For encoding details, versioning, Soroban host crypto, and security expectations,
//! read **INTERFACE.md** and the [crate::interface] docs.
//!
//! ## Soroban note
//!
//! A future implementation will likely require [`Env`](soroban_sdk::Env) for storage
//! (VK hash), metering, and host primitives (`crypto()`, etc.). The current trait stays
//! minimal so it can be implemented from a static context or extended with a wrapper
//! struct holding `Env`.

pub mod interface;

pub use interface::{
    verify_eligibility_proof, PlaceholderZkProofVerifier, PublicInputs, RUST_INTERFACE_VERSION,
    ZKProofVerifier,
};
