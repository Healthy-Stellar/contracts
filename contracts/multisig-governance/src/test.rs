#![cfg(test)]

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Bytes, Env, Vec,
};

fn make_signers(env: &Env, n: u32) -> Vec<Address> {
    let mut v = Vec::new(env);
    for _ in 0..n {
        v.push_back(Address::generate(env));
    }
    v
}

/// quorum_min = 2 by default (out of 3 signers).
fn setup(n: u32, threshold: u32) -> (Env, Vec<Address>, MultisigGovernanceClient<'static>) {
    setup_with_quorum(n, threshold, 2)
}

fn setup_with_quorum(
    n: u32,
    threshold: u32,
    quorum_min: u32,
) -> (Env, Vec<Address>, MultisigGovernanceClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MultisigGovernance, ());
    let client = MultisigGovernanceClient::new(&env, &contract_id);
    let signers = make_signers(&env, n);
    client.initialize(&signers, &threshold, &3600u64, &quorum_min);
    (env, signers, client)
}

fn payload(env: &Env) -> Bytes {
    Bytes::from_slice(env, b"export_all_records")
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn test_double_initialize_returns_error() {
    let (_env, signers, client) = setup(3, 2);
    let err = client
        .try_initialize(&signers, &2u32, &3600u64, &2u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyInitialized);
}

#[test]
fn test_front_run_initialize_prevented() {
    // With mock_all_auths, any address can authenticate, so the legitimate
    // deployer's initialize succeeds and locks out subsequent attempts.
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MultisigGovernance, ());
    let client = MultisigGovernanceClient::new(&env, &contract_id);

    let deployer = Address::generate(&env);

    // Legitimate deployer initializes with their own signer set
    let mut real_signers = Vec::new(&env);
    real_signers.push_back(deployer.clone());
    client.initialize(&real_signers, &1u32, &3600u64, &1u32);

    // An attacker (with different signers) tries to re-initialize — should fail
    let attacker = Address::generate(&env);
    let mut attacker_signers = Vec::new(&env);
    attacker_signers.push_back(attacker.clone());
    let err = client
        .try_initialize(&attacker_signers, &1u32, &3600u64, &1u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyInitialized);

    // Verify that the deployer's signer set is intact, not the attacker's
    let stored_signers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::Signers)
        .unwrap();
    assert_eq!(stored_signers.len(), 1);
    assert_eq!(stored_signers.get(0).unwrap(), deployer);
}

#[test]
fn test_invalid_threshold_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MultisigGovernance, ());
    let client = MultisigGovernanceClient::new(&env, &contract_id);
    let signers = make_signers(&env, 2);
    let err = client
        .try_initialize(&signers, &3u32, &3600u64, &2u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidThreshold);
}

#[test]
fn test_propose_before_init_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MultisigGovernance, ());
    let client = MultisigGovernanceClient::new(&env, &contract_id);
    let signer = Address::generate(&env);
    let err = client
        .try_propose_multisig_action(&signer, &symbol_short!("export"), &payload(&env))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotInitialized);
}

// ── propose ───────────────────────────────────────────────────────────────────

#[test]
fn test_non_signer_propose_returns_error() {
    let (env, _signers, client) = setup(3, 2);
    let stranger = Address::generate(&env);
    let err = client
        .try_propose_multisig_action(&stranger, &symbol_short!("export"), &payload(&env))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotASigner);
}

#[test]
fn test_duplicate_proposal_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    let err = client
        .try_propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ProposalExists);
}

// ── domain tag ────────────────────────────────────────────────────────────────

#[test]
fn test_domain_tags_differ_per_action() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.propose_multisig_action(&s0, &symbol_short!("import"), &payload(&env));
    let p1 = client.get_proposal(&symbol_short!("export"));
    let p2 = client.get_proposal(&symbol_short!("import"));
    assert_ne!(p1.domain_tag, p2.domain_tag, "domain tags must differ per action");
}

// ── eligible signer snapshot ──────────────────────────────────────────────────

#[test]
fn test_eligible_signers_snapshot_stored() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.eligible_signers.len(), 3);
}

// ── approve: under-threshold ──────────────────────────────────────────────────

#[test]
fn test_under_threshold_stays_pending() {
    let (env, signers, client) = setup_with_quorum(3, 3, 1);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("export"));
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(proposal.approvals.len(), 2);
}

// ── approve: at-threshold with quorum ────────────────────────────────────────

