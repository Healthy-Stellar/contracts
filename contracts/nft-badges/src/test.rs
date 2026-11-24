#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Events as _, Address, Env, String};

fn setup() -> (Env, NftBadgesContractClient<'static>, Address) {
    let env         = Env::default();
    env.mock_all_auths();
    let admin       = Address::generate(&env);
    let contract_id = env.register(NftBadgesContract, (&admin,));
    let client      = NftBadgesContractClient::new(&env, &contract_id);
    // Allow the badge types used across the tests.
    client.add_badge_type(&admin, &s(&env, "completion"));
    client.add_badge_type(&admin, &s(&env, "t"));
    client.add_badge_type(&admin, &s(&env, "t1"));
    client.add_badge_type(&admin, &s(&env, "t2"));
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

#[test]
fn is_valid_reflects_revocation() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    let id = client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u"));

    assert!(client.is_valid(&id));
    client.revoke(&admin, &id, &s(&env, "fraud"));
    assert!(!client.is_valid(&id));
}

#[test]
fn has_active_badge_tracks_active_index() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    assert!(!client.has_active_badge(&student, &s(&env, "completion")));

    let id = client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u"));
    assert!(client.has_active_badge(&student, &s(&env, "completion")));
    assert!(!client.has_active_badge(&student, &s(&env, "t")));

    client.revoke(&admin, &id, &s(&env, "fraud"));
    assert!(!client.has_active_badge(&student, &s(&env, "completion")));

    // Re-mint restores the active index.
    client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u2"));
    assert!(client.has_active_badge(&student, &s(&env, "completion")));
}

#[test]
fn mint_event_includes_badge_type() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    let id = client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u"));

    let events = env.events().all();
    let mut found = false;
    for (contract, topics, data) in events.iter() {
        if contract != client.address {
            continue;
        }
        let (topic0, topic1): (Symbol, Address) = topics.clone().try_into().unwrap();
        if topic0 == Symbol::new(&env, "MINT") && topic1 == student {
            let (event_id, badge_type): (u64, String) = data.try_into().unwrap();
            assert_eq!(event_id, id);
            assert_eq!(badge_type, s(&env, "completion"));
            found = true;
        }
    }
    assert!(found, "mint event not found");
}

#[test]
fn revoke_event_includes_reason() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    let id = client.mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u"));
    client.revoke(&admin, &id, &s(&env, "fraudulent credential"));

    let events = env.events().all();
    let mut found = false;
    for (contract, topics, data) in events.iter() {
        if contract != client.address {
            continue;
        }
        let (topic0, topic1): (Symbol, Address) = topics.clone().try_into().unwrap();
        if topic0 == Symbol::new(&env, "REVOKE") && topic1 == student {
            let (event_id, reason): (u64, String) = data.try_into().unwrap();
            assert_eq!(event_id, id);
            assert_eq!(reason, s(&env, "fraudulent credential"));
            found = true;
        }
    }
    assert!(found, "revoke event not found");
}

#[test]
fn propose_and_accept_admin_rotates_admin() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);

    client.propose_admin(&admin, &new_admin);
    client.accept_admin(&new_admin);

    // Old admin can no longer mint.
    let student = Address::generate(&env);
    let err = client
        .try_mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Unauthorized);

    // New admin can mint.
    let id = client.mint(&new_admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, "u"));
    assert_eq!(id, 1);
}

#[test]
fn unauthorized_accepter_cannot_accept_admin() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);
    let attacker  = Address::generate(&env);

    client.propose_admin(&admin, &new_admin);
    let err = client.try_accept_admin(&attacker).unwrap_err().unwrap();
    assert_eq!(err, Error::Unauthorized);
}

#[test]
fn empty_strings_are_rejected() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);

    let err = client
        .try_mint(&admin, &student, &s(&env, "completion"), &s(&env, ""), &s(&env, "u"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidInput);

    let err = client
        .try_mint(&admin, &student, &s(&env, "completion"), &s(&env, "A"), &s(&env, ""))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidInput);
}

#[test]
fn oversized_strings_are_rejected() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);
    let long = String::from_str(&env, &"a".repeat(300));

    let err = client
        .try_mint(&admin, &student, &s(&env, "completion"), &long, &s(&env, "u"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidInput);
}

#[test]
fn disallowed_badge_type_is_rejected() {
    let (env, client, admin) = setup();
    let student = Address::generate(&env);

    let err = client
        .try_mint(&admin, &student, &s(&env, "not-allowed"), &s(&env, "A"), &s(&env, "u"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidBadgeType);
}
