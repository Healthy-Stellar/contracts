//! Shared helpers used across the workspace contracts.
//!
//! This crate intentionally contains **no** `#[contract]` / `#[contractimpl]`
//! code. Linking it must never emit `#[export_name]` symbols into a dependent
//! WASM, so it is safe for every contract to depend on it.

/// Increment `value` by one, returning the new value.
///
/// Saturates at `u32::MAX` instead of overflowing.
pub fn safe_increment(value: u32) -> u32 {
    value.saturating_add(1)
}

/// Increment `value` by `amount`, returning the new value.
///
/// Saturates at `u32::MAX` instead of overflowing.
pub fn safe_increment_by(value: u32, amount: u32) -> u32 {
    value.saturating_add(amount)
}

/// Increment `value` by one, returning `None` on overflow.
pub fn safe_increment_checked(value: u32) -> Option<u32> {
    value.checked_add(1)
}

/// Increment `value` by `amount`, returning `None` on overflow.
pub fn safe_increment_by_checked(value: u32, amount: u32) -> Option<u32> {
    value.checked_add(amount)
}