#[test]
fn test_at_threshold_with_quorum_executes() {
    let (env, signers, client) = setup_with_quorum(3, 2, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("upgrade"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("upgrade"));
    let proposal = client.get_proposal(&symbol_short!("upgrade"));
    assert_eq!(proposal.status, ProposalStatus::Executed);
}

// ── quorum not met ────────────────────────────────────────────────────────────

#[test]
fn test_threshold_met_but_quorum_not_met_returns_error() {
    // 3 signers, threshold=1, quorum_min=3.
    // Proposer's single vote meets threshold but not quorum.
    // A second approver triggers try_finalize which returns QuorumNotMet.
    let (env, signers, client) = setup_with_quorum(3, 1, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    // s0 already has 1 approval (threshold=1 met), but quorum_min=3 not met.
    // Approving with s1 triggers finalization → QuorumNotMet.
    let err = client
        .try_approve_multisig_action(&s1, &symbol_short!("export"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::QuorumNotMet);

    // The QuorumNotMet error must not erase s1's already-cast approval.
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert!(proposal.approvals.contains(&s1));
    assert_eq!(proposal.approvals.len(), 2);
}

// ── abstention ────────────────────────────────────────────────────────────────

#[test]
fn test_abstention_counts_toward_quorum_not_threshold() {
    // 3 signers, threshold=2, quorum_min=2.
    // s0 proposes (1 approval), s1 abstains → participation=2 (quorum met),
    // but approvals=1 (threshold not met) → stays Pending.
    let (env, signers, client) = setup_with_quorum(3, 2, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.abstain_multisig_action(&s1, &symbol_short!("export"));
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.status, ProposalStatus::Pending);
    assert_eq!(proposal.approvals.len(), 1);
    assert_eq!(proposal.abstentions.len(), 1);
}

#[test]
fn test_abstention_plus_approval_reaches_quorum_and_threshold() {
    // 3 signers, threshold=2, quorum_min=2.
    // s0 proposes (1 approval), s1 abstains, s2 approves → 2 approvals, 1 abstention.
    let (env, signers, client) = setup_with_quorum(3, 2, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.abstain_multisig_action(&s1, &symbol_short!("export"));
    client.approve_multisig_action(&s2, &symbol_short!("export"));
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.status, ProposalStatus::Executed);
}

#[test]
fn test_duplicate_abstention_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.abstain_multisig_action(&s1, &symbol_short!("export"));
    let err = client
        .try_abstain_multisig_action(&s1, &symbol_short!("export"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyVoted);
}

// ── finalization: all voted, threshold not met ────────────────────────────────

#[test]
fn test_all_voted_without_threshold_marks_failed() {
    // 3 signers, threshold=3, quorum_min=1.
    // s0 proposes (1 approval), s1 abstains, s2 abstains → all 3 participated,
    // threshold=3 not met → Failed.
    let (env, signers, client) = setup_with_quorum(3, 3, 1);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.abstain_multisig_action(&s1, &symbol_short!("export"));
    client.abstain_multisig_action(&s2, &symbol_short!("export"));
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.status, ProposalStatus::Failed);
}

// ── finalize_expired ──────────────────────────────────────────────────────────

#[test]
fn test_finalize_expired_marks_failed() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    env.ledger().with_mut(|li| { li.timestamp += 3601; });
    client.finalize_expired(&symbol_short!("export"));
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.status, ProposalStatus::Failed);
}

#[test]
fn test_finalize_not_yet_expired_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    let err = client
        .try_finalize_expired(&symbol_short!("export"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Expired);
}

// ── approve: over-threshold ───────────────────────────────────────────────────

#[test]
fn test_approve_after_execution_returns_error() {
    let (env, signers, client) = setup_with_quorum(3, 2, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("upgrade"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("upgrade"));
    let err = client
        .try_approve_multisig_action(&s2, &symbol_short!("upgrade"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyExecuted);
}

// ── duplicate approval ────────────────────────────────────────────────────────

#[test]
fn test_duplicate_approval_returns_error() {
    let (env, signers, client) = setup_with_quorum(3, 3, 1);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("export"));
    let err = client
        .try_approve_multisig_action(&s1, &symbol_short!("export"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyVoted);
}

// ── non-signer approval ───────────────────────────────────────────────────────

#[test]
fn test_non_signer_approve_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let stranger = Address::generate(&env);
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    let err = client
        .try_approve_multisig_action(&stranger, &symbol_short!("export"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotASigner);
}

// ── TTL expiry ────────────────────────────────────────────────────────────────

#[test]
fn test_expired_proposal_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    env.ledger().with_mut(|li| { li.timestamp += 3601; });
    let err = client
        .try_approve_multisig_action(&s1, &symbol_short!("export"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Expired);
}

// ── signer lifecycle: add ─────────────────────────────────────────────────────

#[test]
fn test_propose_add_signer_non_signer_returns_error() {
    let (env, _signers, client) = setup(3, 2);
    let stranger = Address::generate(&env);
    let new_signer = Address::generate(&env);
    let err = client
        .try_propose_signer_change(&stranger, &SignerChangeKind::Add, &new_signer)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotASigner);
}

#[test]
fn test_propose_add_existing_signer_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let err = client
        .try_propose_signer_change(&s0, &SignerChangeKind::Add, &s1)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadySigner);
}

