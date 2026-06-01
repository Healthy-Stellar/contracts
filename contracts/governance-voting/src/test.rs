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

fn create(env: &Env, client: &GovernanceVotingContractClient, proposer: &Address) -> u64 {
    client.create_proposal(proposer, &s(env, "T"), &s(env, "D"), &3, &86_400)
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
    client.vote(&v, &id, &VoteChoice::Yes); // only 1, quorum=3
    env.ledger().set_timestamp(env.ledger().timestamp() + 86_401);
    let status = client.finalize(&id);
    assert_eq!(status, ProposalStatus::Rejected);
}

#[test]
fn has_voted_returns_false_before_voting() {
    let (env, client, admin) = setup();
    let voter = Address::generate(&env);
    let id    = create(&env, &client, &admin);
    assert!(!client.has_voted(&id, &voter));
    client.vote(&voter, &id, &VoteChoice::Yes);
    assert!(client.has_voted(&id, &voter));
}
