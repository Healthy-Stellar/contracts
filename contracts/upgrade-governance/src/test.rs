#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Bytes, BytesN, Env, Vec,
};

fn make_signers(env: &Env, n: u32) -> Vec<Address> {
    let mut v = Vec::new(env);
    for _ in 0..n {
        v.push_back(Address::generate(env));
    }
    v
}

fn dummy_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[1u8; 32])
}

fn release_metadata(env: &Env) -> ReleaseMetadata {
    ReleaseMetadata {
        version: Bytes::from_slice(env, b"v1.2.3"),
        audit_digest: BytesN::from_array(env, &[2u8; 32]),
        build_manifest: BytesN::from_array(env, &[3u8; 32]),
    }
}

fn metadata_hash(env: &Env, metadata: &ReleaseMetadata) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.append(&Bytes::from_slice(env, b"release-metadata-v1"));
    data.append(&metadata.version);
    data.append(&metadata.audit_digest.clone().into());
    data.append(&metadata.build_manifest.clone().into());
    env.crypto().sha256(&data).into()
}

fn setup(n: u32, threshold: u32) -> (Env, Vec<Address>, UpgradeGovernanceClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(UpgradeGovernance, ());
    let client = UpgradeGovernanceClient::new(&env, &contract_id);
    let signers = make_signers(&env, n);
    client.initialize(&signers, &threshold);
    (env, signers, client)
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn test_double_initialize_returns_error() {
    let (_env, signers, client) = setup(3, 2);
    let err = client.try_initialize(&signers, &2u32).unwrap_err().unwrap();
    assert_eq!(err, Error::AlreadyInitialized);
}

#[test]
fn test_invalid_threshold_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(UpgradeGovernance, ());
    let client = UpgradeGovernanceClient::new(&env, &contract_id);
    let signers = make_signers(&env, 2);
    let err = client.try_initialize(&signers, &3u32).unwrap_err().unwrap();
    assert_eq!(err, Error::InvalidThreshold);
}

#[test]
fn test_propose_before_init_returns_error() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(UpgradeGovernance, ());
    let client = UpgradeGovernanceClient::new(&env, &contract_id);
    let signer = Address::generate(&env);
    let err = client
        .try_propose_upgrade(
            &signer,
            &dummy_hash(&env),
            &release_metadata(&env),
            &metadata_hash(&env, &release_metadata(&env)),
            &0u32,
        )
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
        .try_propose_upgrade(
            &stranger,
            &dummy_hash(&env),
            &release_metadata(&env),
            &metadata_hash(&env, &release_metadata(&env)),
            &0u32,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotASigner);
}

#[test]
fn test_propose_returns_incrementing_ids() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id0 = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    let id1 = client.propose_upgrade(
        &s0,
        &BytesN::from_array(&env, &[2u8; 32]),
        &metadata,
        &metadata_hash,
        &0u32,
    );
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
}

#[test]
fn test_propose_with_invalid_release_metadata_hash_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let metadata = release_metadata(&env);
    let wrong_hash = BytesN::from_array(&env, &[9u8; 32]);
    let err = client
        .try_propose_upgrade(&s0, &dummy_hash(&env), &metadata, &wrong_hash, &0u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::InvalidReleaseMetadata);
}

// ── domain tag ────────────────────────────────────────────────────────────────

#[test]
fn test_domain_tags_differ_per_proposal() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id0 = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    let id1 = client.propose_upgrade(
        &s0,
        &BytesN::from_array(&env, &[2u8; 32]),
        &metadata,
        &metadata_hash,
        &0u32,
    );
    let p0 = client.get_proposal(&id0);
    let p1 = client.get_proposal(&id1);
    assert_ne!(p0.domain_tag, p1.domain_tag, "domain tags must differ per proposal");
}

// ── vote ──────────────────────────────────────────────────────────────────────

#[test]
fn test_non_signer_vote_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let stranger = Address::generate(&env);
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    let err = client.try_vote_upgrade(&stranger, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::NotASigner);
}

#[test]
fn test_duplicate_vote_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.vote_upgrade(&s1, &id);
    let err = client.try_vote_upgrade(&s1, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::AlreadyVoted);
}

#[test]
fn test_vote_after_expiry_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    env.ledger().with_mut(|li| { li.timestamp += VOTING_WINDOW + 1; });
    let err = client.try_vote_upgrade(&s1, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::Expired);
}

// ── timelock ──────────────────────────────────────────────────────────────────

#[test]
fn test_execute_before_timelock_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.vote_upgrade(&s1, &id);
    // Threshold reached but timelock not elapsed.
    let err = client.try_execute_upgrade(&s0, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::TimelockActive);
}

#[test]
fn test_execute_without_approved_metadata_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.vote_upgrade(&s1, &id);
    env.ledger().with_mut(|li| {
        li.timestamp += TIMELOCK_DELAY + 1;
    });
    let err = client.try_execute_upgrade(&s0, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::UnapprovedArtifactMetadata);
}

