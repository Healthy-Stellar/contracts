#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup() -> (Env, NftBadgesContractClient<'static>, Address) {
    let env         = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(NftBadgesContract, ());
    let client      = NftBadgesContractClient::new(&env, &contract_id);
    let admin       = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

fn s(env: &Env, v: &str) -> String { String::from_str(env, v) }

#[test]
fn mint_creates_badge_with_correct_metadata() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    let id = client.mint(
        &admin, &student,
        &s(&env, "completion"), &s(&env, "Course Complete"),
        &s(&env, "ipfs://Qm123"),
    );
    assert_eq!(id, 1);
    let badge = client.get_badge(&id);
    assert_eq!(badge.recipient, student);
    assert_eq!(badge.id, 1);
}

#[test]
fn badges_of_returns_all_minted_for_owner() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    client.mint(&admin, &student, &s(&env, "t1"), &s(&env, "A1"), &s(&env, "uri1"));
    client.mint(&admin, &student, &s(&env, "t2"), &s(&env, "A2"), &s(&env, "uri2"));
    let badges = client.badges_of(&student);
    assert_eq!(badges.len(), 2);
}

#[test]
fn id_increments_across_mints() {
    let (env, client, admin) = setup();
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let id1 = client.mint(&admin, &s1, &s(&env, "t"), &s(&env, "A"), &s(&env, "u"));
    let id2 = client.mint(&admin, &s2, &s(&env, "t"), &s(&env, "B"), &s(&env, "u"));
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
#[should_panic]
fn transfer_always_panics_soulbound() {
    let (env, client, admin) = setup();
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    client.mint(&admin, &s1, &s(&env, "t"), &s(&env, "A"), &s(&env, "u"));
    client.transfer(&s1, &s2, &1u64);
}

#[test]
#[should_panic]
fn non_admin_cannot_mint() {
    let (env, client, _admin) = setup();
    let attacker = Address::generate(&env);
    let student  = Address::generate(&env);
    client.mint(&attacker, &student, &s(&env, "t"), &s(&env, "A"), &s(&env, "u"));
}
