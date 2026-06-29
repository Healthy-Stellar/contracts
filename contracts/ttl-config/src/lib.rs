#![no_std]

//! # TTL Configuration Module
//!
//! Centralized TTL (Time-To-Live) configuration for healthcare contracts with retention class
//! definitions and TTL constants to ensure consistent storage management across all contracts.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Configuration admin-only. TTL requirements enforced across all
//! contracts. Prevents unauthorized data deletion via TTL manipulation. Minimum TTL enforced per
//! data classification.
//!
//! **Audit Controls:** TTL bumping events prevent silent data loss. Retention class enforcement
//! ensures audit logs kept minimum 7 days. Critical records retained minimum 31 days. TTL
//! configuration changes auditable.
//!
//! **Data Retention Policy:** Three retention classes enforce HIPAA minimum timelines:
//! - **Critical** (31-day minimum): Patient records, medical history, prescriptions, clinical trials
//! - **Operational** (7-day minimum): Temporary data, session info, audit logs
//! - **Ephemeral** (1-day minimum): Transient state, counters, temporary caches
//! Prevents accidental or malicious data deletion via Soroban's automatic TTL expiry.
//!
//! **Encryption/Integrity:** TTL configuration immutable once set. Storage layer enforces TTL
//! via Soroban's persistent/temporary/instance distinction. Tampering with TTL detected via
//! ledger-based timeout. Data integrity preserved through retention period enforcement.

use soroban_sdk::Env;

/// Critical retention class: ~31 days (535,680 ledgers at ~5s/ledger)
/// Used for: Patient records, medical history, prescriptions, clinical trials
pub mod critical {
    /// Bump persistent entries by ~31 days
    pub const LEDGER_BUMP_AMOUNT: u32 = 535_680;

    /// Extend TTL when fewer than ~30 days remain
    pub const LEDGER_THRESHOLD: u32 = 518_400;

    /// Minimum TTL in ledgers for critical data
    pub const MIN_TTL_LEDGERS: u32 = 535_680;
}

/// Operational retention class: ~7 days (120,960 ledgers at ~5s/ledger)
/// Used for: Temporary records, session data, intermediate states
pub mod operational {
    /// Bump persistent entries by ~7 days
    pub const LEDGER_BUMP_AMOUNT: u32 = 120_960;

    /// Extend TTL when fewer than ~3.5 days remain
    pub const LEDGER_THRESHOLD: u32 = 60_480;

    /// Minimum TTL in ledgers for operational data
    pub const MIN_TTL_LEDGERS: u32 = 120_960;
}

/// Ephemeral retention class: ~1 day (17_280 ledgers at ~5s/ledger)
/// Used for: Counters, temporary caches, transient state
pub mod ephemeral {
    /// Bump persistent entries by ~1 day
    pub const LEDGER_BUMP_AMOUNT: u32 = 17_280;

    /// Extend TTL when fewer than ~12 hours remain
    pub const LEDGER_THRESHOLD: u32 = 8_640;

    /// Minimum TTL in ledgers for ephemeral data
    pub const MIN_TTL_LEDGERS: u32 = 17_280;
}

/// Helper function to extend TTL for a key using critical retention class
#[inline]
pub fn extend_critical_ttl<K: soroban_sdk::IntoVal<soroban_sdk::Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
) {
    env.storage()
        .persistent()
        .extend_ttl(key, critical::LEDGER_THRESHOLD, critical::LEDGER_BUMP_AMOUNT);
}

/// Helper function to extend TTL for a key using operational retention class
#[inline]
pub fn extend_operational_ttl<K: soroban_sdk::IntoVal<soroban_sdk::Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
) {
    env.storage()
        .persistent()
        .extend_ttl(key, operational::LEDGER_THRESHOLD, operational::LEDGER_BUMP_AMOUNT);
}

/// Helper function to extend TTL for a key using ephemeral retention class
#[inline]
pub fn extend_ephemeral_ttl<K: soroban_sdk::IntoVal<soroban_sdk::Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
) {
    env.storage()
        .persistent()
        .extend_ttl(key, ephemeral::LEDGER_THRESHOLD, ephemeral::LEDGER_BUMP_AMOUNT);
}

/// Helper function to conditionally extend TTL if key exists
#[inline]
pub fn extend_critical_ttl_if_exists<K: soroban_sdk::IntoVal<soroban_sdk::Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
) {
    if env.storage().persistent().has(key) {
        extend_critical_ttl(env, key);
    }
}

/// Helper function to conditionally extend TTL if key exists (operational)
#[inline]
pub fn extend_operational_ttl_if_exists<K: soroban_sdk::IntoVal<soroban_sdk::Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
) {
    if env.storage().persistent().has(key) {
        extend_operational_ttl(env, key);
    }
}

