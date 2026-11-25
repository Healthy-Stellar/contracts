//! # Counter Utilities
//!
//! Overflow-checked, monotonically-increasing `u64` id counters shared by every
//! contract. This module is a plain library (no `#[contract]` /
//! `#[contractimpl]`), so depending on it links no contract entry points into
//! the dependent WASM.

use soroban_sdk::Env;

// Every contract that needs a monotonically-increasing ID should call one of
// these helpers instead of open-coding `unwrap_or(0) + 1`.  The helpers:
//
//   1. Read the current value (defaulting to 0 on first use).
//   2. Perform a checked add — panicking on overflow so the contract halts
//      rather than silently wrapping and reusing an old ID.
//   3. Persist the incremented value back to storage.
//   4. Return the *new* value (i.e. the ID to use for the record being created).
//
// Two storage tiers are provided:
//   • `safe_increment`            — instance storage  (cheap, contract-lifetime)
//   • `safe_increment_persistent` — persistent storage (survives ledger archival)
//
// A namespaced variant is provided for per-entity counters (e.g. per-patient
// record sequences) where two entities must never share an ID space:
//   • `safe_increment_ns`            — instance storage,   key = (namespace, sub_key)
//   • `safe_increment_persistent_ns` — persistent storage, key = (namespace, sub_key)
//
// All four functions are generic over the storage-key type so callers can pass
// any `contracttype`-derived key without boxing.

/// Increment a `u64` counter stored in **instance** storage and return the new
/// value.  Panics with `"counter overflow"` if the counter would exceed
/// `u64::MAX`.
pub fn safe_increment<K>(env: &Env, key: &K) -> u64
where
    K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
{
    let current: u64 = env.storage().instance().get(key).unwrap_or(0u64);
    let next = current.checked_add(1).expect("counter overflow");
    env.storage().instance().set(key, &next);
    next
}

/// Increment a `u64` counter stored in **persistent** storage and return the
/// new value.  Panics with `"counter overflow"` if the counter would exceed
/// `u64::MAX`.
pub fn safe_increment_persistent<K>(env: &Env, key: &K) -> u64
where
    K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
{
    let current: u64 = env.storage().persistent().get(key).unwrap_or(0u64);
    let next = current.checked_add(1).expect("counter overflow");
    env.storage().persistent().set(key, &next);
    next
}

/// Increment a namespaced `u64` counter in **instance** storage.
///
/// The storage key is `(namespace, sub_key)`, keeping per-entity counters
/// isolated from each other and from global counters.
pub fn safe_increment_ns(env: &Env, namespace: &soroban_sdk::Symbol, sub_key: &soroban_sdk::Symbol) -> u64 {
    let compound = (namespace, sub_key);
    let current: u64 = env.storage().instance().get(&compound).unwrap_or(0u64);
    let next = current.checked_add(1).expect("counter overflow");
    env.storage().instance().set(&compound, &next);
    next
}

/// Increment a namespaced `u64` counter in **persistent** storage.
///
/// The storage key is `(namespace, sub_key)`, keeping per-entity counters
/// isolated from each other and from global counters.
pub fn safe_increment_persistent_ns(env: &Env, namespace: &soroban_sdk::Symbol, sub_key: &soroban_sdk::Symbol) -> u64 {
    let compound = (namespace, sub_key);
    let current: u64 = env.storage().persistent().get(&compound).unwrap_or(0u64);
    let next = current.checked_add(1).expect("counter overflow");
    env.storage().persistent().set(&compound, &next);
    next
}
