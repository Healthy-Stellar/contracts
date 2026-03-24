#![cfg(test)]

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Bytes, Env, Vec,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_signers(env: &Env, n: u32) -> Vec<Address> {
    let mut v = Vec::new(env);
    for _ in 0..n {
        v.push_back(Address::generate(env));
    }
    v
}

/// Sets up a contract with `n` signers, the given threshold, and a 3600s TTL.
fn setup(n: u32, threshold: u32) -> (Env, Vec<Address>, MultisigGovernanceClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MultisigGovernance);
    let client = MultisigGovernanceClient::new(&env, &contract_id);
    let signers = make_signers(&env, n);
    client.initialize(&signers, &threshold, &3600u64);
    (env, signers, client)
}

fn payload(env: &Env) -> Bytes {
    Bytes::from_slice(env, b"export_all_records")
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialize() {
    let (_env, signers, client) = setup(3, 2);
    client.initialize(&signers, &2u32, &3600u64);
}

#[test]
#[should_panic(expected = "Invalid threshold")]
fn test_threshold_exceeds_signers() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, MultisigGovernance);
    let client = MultisigGovernanceClient::new(&env, &contract_id);
    let signers = make_signers(&env, 2);
    client.initialize(&signers, &3u32, &3600u64);
}

// ── propose ───────────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized: not an admin signer")]
fn test_non_signer_cannot_propose() {
    let (env, _signers, client) = setup(3, 2);
    let stranger = Address::generate(&env);
    client.propose_multisig_action(&stranger, &symbol_short!("export"), &payload(&env));
}

#[test]
#[should_panic(expected = "Proposal already exists")]
fn test_duplicate_proposal_rejected() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
}

// ── approve: under-threshold ──────────────────────────────────────────────────

#[test]
fn test_under_threshold_stays_pending() {
    // threshold = 3, only 2 approvals (proposer + 1)
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();

    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("export"));

    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(proposal.approvals.len(), 2);
}

// ── approve: at-threshold ─────────────────────────────────────────────────────

#[test]
fn test_at_threshold_executes() {
    // threshold = 2, proposer counts as first approval
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();

    client.propose_multisig_action(&s0, &symbol_short!("upgrade"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("upgrade"));

    let proposal = client.get_proposal(&symbol_short!("upgrade"));
    assert_eq!(proposal.status, ProposalStatus::Executed);
    assert_eq!(proposal.approvals.len(), 2);
}

// ── approve: over-threshold ───────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Proposal already executed")]
fn test_over_threshold_rejected_after_execution() {
    // threshold = 2; a 3rd signer tries to approve after execution
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();

    client.propose_multisig_action(&s0, &symbol_short!("upgrade"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("upgrade"));
    // proposal is now Executed — this must panic
    client.approve_multisig_action(&s2, &symbol_short!("upgrade"));
}

// ── duplicate approval ────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Already approved")]
fn test_duplicate_approval_rejected() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();

    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("export"));
    // s1 tries to approve again
    client.approve_multisig_action(&s1, &symbol_short!("export"));
}

// ── non-signer approval ───────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized: not an admin signer")]
fn test_non_signer_cannot_approve() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let stranger = Address::generate(&env);

    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.approve_multisig_action(&stranger, &symbol_short!("export"));
}

// ── TTL expiry ────────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Proposal expired")]
fn test_expired_proposal_rejected() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();

    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));

    // Advance ledger time past the 3600s TTL
    env.ledger().with_mut(|li| {
        li.timestamp += 3601;
    });

    client.approve_multisig_action(&s1, &symbol_short!("export"));
}
