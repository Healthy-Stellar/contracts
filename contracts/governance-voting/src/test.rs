#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, String};

fn setup() -> (Env, GovernanceVotingContractClient<'static>, Address) {
    let env         = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(GovernanceVotingContract, ());
    let client      = GovernanceVotingContractClient::new(&env, &contract_id);
    let admin       = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

fn s(env: &Env, v: &str) -> String { String::from_str(env, v) }

fn create(env: &Env, client: &GovernanceVotingContractClient, admin: &Address) -> u64 {
    client.create_proposal(admin, &s(env, "T"), &s(env, "D"), &3, &86_400)
}

#[test]
fn create_proposal_returns_id_1() {
    let (env, client, admin) = setup();
    let id = create(&env, &client, &admin);
    assert_eq!(id, 1);
}

#[test]
fn vote_yes_increments_yes_votes() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    client.register_member(&admin, &voter);
    let id    = create(&env, &client, &admin);
    client.vote(&voter, &id, &VoteChoice::Yes);
    let p = client.get_proposal(&id);
    assert_eq!(p.yes_votes, 1);
    assert_eq!(p.no_votes,  0);
}

#[test]
fn vote_no_increments_no_votes() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    client.register_member(&admin, &voter);
    let id    = create(&env, &client, &admin);
    client.vote(&voter, &id, &VoteChoice::No);
    let p = client.get_proposal(&id);
    assert_eq!(p.no_votes, 1);
}

#[test]
#[should_panic]
fn double_vote_panics() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    client.register_member(&admin, &voter);
    let id    = create(&env, &client, &admin);
    client.vote(&voter, &id, &VoteChoice::Yes);
    client.vote(&voter, &id, &VoteChoice::No);
}

#[test]
fn finalize_passes_when_quorum_met_and_yes_majority() {
    let (env, client, admin) = setup();
    let id = create(&env, &client, &admin);
    for _ in 0..3 {
        let v = Address::generate(&env);
        client.register_member(&admin, &v);
        client.vote(&v, &id, &VoteChoice::Yes);
    }
    env.ledger().set_timestamp(env.ledger().timestamp() + 86_401);
    let status = client.finalize(&id);
    assert_eq!(status, ProposalStatus::Passed);
}

#[test]
fn finalize_rejected_when_quorum_not_met() {
    let (env, client, admin) = setup();
    let id = create(&env, &client, &admin);
    let v = Address::generate(&env);
    client.register_member(&admin, &v);
    client.vote(&v, &id, &VoteChoice::Yes); // only 1, quorum=3
    env.ledger().set_timestamp(env.ledger().timestamp() + 86_401);
    let status = client.finalize(&id);
    assert_eq!(status, ProposalStatus::Rejected);
}

#[test]
fn has_voted_returns_false_before_voting() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    client.register_member(&admin, &voter);
    let id    = create(&env, &client, &admin);
    assert!(!client.has_voted(&id, &voter));
    client.vote(&voter, &id, &VoteChoice::Yes);
    assert!(client.has_voted(&id, &voter));
}

#[test]
#[should_panic]
fn non_admin_cannot_create_proposal() {
    let (env, client, admin) = setup();
    let non_admin = Address::generate(&env);
    client.create_proposal(&non_admin, &s(&env, "T"), &s(&env, "D"), &3, &86_400);
}

#[test]
#[should_panic]
fn non_member_cannot_vote() {
    let (env, client, admin) = setup();
    let non_member = Address::generate(&env);
    let id = create(&env, &client, &admin);
    client.vote(&non_member, &id, &VoteChoice::Yes);
}

#[test]
fn max_proposals_enforced() {
    let (env, client, admin) = setup();
    for i in 0..100 {
        let id = create(&env, &client, &admin);
        assert_eq!(id, (i + 1) as u64);
    }
    let res = client.try_create_proposal(&admin, &s(&env, "T"), &s(&env, "D"), &3, &86_400);
    assert_eq!(res, Err(Ok(Error::TooManyProposals)));
}

