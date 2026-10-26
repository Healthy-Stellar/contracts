#![allow(dead_code)]

use soroban_sdk::{contracttype, panic_with_error, symbol_short, Address, Env, Symbol};

// ── Storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum PauseKey {
    /// Single global kill-switch: halts ALL write paths across the contract.
    Global,
    /// Feature-scoped pause: halts only the named write path (e.g. "prescribe").
    Feature(Symbol),
    /// The address authorised to toggle pause flags (admin or multisig proxy).
    PauseAdmin,
}

// ── Error ─────────────────────────────────────────────────────────────────────

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PauseError {
    /// The contract (or feature) is currently paused.
    Paused = 200,
    /// Caller is not the pause admin.
    NotPauseAdmin = 201,
    /// Pause admin has not been initialised.
    PauseAdminNotSet = 202,
    /// Pause admin has already been initialised.
    PauseAdminAlreadySet = 203,
}

// ── TTL ───────────────────────────────────────────────────────────────────────

/// Ledgers to extend pause state by on every touch (~30 days at 5s/ledger).
const PAUSE_TTL_LEDGERS: u32 = 518_400;

fn bump_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(PAUSE_TTL_LEDGERS, PAUSE_TTL_LEDGERS);
}

// ── Initialisation ────────────────────────────────────────────────────────────

/// Register the address that may toggle pause flags.
/// Call once during contract `initialize`. Rejects a second call so an
/// unauthenticated caller cannot silently overwrite the configured admin.
pub fn init_pause_admin(env: &Env, admin: &Address) {
    if env
        .storage()
        .instance()
        .has(&PauseKey::PauseAdmin)
    {
        panic_with_error!(env, PauseError::PauseAdminAlreadySet);
    }
    env.storage()
        .instance()
        .set(&PauseKey::PauseAdmin, admin);
    bump_ttl(env);
}

/// Rotate the pause admin. Only the current pause admin may rotate it.
pub fn rotate_pause_admin(env: &Env, caller: &Address, new_admin: &Address) {
    caller.require_auth();
    assert_pause_admin(env, caller);
    env.storage()
        .instance()
        .set(&PauseKey::PauseAdmin, new_admin);
    bump_ttl(env);
    env.events().publish(
        (symbol_short!("pause"), symbol_short!("rotate")),
        (caller.clone(), new_admin.clone()),
    );
}

// ── Guards (write paths call these) ──────────────────────────────────────────

/// Panics with `PauseError::Paused` when the global pause flag is set.
/// Read-only paths MUST NOT call this — they bypass it by design.
pub fn require_not_paused(env: &Env) {
    if is_paused(env) {
        panic_with_error!(env, PauseError::Paused);
    }
}

/// Panics with `PauseError::Paused` when either the global flag OR the named
/// feature flag is set.  Read-only paths MUST NOT call this.
pub fn require_not_paused_feature(env: &Env, feature: &Symbol) {
    if is_paused(env) || is_feature_paused(env, feature) {
        panic_with_error!(env, PauseError::Paused);
    }
}

// ── State queries (safe for read paths) ──────────────────────────────────────

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<PauseKey, bool>(&PauseKey::Global)
        .unwrap_or(false)
}

pub fn is_feature_paused(env: &Env, feature: &Symbol) -> bool {
    env.storage()
        .instance()
        .get::<PauseKey, bool>(&PauseKey::Feature(feature.clone()))
        .unwrap_or(false)
}

// ── Admin toggles ─────────────────────────────────────────────────────────────

/// Pause or unpause the entire contract.
/// `caller` must be the registered pause admin (single admin or multisig proxy).
pub fn set_paused(env: &Env, caller: &Address, paused: bool) {
    caller.require_auth();
    assert_pause_admin(env, caller);
    env.storage()
        .instance()
        .set(&PauseKey::Global, &paused);
    bump_ttl(env);
    let action = if paused {
        symbol_short!("paused")
    } else {
        symbol_short!("unpaused")
    };
    env.events()
        .publish((symbol_short!("pause"), action), caller.clone());
}

/// Pause or unpause a single named feature write path.
/// `caller` must be the registered pause admin.
pub fn set_feature_paused(env: &Env, caller: &Address, feature: Symbol, paused: bool) {
    caller.require_auth();
    assert_pause_admin(env, caller);
    env.storage()
        .instance()
        .set(&PauseKey::Feature(feature.clone()), &paused);
    bump_ttl(env);
    let action = if paused {
        symbol_short!("f_paused")
    } else {
        symbol_short!("f_resume")
    };
    env.events()
        .publish((symbol_short!("pause"), action, feature), caller.clone());
}

// ── Internal helper ───────────────────────────────────────────────────────────

fn assert_pause_admin(env: &Env, caller: &Address) {
    let admin: Address = env
        .storage()
        .instance()
        .get(&PauseKey::PauseAdmin)
        .unwrap_or_else(|| panic_with_error!(env, PauseError::PauseAdminNotSet));
    if admin != *caller {
        panic_with_error!(env, PauseError::NotPauseAdmin);
    }
}