#[test]
fn test_execute_after_timelock_passes_governance_checks() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.vote_upgrade(&s1, &id);
    client.approve_artifact_metadata(&s0, &metadata_hash);
    // Advance past the timelock.
    env.ledger().with_mut(|li| { li.timestamp += TIMELOCK_DELAY + 1; });
    // Governance checks pass; deployer panics on dummy hash — that's expected.
    // We verify the error is NOT a governance error.
    let result = client.try_execute_upgrade(&s0, &id);
    match result {
        Err(Ok(e)) => {
            assert!(
                e != Error::TimelockActive
                    && e != Error::ThresholdNotMet
                    && e != Error::Expired
                    && e != Error::AlreadyExecuted,
                "unexpected governance error: {e:?}"
            );
        }
        // Panics from the deployer are also acceptable.
        _ => {}
    }
}

// ── cancellation ──────────────────────────────────────────────────────────────

#[test]
fn test_cancel_active_proposal() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.cancel_upgrade(&s1, &id);
    let proposal = client.get_proposal(&id);
    assert_eq!(proposal.status, ProposalStatus::Cancelled);
}

#[test]
fn test_cancel_approved_proposal_during_timelock() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.vote_upgrade(&s1, &id);
    // Proposal is now Approved; cancel during timelock window.
    client.cancel_upgrade(&s2, &id);
    let proposal = client.get_proposal(&id);
    assert_eq!(proposal.status, ProposalStatus::Cancelled);
}

#[test]
fn test_vote_on_cancelled_proposal_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.cancel_upgrade(&s1, &id);
    let err = client.try_vote_upgrade(&s2, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::Cancelled);
}

#[test]
fn test_execute_cancelled_proposal_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.vote_upgrade(&s1, &id);
    client.cancel_upgrade(&s0, &id);
    env.ledger().with_mut(|li| { li.timestamp += TIMELOCK_DELAY + 1; });
    let err = client.try_execute_upgrade(&s0, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::Cancelled);
}

// ── execute: under threshold ──────────────────────────────────────────────────

#[test]
fn test_execute_under_threshold_returns_error() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    env.ledger().with_mut(|li| { li.timestamp += TIMELOCK_DELAY + 1; });
    let err = client.try_execute_upgrade(&s0, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::ThresholdNotMet);
}

// ── execute: expired ─────────────────────────────────────────────────────────

#[test]
fn test_execute_expired_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.vote_upgrade(&s1, &id);
    env.ledger().with_mut(|li| { li.timestamp += VOTING_WINDOW + 1; });
    let err = client.try_execute_upgrade(&s0, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::Expired);
}

// ── execute: already executed ─────────────────────────────────────────────────

#[test]
fn test_vote_on_executed_proposal_returns_error() {
    let (env, signers, client) = setup(3, 2);
    let s0 = signers.get(0).unwrap();
    let s1 = signers.get(1).unwrap();
    let s2 = signers.get(2).unwrap();
    let metadata = release_metadata(&env);
    let metadata_hash = metadata_hash(&env, &metadata);
    let id = client.propose_upgrade(&s0, &dummy_hash(&env), &metadata, &metadata_hash, &0u32);
    client.vote_upgrade(&s1, &id);

    let mut proposal: UpgradeProposal = client.get_proposal(&id);
    proposal.status = ProposalStatus::Executed;
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(id), &proposal);
    });

    let err = client.try_vote_upgrade(&s2, &id).unwrap_err().unwrap();
    assert_eq!(err, Error::AlreadyExecuted);
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
    client.approve_signer_change(&s1);

    // New signer can now propose upgrades.
    let id = client.propose_upgrade(&new_signer, &dummy_hash(&env), &release_metadata(&env), &metadata_hash(&env, &release_metadata(&env)), &0u32);
    let proposal = client.get_proposal(&id);
    assert_eq!(proposal.votes.len(), 1);
}

#[test]
fn test_add_signer_under_threshold_stays_pending() {
    let (env, signers, client) = setup(3, 3);
    let s0 = signers.get(0).unwrap();
    let new_signer = Address::generate(&env);

    client.propose_signer_change(&s0, &SignerChangeKind::Add, &new_signer);
    let sp = client.get_signer_proposal();
    assert_eq!(sp.status, SignerProposalStatus::Pending);
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
        .try_propose_upgrade(&s2, &dummy_hash(&env), &release_metadata(&env), &metadata_hash(&env, &release_metadata(&env)), &0u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::NotASigner);
}

#[test]
fn test_remove_signer_threshold_breach_returns_error() {
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
    env.ledger().with_mut(|li| { li.timestamp += VOTING_WINDOW + 1; });
    let err = client
        .try_approve_signer_change(&s1)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, Error::Expired);
}
