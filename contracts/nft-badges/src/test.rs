#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup() -> (Env, NftBadgesContractClient<'static>, Address) {
    let env         = Env::default();
    env.mock_all_auths();
    let admin       = Address::generate(&env);
    let contract_id = env.register(NftBadgesContract, (&admin,));
    let client      = NftBadgesContractClient::new(&env, &contract_id);
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

#[test]
fn revoke_flags_badge_with_reason() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    let id = client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u"));

    client.revoke(&admin, &id, &s(&env, "fraudulent credential"));

    let badge = client.get_badge(&id);
    assert!(badge.revoked);
    assert_eq!(badge.revoked_reason, Some(s(&env, "fraudulent credential")));
}

#[test]
fn double_revoke_is_rejected() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    let id = client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u"));

    client.revoke(&admin, &id, &s(&env, "reason 1"));
    let err = client.try_revoke(&admin, &id, &s(&env, "reason 2")).unwrap_err().unwrap();
    assert_eq!(err, Error::AlreadyRevoked);
}

#[test]
fn duplicate_mint_same_recipient_and_badge_type_is_rejected() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u1"));

    let err = client
        .try_mint(&admin, &student, &s(&env, "completion"), &s(&env, "B"), &s(&env, "u2"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyMinted);
}

#[test]
fn revoked_badge_allows_remint_of_same_type() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    let id = client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u1"));

    client.revoke(&admin, &id, &s(&env, "issued in error"));

    // Re-mint of the same badge_type must succeed after revocation.
    let id2 = client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u2"));
    assert!(id2 > id);
    let badge2 = client.get_badge(&id2);
    assert!(!badge2.revoked);
}