#[test]
fn test_add_signer_executes_at_threshold() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let new_signer = Address::generate(&env);

    client.propose_signer_change(&s0, &SignerChangeKind::Add, &new_signer);
    // s0 already approved; s1 approval reaches threshold=2 → executes.
    client.approve_signer_change(&s1);

    // New signer can now propose actions.
    client.propose_multisig_action(&new_signer, &symbol_short!("test"), &payload(&env));
    let proposal = client.get_proposal(&symbol_short!("test"));
    assert_eq!(proposal.approvals.len(), 1);
}

#[test]
fn test_add_signer_under_threshold_stays_pending() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let new_signer = Address::generate(&env);

    client.propose_signer_change(&s0, &SignerChangeKind::Add, &new_signer);
    let sp = client.get_signer_proposal();
    assert_eq!(sp.status, ProposalStatus::Pending);
    assert_eq!(sp.approvals.len(), 1);
}

#[test]
fn test_duplicate_signer_proposal_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let new_signer = Address::generate(&env);
    let new_signer2 = Address::generate(&env);

    client.propose_signer_change(&s0, &SignerChangeKind::Add, &new_signer);
    let err = client
        .try_propose_signer_change(&s0, &SignerChangeKind::Add, &new_signer2)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ProposalExists);
}

#[test]
fn test_duplicate_approve_signer_change_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let new_signer = Address::generate(&env);

    client.propose_signer_change(&s0, &SignerChangeKind::Add, &new_signer);
    client.approve_signer_change(&s1);
    let err = client
        .try_approve_signer_change(&s1)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::AlreadyVoted);
}

// ── signer lifecycle: remove ──────────────────────────────────────────────────

#[test]
fn test_remove_signer_executes_at_threshold() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();

    client.propose_signer_change(&s0, &SignerChangeKind::Remove, &s2);
    client.approve_signer_change(&s1);

    // s2 should no longer be a signer.
    let err = client
        .try_propose_multisig_action(&s2, &symbol_short!("test"), &payload(&env))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotASigner);
}

#[test]
fn test_remove_signer_threshold_breach_returns_error() {
    // 3 signers, threshold=3 → removing any signer would leave 2 < threshold.
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let err = client
        .try_propose_signer_change(&s0, &SignerChangeKind::Remove, &s1)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ThresholdBreached);
}

#[test]
fn test_remove_signer_quorum_breach_returns_error() {
    // 5 signers, threshold=3, quorum_min=5 → removing any signer would leave
    // only 4 signers, making quorum_min=5 permanently unreachable.
    let (_env, signers, client) = setup_with_quorum(5, 3, 5);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let err = client
        .try_propose_signer_change(&s0, &SignerChangeKind::Remove, &s1)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::QuorumBreached);
}

#[test]
fn test_remove_non_signer_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let stranger = Address::generate(&env);
    let err = client
        .try_propose_signer_change(&s0, &SignerChangeKind::Remove, &stranger)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotASigner);
}

#[test]
fn test_approve_signer_change_expired_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let new_signer = Address::generate(&env);

    client.propose_signer_change(&s0, &SignerChangeKind::Add, &new_signer);
    env.ledger().with_mut(|li| { li.timestamp += 3601; });
    let err = client
        .try_approve_signer_change(&s1)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Expired);
}

// ── Key rotation attack hardening (#413) ─────────────────────────────────────

#[test]
fn test_new_signer_cannot_approve_old_proposal() {
    // s0 proposes, then s3 is added. s3 should NOT be able to vote on the
    // old proposal because they were not in the eligible_signers snapshot.
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let new_signer = Address::generate(&env);

    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));

    // Add new signer via signer-change proposal.
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();
    client.propose_signer_change(&s0, &SignerChangeKind::Add, &new_signer);
    client.approve_signer_change(&s1);
    client.approve_signer_change(&s2);
    // new_signer is now a current signer.

    // new_signer tries to approve the OLD proposal — they were not in eligible_signers.
    let err = client
        .try_approve_multisig_action(&new_signer, &symbol_short!("export"))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotASigner);
}

#[test]
fn test_removed_signer_votes_invalidated() {
    // s0 proposes, s0 and s1 approve. Then s1 is removed via signer-change.
    // s1's prior approval should be stripped from the in-flight proposal.
    let (env, signers, client) = setup_with_quorum(4, 3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();
    let s3 = signers.get(3).unwrap();

    // Create a proposal that needs 3 approvals.
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("export"));

    // At this point approvals = [s0, s1]. Remove s1.
    client.propose_signer_change(&s0, &SignerChangeKind::Remove, &s1);
    client.approve_signer_change(&s2);
    client.approve_signer_change(&s3);
    // s1 is now removed; their vote should have been invalidated.

    // Proposal should still be Pending (s0's vote remains, s1's was stripped).
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.status, ProposalStatus::Pending);
    // Only s0's approval should remain.
    assert_eq!(proposal.approvals.len(), 1);
}