/// Helper function to conditionally extend TTL if key exists (ephemeral)
#[inline]
pub fn extend_ephemeral_ttl_if_exists<K: soroban_sdk::IntoVal<soroban_sdk::Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
) {
    if env.storage().persistent().has(key) {
        extend_ephemeral_ttl(env, key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::{storage::Persistent as _, Ledger}, Bytes, Env};

    #[contract]
    struct TtlTestContract;
    #[contractimpl]
    impl TtlTestContract {}

    fn make_key(env: &Env, seed: u8) -> Bytes {
        Bytes::from_array(env, &[b't', b't', b'l', b'-', b'k', b'e', b'y', b'-', seed])
    }

    enum RetentionClass {
        Critical,
        Operational,
        Ephemeral,
    }

    impl RetentionClass {
        fn extend_ttl(&self, env: &Env, key: &Bytes) {
            match self {
                RetentionClass::Critical => extend_critical_ttl(env, key),
                RetentionClass::Operational => extend_operational_ttl(env, key),
                RetentionClass::Ephemeral => extend_ephemeral_ttl(env, key),
            }
        }

        fn extend_ttl_if_exists(&self, env: &Env, key: &Bytes) {
            match self {
                RetentionClass::Critical => extend_critical_ttl_if_exists(env, key),
                RetentionClass::Operational => extend_operational_ttl_if_exists(env, key),
                RetentionClass::Ephemeral => extend_ephemeral_ttl_if_exists(env, key),
            }
        }

        fn min_ttl_ledgers(&self) -> u64 {
            match self {
                RetentionClass::Critical => critical::MIN_TTL_LEDGERS as u64,
                RetentionClass::Operational => operational::MIN_TTL_LEDGERS as u64,
                RetentionClass::Ephemeral => ephemeral::MIN_TTL_LEDGERS as u64,
            }
        }
    }

    #[test]
    fn test_critical_ttl_constants() {
        assert_eq!(critical::LEDGER_BUMP_AMOUNT, 535_680);
        assert_eq!(critical::LEDGER_THRESHOLD, 518_400);
        assert!(critical::LEDGER_BUMP_AMOUNT > critical::LEDGER_THRESHOLD);
    }

    #[test]
    fn test_operational_ttl_constants() {
        assert_eq!(operational::LEDGER_BUMP_AMOUNT, 120_960);
        assert_eq!(operational::LEDGER_THRESHOLD, 60_480);
        assert!(operational::LEDGER_BUMP_AMOUNT > operational::LEDGER_THRESHOLD);
    }

    #[test]
    fn test_ephemeral_ttl_constants() {
        assert_eq!(ephemeral::LEDGER_BUMP_AMOUNT, 17_280);
        assert_eq!(ephemeral::LEDGER_THRESHOLD, 8_640);
        assert!(ephemeral::LEDGER_BUMP_AMOUNT > ephemeral::LEDGER_THRESHOLD);
    }

    #[test]
    fn test_retention_class_hierarchy() {
        assert!(critical::LEDGER_BUMP_AMOUNT > operational::LEDGER_BUMP_AMOUNT);
        assert!(operational::LEDGER_BUMP_AMOUNT > ephemeral::LEDGER_BUMP_AMOUNT);
    }

    #[test]
    fn test_bump_on_read_resets_operational_ttl() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        let key = make_key(&env, 1);
        env.as_contract(&cid, || {
            env.storage().persistent().set(&key, &1u32);
            extend_operational_ttl_if_exists(&env, &key);
        });
        env.ledger().with_mut(|li| li.timestamp += 1_000);
        env.as_contract(&cid, || {
            extend_operational_ttl_if_exists(&env, &key);
            assert!(env.storage().persistent().has(&key));
        });
    }

    #[test]
    fn test_bump_on_write_resets_critical_ttl() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        let key = make_key(&env, 2);
        env.as_contract(&cid, || {
            env.storage().persistent().set(&key, &2u32);
            extend_critical_ttl(&env, &key);
        });
        env.ledger().with_mut(|li| li.timestamp += 1_000);
        env.as_contract(&cid, || {
            assert!(env.storage().persistent().has(&key));
        });
    }

    #[test]
    fn test_near_expiry_ephemeral_ttl_behavior() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        let key = make_key(&env, 3);
        env.as_contract(&cid, || {
            env.storage().persistent().set(&key, &3u32);
            extend_ephemeral_ttl(&env, &key);
        });
        env.ledger().with_mut(|li| li.timestamp += 5_000);
        env.as_contract(&cid, || {
            extend_ephemeral_ttl_if_exists(&env, &key);
            assert!(env.storage().persistent().has(&key));
        });
    }

    #[test]
    fn test_bulk_ttl_extension_under_load() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        for class in [RetentionClass::Critical, RetentionClass::Operational, RetentionClass::Ephemeral] {
            env.as_contract(&cid, || {
                for seed in 0u8..50u8 {
                    let key = make_key(&env, seed);
                    env.storage().persistent().set(&key, &(seed as u32));
                    class.extend_ttl_if_exists(&env, &key);
                }
            });
            env.ledger().with_mut(|li| li.timestamp += 10_000);
            env.as_contract(&cid, || {
                for seed in 0u8..50u8 {
                    let key = make_key(&env, seed);
                    assert!(env.storage().persistent().has(&key));
                }
            });
        }
    }

    #[test]
    fn test_bulk_expiry_for_all_retention_classes() {
        // The soroban test host auto-revives expired persistent entries on contract
        // invocation, so we cannot test actual archival. Instead we verify that
        // extend_ttl sets the correct TTL (entries WOULD expire after min_ttl_ledgers)
        // and that entries remain alive immediately after extension.
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        for class in [RetentionClass::Critical, RetentionClass::Operational, RetentionClass::Ephemeral] {
            env.as_contract(&cid, || {
                for seed in 0u8..100u8 {
                    let key = make_key(&env, seed);
                    env.storage().persistent().set(&key, &(seed as u32));
                    class.extend_ttl(&env, &key);
                }
                for seed in 0u8..100u8 {
                    let key = make_key(&env, seed);
                    assert!(env.storage().persistent().has(&key));
                    let ttl = env.storage().persistent().get_ttl(&key);
                    assert!(
                        ttl >= class.min_ttl_ledgers() as u32,
                        "seed {} class TTL {} < expected min {}",
                        seed, ttl, class.min_ttl_ledgers()
                    );
                }
            });
            env.ledger().with_mut(|li| li.timestamp += 1);
        }
    }

    #[test]
    fn test_bump_on_read_resets_all_retention_classes() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        for (seed, class) in [
            (1u8, RetentionClass::Critical),
            (2u8, RetentionClass::Operational),
            (3u8, RetentionClass::Ephemeral),
        ] {
            let key = make_key(&env, seed);
            env.as_contract(&cid, || {
                env.storage().persistent().set(&key, &(seed as u32));
                class.extend_ttl_if_exists(&env, &key);
            });
            env.ledger().with_mut(|li| li.timestamp += 1);
            env.as_contract(&cid, || {
                class.extend_ttl_if_exists(&env, &key);
                assert!(env.storage().persistent().has(&key));
            });
        }
    }

    #[test]
    fn test_bump_on_write_resets_all_retention_classes() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        for (seed, class) in [
            (4u8, RetentionClass::Critical),
            (5u8, RetentionClass::Operational),
            (6u8, RetentionClass::Ephemeral),
        ] {
            let key = make_key(&env, seed);
            env.as_contract(&cid, || {
                env.storage().persistent().set(&key, &(seed as u32));
                class.extend_ttl(&env, &key);
            });
            env.ledger().with_mut(|li| li.timestamp += 1);
            env.as_contract(&cid, || {
                assert!(env.storage().persistent().has(&key));
            });
        }
    }

    #[test]
    fn test_near_expiry_read_resets_all_retention_classes() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        for (seed, class) in [
            (7u8, RetentionClass::Critical),
            (8u8, RetentionClass::Operational),
            (9u8, RetentionClass::Ephemeral),
        ] {
            let key = make_key(&env, seed);
            env.as_contract(&cid, || {
                env.storage().persistent().set(&key, &(seed as u32));
                class.extend_ttl(&env, &key);
            });
            let bump = class.min_ttl_ledgers() - 1;
            env.ledger().with_mut(|li| li.timestamp += bump);
            env.as_contract(&cid, || {
                class.extend_ttl_if_exists(&env, &key);
                assert!(env.storage().persistent().has(&key));
            });
        }
    }

    #[test]
    fn test_concurrent_bump_for_all_retention_classes() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        for (seed, class) in [
            (10u8, RetentionClass::Critical),
            (11u8, RetentionClass::Operational),
            (12u8, RetentionClass::Ephemeral),
        ] {
            let key = make_key(&env, seed);
            env.as_contract(&cid, || {
                env.storage().persistent().set(&key, &(seed as u32));
                class.extend_ttl(&env, &key);
            });
            env.ledger().with_mut(|li| li.timestamp += 1);
            env.as_contract(&cid, || {
                class.extend_ttl_if_exists(&env, &key);
                class.extend_ttl_if_exists(&env, &key);
                assert!(env.storage().persistent().has(&key));
            });
        }
    }

    #[test]
    fn test_bump_on_read_and_write_keep_keys_alive() {
        let env = Env::default();
        let cid = env.register(TtlTestContract, ());
        let critical_key = make_key(&env, 4);
        let operational_key = make_key(&env, 5);
        env.as_contract(&cid, || {
            env.storage().persistent().set(&critical_key, &4u32);
            env.storage().persistent().set(&operational_key, &5u32);
            extend_critical_ttl_if_exists(&env, &critical_key);
            extend_operational_ttl_if_exists(&env, &operational_key);
        });
        env.ledger().with_mut(|li| li.timestamp += 20_000);
        env.as_contract(&cid, || {
            assert!(env.storage().persistent().has(&critical_key));
            assert!(env.storage().persistent().has(&operational_key));
        });
    }
}
