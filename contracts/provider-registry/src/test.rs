#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, String};

fn setup() -> (Env, Address, ProviderRegistryClient<'static>) {
    let env = Env::default();
    let contract_id = env.register_contract(None, ProviderRegistry);
    let client = ProviderRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin);
    (env, admin, client)
}

#[test]
fn test_register_and_is_provider() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    assert!(!client.is_provider(&provider));
    client.register_provider(&admin, &provider);
    assert!(client.is_provider(&provider));
}

#[test]
fn test_revoke_provider() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    client.register_provider(&admin, &provider);
    assert!(client.is_provider(&provider));

    client.revoke_provider(&admin, &provider);
    assert!(!client.is_provider(&provider));
}

#[test]
fn test_add_record_by_whitelisted_provider() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    client.register_provider(&admin, &provider);
    client.add_record(
        &provider,
        &String::from_str(&env, "REC001"),
        &String::from_str(&env, "Patient data"),
    );

    let rec = client.get_record(&String::from_str(&env, "REC001"));
    assert_eq!(rec.data, String::from_str(&env, "Patient data"));
    assert_eq!(rec.created_by, provider);
}

#[test]
#[should_panic(expected = "Unauthorized: not a whitelisted provider")]
fn test_add_record_rejected_for_non_provider() {
    let (env, _admin, client) = setup();
    let stranger = Address::generate(&env);

    client.add_record(
        &stranger,
        &String::from_str(&env, "REC002"),
        &String::from_str(&env, "Malicious data"),
    );
}

#[test]
#[should_panic(expected = "Unauthorized: not a whitelisted provider")]
fn test_add_record_rejected_after_revocation() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    client.register_provider(&admin, &provider);
    client.revoke_provider(&admin, &provider);

    client.add_record(
        &provider,
        &String::from_str(&env, "REC003"),
        &String::from_str(&env, "Should fail"),
    );
}

#[test]
#[should_panic(expected = "Unauthorized: admin only")]
fn test_register_provider_non_admin_rejected() {
    let (env, _admin, client) = setup();
    let non_admin = Address::generate(&env);
    let provider = Address::generate(&env);

    client.register_provider(&non_admin, &provider);
}

#[test]
#[should_panic(expected = "Unauthorized: admin only")]
fn test_revoke_provider_non_admin_rejected() {
    let (env, admin, client) = setup();
    let non_admin = Address::generate(&env);
    let provider = Address::generate(&env);

    client.register_provider(&admin, &provider);
    client.revoke_provider(&non_admin, &provider);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialize() {
    let (_env, admin, client) = setup();
    client.initialize(&admin);
}

// --- Rate limit: within limit (max 50/hour, add 3) ---
#[test]
fn test_rate_limit_within_limit() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    env.ledger().with_mut(|li| li.timestamp = 1_000_000);

    client.register_provider(&admin, &provider);
    client.set_rate_limit(&admin, &50, &3600);

    for id in ["REC-W-0", "REC-W-1", "REC-W-2"] {
        client.add_record(
            &provider,
            &String::from_str(&env, id),
            &String::from_str(&env, "data"),
        );
    }
}

// --- At limit: exactly max_records succeed, next fails ---
#[test]
fn test_rate_limit_at_limit() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    env.ledger().with_mut(|li| li.timestamp = 2_000_000);

    client.register_provider(&admin, &provider);
    const MAX: u32 = 5;
    client.set_rate_limit(&admin, &MAX, &3600);

    let ids = ["REC-A-0", "REC-A-1", "REC-A-2", "REC-A-3", "REC-A-4"];
    for id in ids {
        client.add_record(
            &provider,
            &String::from_str(&env, id),
            &String::from_str(&env, "data"),
        );
    }

    let over = client.try_add_record(
        &provider,
        &String::from_str(&env, "REC-A-OVER"),
        &String::from_str(&env, "data"),
    );
    assert!(matches!(over, Err(Ok(ContractError::RateLimitExceeded))));
}

// --- Over limit (same as at-limit verification) ---
#[test]
fn test_rate_limit_over_limit_rejected() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    env.ledger().with_mut(|li| li.timestamp = 3_000_000);

    client.register_provider(&admin, &provider);
    client.set_rate_limit(&admin, &2, &100);

    client.add_record(
        &provider,
        &String::from_str(&env, "R1"),
        &String::from_str(&env, "d"),
    );
    client.add_record(
        &provider,
        &String::from_str(&env, "R2"),
        &String::from_str(&env, "d"),
    );

    let third = client.try_add_record(
        &provider,
        &String::from_str(&env, "R3"),
        &String::from_str(&env, "d"),
    );
    assert!(matches!(third, Err(Ok(ContractError::RateLimitExceeded))));
}

