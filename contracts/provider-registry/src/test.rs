#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

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

    assert_eq!(
        client.get_record(&String::from_str(&env, "REC001")),
        String::from_str(&env, "Patient data")
    );
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
    let (env, admin, client) = setup();
    client.initialize(&admin);
}