#[test]
fn active_proposals_decremented_on_finalize_and_allows_new_proposals() {
    let (env, client, admin) = setup();
    for i in 0..100 {
        let id = create(&env, &client, &admin);
        assert_eq!(id, (i + 1) as u64);
    }
    // 101st proposal fails with TooManyProposals
    let res = client.try_create_proposal(&admin, &s(&env, "T"), &s(&env, "D"), &3, &86_400);
    assert_eq!(res, Err(Ok(Error::TooManyProposals)));

    // Advance time past deadline and finalize 5 proposals (they become Rejected because quorum not met)
    env.ledger().set_timestamp(env.ledger().timestamp() + 86_401);
    for id in 1..=5 {
        let status = client.finalize(&id);
        assert_eq!(status, ProposalStatus::Rejected);
    }

    // Now 5 more proposals (IDs 101 to 105) can be created
    for i in 101..=105 {
        let id = create(&env, &client, &admin);
        assert_eq!(id, i as u64);
    }

    // 106th proposal hits the capacity limit again
    let res2 = client.try_create_proposal(&admin, &s(&env, "T"), &s(&env, "D"), &3, &86_400);
    assert_eq!(res2, Err(Ok(Error::TooManyProposals)));
}

#[test]
fn active_proposals_decremented_when_proposal_passes() {
    let (env, client, admin) = setup();
    let mut voters = soroban_sdk::Vec::new(&env);
    for _ in 0..3 {
        let voter = Address::generate(&env);
        client.register_member(&admin, &voter);
        voters.push_back(voter);
    }

    for _ in 0..100 {
        create(&env, &client, &admin);
    }

    // Vote yes on proposal 1 with quorum 3
    for i in 0..3 {
        let voter = voters.get(i).unwrap();
        client.vote(&voter, &1, &VoteChoice::Yes);
    }

    // Advance past deadline and finalize proposal 1
    env.ledger().set_timestamp(env.ledger().timestamp() + 86_401);
    let status = client.finalize(&1);
    assert_eq!(status, ProposalStatus::Passed);

    // Now creating a new proposal succeeds!
    let new_id = create(&env, &client, &admin);
    assert_eq!(new_id, 101);
}

#[test]
fn vote_after_deadline_returns_proposal_expired() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    client.register_member(&admin, &voter);

    let id = create(&env, &client, &admin);

    // Advance past deadline
    env.ledger().set_timestamp(env.ledger().timestamp() + 86_401);

    let result = client.try_vote(&voter, &id, &VoteChoice::Yes);
    assert_eq!(result, Err(Ok(Error::ProposalExpired)));

    let proposal = client.get_proposal(&id);
    assert_eq!(proposal.status, ProposalStatus::Expired);
    assert!(!client.has_voted(&id, &voter));
}

#[test]
fn active_proposals_decremented_on_expiry_during_vote() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    client.register_member(&admin, &voter);

    for i in 0..100 {
        let id = create(&env, &client, &admin);
        assert_eq!(id, (i + 1) as u64);
    }

    let proposal_id = 1u64;
    env.ledger().set_timestamp(env.ledger().timestamp() + 86_401);

    let result = client.try_vote(&voter, &proposal_id, &VoteChoice::Yes);
    assert_eq!(result, Err(Ok(Error::ProposalExpired)));

    let proposal = client.get_proposal(&proposal_id);
    assert_eq!(proposal.status, ProposalStatus::Expired);
    assert!(!client.has_voted(&proposal_id, &voter));

    let new_id = create(&env, &client, &admin);
    assert_eq!(new_id, 101);
}

#[test]
fn member_can_vote() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    client.register_member(&admin, &voter);
    let id = create(&env, &client, &admin);
    client.vote(&voter, &id, &VoteChoice::Yes);
    assert!(client.has_voted(&id, &voter));
}

#[test]
fn unregister_member_prevents_voting() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    client.register_member(&admin, &voter);
    client.unregister_member(&admin, &voter);
    let id = create(&env, &client, &admin);
    let res = client.try_vote(&voter, &id, &VoteChoice::Yes);
    assert!(res.is_err());
}