// --- Window resets after window_seconds; can record again ---
#[test]
fn test_rate_limit_window_reset_allows_again() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    let t0 = 10_000u64;
    env.ledger().with_mut(|li| li.timestamp = t0);

    client.register_provider(&admin, &provider);
    client.set_rate_limit(&admin, &3, &3600);

    for id in ["REC-R-0", "REC-R-1", "REC-R-2"] {
        client.add_record(
            &provider,
            &String::from_str(&env, id),
            &String::from_str(&env, "data"),
        );
    }

    let blocked = client.try_add_record(
        &provider,
        &String::from_str(&env, "REC-BLOCKED"),
        &String::from_str(&env, "data"),
    );
    assert!(matches!(blocked, Err(Ok(ContractError::RateLimitExceeded))));

    // Past window end: new window starts
    env.ledger().with_mut(|li| li.timestamp = t0 + 3600);

    client.add_record(
        &provider,
        &String::from_str(&env, "REC-AFTER-RESET"),
        &String::from_str(&env, "data"),
    );
    assert_eq!(
        client.get_record(&String::from_str(&env, "REC-AFTER-RESET")).data,
        String::from_str(&env, "data")
    );
}

// ── deactivate_provider tests ─────────────────────────────────────────────────

#[test]
fn test_deactivate_provider_transfers_records_and_removes_whitelist() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    let successor = Address::generate(&env);

    client.register_provider(&admin, &provider);
    client.register_provider(&admin, &successor);

    client.add_record(
        &provider,
        &String::from_str(&env, "R1"),
        &String::from_str(&env, "data1"),
    );
    client.add_record(
        &provider,
        &String::from_str(&env, "R2"),
        &String::from_str(&env, "data2"),
    );

    // Confirm original ownership.
    assert_eq!(client.get_record(&String::from_str(&env, "R1")).created_by, provider);
    assert_eq!(client.get_record(&String::from_str(&env, "R2")).created_by, provider);

    client.deactivate_provider(&admin, &provider, &successor);

    // Provider removed from whitelist.
    assert!(!client.is_provider(&provider));

    // Both records now owned by successor.
    assert_eq!(client.get_record(&String::from_str(&env, "R1")).created_by, successor);
    assert_eq!(client.get_record(&String::from_str(&env, "R2")).created_by, successor);
}

#[test]
fn test_deactivate_provider_no_records() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    let successor = Address::generate(&env);

    client.register_provider(&admin, &provider);
    // No records added — deactivation should still succeed.
    client.deactivate_provider(&admin, &provider, &successor);
    assert!(!client.is_provider(&provider));
}

#[test]
#[should_panic(expected = "Unauthorized: admin only")]
fn test_deactivate_provider_non_admin_rejected() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    let successor = Address::generate(&env);
    let non_admin = Address::generate(&env);

    client.register_provider(&admin, &provider);
    client.deactivate_provider(&non_admin, &provider, &successor);
}

#[test]
fn test_deactivate_provider_successor_accumulates_records() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);
    let successor = Address::generate(&env);

    client.register_provider(&admin, &provider);
    client.register_provider(&admin, &successor);

    // Successor already has a record.
    client.add_record(
        &successor,
        &String::from_str(&env, "S1"),
        &String::from_str(&env, "succ data"),
    );
    // Provider has two records.
    client.add_record(
        &provider,
        &String::from_str(&env, "P1"),
        &String::from_str(&env, "prov data 1"),
    );
    client.add_record(
        &provider,
        &String::from_str(&env, "P2"),
        &String::from_str(&env, "prov data 2"),
    );

    client.deactivate_provider(&admin, &provider, &successor);

    // All three records now belong to successor.
    assert_eq!(client.get_record(&String::from_str(&env, "S1")).created_by, successor);
    assert_eq!(client.get_record(&String::from_str(&env, "P1")).created_by, successor);
    assert_eq!(client.get_record(&String::from_str(&env, "P2")).created_by, successor);
}

#[test]
fn test_rate_limit_disabled_with_zero_max() {
    let (env, admin, client) = setup();
    let provider = Address::generate(&env);

    env.ledger().with_mut(|li| li.timestamp = 4_000_000);

    client.register_provider(&admin, &provider);
    client.set_rate_limit(&admin, &0, &3600);

    let ids = [
        "REC-D-0", "REC-D-1", "REC-D-2", "REC-D-3", "REC-D-4", "REC-D-5", "REC-D-6", "REC-D-7",
        "REC-D-8", "REC-D-9",
    ];
    for id in ids {
        client.add_record(
            &provider,
            &String::from_str(&env, id),
            &String::from_str(&env, "data"),
        );
    }
}