#[test]
fn test_signer_snapshot_stored_at_proposal_time() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    let proposal = client.get_proposal(&symbol_short!("export"));
    // Snapshot must match the 3 initial signers.
    assert_eq!(proposal.eligible_signers.len(), 3);
}

// ── #636 regression: re-propose after expiry ─────────────────────────────────

/// Propose an action, let the TTL expire, then re-propose the *same* action_id
/// without calling cleanup_expired_proposals first.  The re-proposal must
/// succeed (not return ProposalExists).
#[test]
fn test_repropose_same_action_id_after_ttl_expiry() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();

    // First proposal.
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));

    // Advance time past the TTL (3600 s) so the proposal is expired.
    env.ledger().with_mut(|li| {
        li.timestamp += 3601;
    });

    // Re-propose the same action_id without any cleanup call — must succeed.
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));

    // The new proposal is fresh and Pending.
    let proposal = client.get_proposal(&symbol_short!("export"));
    assert_eq!(proposal.status, ProposalStatus::Pending);
    // proposed_at must match the current (advanced) ledger timestamp.
    assert_eq!(proposal.proposed_at, env.ledger().timestamp());
}

/// A proposal that is still within its TTL window must still block re-proposal.
#[test]
fn test_repropose_while_active_still_blocked() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();

    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));

    // TTL has NOT elapsed — re-proposal must still fail.
    let err = client
        .try_propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::ProposalExists);
}

/// A finalized (Executed) proposal must not block re-proposal of the same
/// action_id, even within the original TTL window.
#[test]
fn test_repropose_after_execution_succeeds() {
    // threshold=2, quorum_min=2 → s0 propose + s1 approve = Executed.
    let (env, signers, client) = setup_with_quorum(3, 2, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();

    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.approve_multisig_action(&s1, &symbol_short!("export"));
    let p = client.get_proposal(&symbol_short!("export"));
    assert_eq!(p.status, ProposalStatus::Executed);

    // Re-proposing an Executed action_id must succeed.
    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    let p2 = client.get_proposal(&symbol_short!("export"));
    assert_eq!(p2.status, ProposalStatus::Pending);
}

#[test]
fn test_abstain_counts_toward_quorum() {
    let (env, signers, client) = setup_with_quorum(3, 2, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();

    client.propose_multisig_action(&s0, &symbol_short!("export"), &payload(&env));
    client.abstain_multisig_action(&s1, &symbol_short!("export"));
    client.abstain_multisig_action(&s2, &symbol_short!("export"));

    let proposal = client.get_proposal(&symbol_short!("export"));
    // All 3 voted (1 approval, 2 abstentions) → quorum met, threshold not met → Failed.
    assert_eq!(proposal.status, ProposalStatus::Failed);
}

// ── #783: Domain tag is off-chain-only replay protection ──────────────────────

#[test]
fn test_domain_tag_computed_from_action_id() {
    let (env, signers, client) = setup_with_quorum(2, 1, 2);
    let s0 = signers.get(0).unwrap();

    let action_id = symbol_short!("test_act");
    client.propose_multisig_action(&s0, &action_id, &payload(&env));

    let proposal = client.get_proposal(&action_id);

    // Verify domain_tag is derived from action_id (deterministic)
    // Independently compute what the domain_tag should be
    let expected_tag: BytesN<32> = {
        let data: Bytes = action_id.clone().to_xdr(&env);
        env.crypto().sha256(&data).into()
    };

    assert_eq!(proposal.domain_tag, expected_tag);
}

#[test]
fn test_domain_tag_is_not_verified_on_chain() {
    let (env, signers, client) = setup_with_quorum(2, 1, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();

    let action_id = symbol_short!("test_act");
    client.propose_multisig_action(&s0, &action_id, &payload(&env));

    let proposal = client.get_proposal(&action_id);
    let stored_domain_tag = proposal.domain_tag.clone();

    // Verify that approvals work regardless of domain_tag.
    // If domain_tag was verified on-chain, a caller couldn't control it.
    // Since domain_tag is off-chain-only and not a parameter, approval succeeds without any domain_tag input.
    let result = client.try_approve_multisig_action(&s1, &action_id);
    assert!(
        result.is_ok(),
        "Approval should succeed; domain_tag is off-chain-only (not verified on-chain)"
    );

    let updated = client.get_proposal(&action_id);
    // Verify domain_tag remains unchanged and matches what was stored
    assert_eq!(updated.domain_tag, stored_domain_tag);
    // Verify approval was recorded
    assert_eq!(updated.approvals.len(), 2);
}
