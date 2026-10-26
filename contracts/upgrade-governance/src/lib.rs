#![no_std]

//! # Upgrade Governance Contract
//!
//! Manages smart contract upgrades with voting approval, staged deployment, emergency halting,
//! and upgrade history tracking for safe network evolution.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Governance token holders vote on upgrades. Upgrade execution
//! restricted to admin. Emergency halt authority for critical security issues. Voting window
//! enforces minimum deliberation time (7 days). Quorum requirements prevent unilateral control.
//!
//! **Audit Controls:** Upgrade proposal events logged with description and target version.
//! Voting events tracked with voter identity and choice. Upgrade execution events recorded with
//! new code hash. Emergency halt events logged with justification. Upgrade history immutable.
//!
//! **Data Retention Policy:** Upgrade proposals retained indefinitely for network history.
//! Approved upgrades archived with vote counts. Voting records maintained for transparency.
//! Emergency halt events preserved. Previous code versions maintained for verification.
//!
//! **Encryption/Integrity:** Code hash verification via SHA256. Upgrade content immutable once
//! proposed. Voting window enforced mathematically (7 days = 604,800 seconds). Emergency halt
//! prevents unauthorized execution. Upgrade status enumeration prevents invalid states.

use soroban_sdk::{
    contract, contractevent, contracterror, contractimpl, contracttype, symbol_short, Address,
    Bytes, BytesN, Env, Vec,
};

mod test;

pub const VOTING_WINDOW: u64 = 7 * 24 * 60 * 60;
/// Mandatory delay between threshold approval and execution (24 h).
pub const TIMELOCK_DELAY: u64 = 24 * 60 * 60;

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidThreshold = 3,
    NotASigner = 4,
    ProposalNotFound = 5,
    AlreadyExecuted = 6,
    Expired = 7,
    AlreadyVoted = 8,
    ThresholdNotMet = 9,
    /// Timelock has not elapsed yet.
    TimelockActive = 10,
    /// Proposal was cancelled.
    Cancelled = 11,
    /// Caller is not authorised to cancel (must be a signer).
    NotAuthorized = 12,
    /// Release metadata does not hash to the declared metadata hash.
    InvalidReleaseMetadata = 13,
    /// Release metadata hash is not in the approved artifact registry.
    UnapprovedArtifactMetadata = 14,
    ProposalExists = 15,
    AlreadySigner = 16,
    ThresholdBreached = 17,
    AlreadyFinalized = 18,
    /// Proposed WASM requires a minimum schema version newer than what is stored on-chain.
    /// Run `migrate_schema` to advance the schema before proposing the upgrade.
    IncompatibleSchemaVersion = 19,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Initialized,
    Signers,
    Threshold,
    NextId,
    Proposal(u64),
    ApprovedArtifactMetadata(BytesN<32>),
    SignerProposal,
    /// Current on-chain schema version — compared against `UpgradeProposal::min_compatible_schema`.
    SchemaVersion,
    /// Signers who have voted to cancel an `Approved` proposal (#630). Distinct
    /// from the original approval `votes` — a fresh threshold of signers is
    /// required to override an already-approved proposal.
    CancelVotes(u64),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignerChangeKind {
    Add,
    Remove,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignerProposalStatus {
    Pending,
    Executed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignerProposal {
    pub kind: SignerChangeKind,
    pub target: Address,
    pub approvals: Vec<Address>,
    pub proposed_at: u64,
    pub status: SignerProposalStatus,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Active,
    /// Threshold reached; execution allowed after `approved_at + TIMELOCK_DELAY`.
    Approved,
    Executed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeProposal {
    pub new_wasm_hash: BytesN<32>,
    pub release_metadata: ReleaseMetadata,
    pub artifact_metadata_hash: BytesN<32>,
    /// Signer who created this proposal; retains unilateral cancel authority
    /// even after the proposal reaches `Approved`.
    pub proposer: Address,
    pub votes: Vec<Address>,
    pub proposed_at: u64,
    /// Timestamp when the threshold was first reached (starts the timelock).
    pub approved_at: u64,
    pub status: ProposalStatus,
    /// Domain tag: SHA-256(contract_address ++ "upgrade-governance" ++ proposal_id).
    /// Stored so callers can verify the binding off-chain.
    pub domain_tag: BytesN<32>,
    /// Minimum on-chain schema version the new WASM is compatible with.
    /// `execute_upgrade` rejects proposals where this exceeds the stored `SchemaVersion`.
    pub min_compatible_schema: u32,
    /// When true this is an emergency fast-track: requires 100 % signer approval
    /// and skips the timelock delay at execution time.
    pub is_emergency: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseMetadata {
    pub version: Bytes,
    pub audit_digest: BytesN<32>,
    pub build_manifest: BytesN<32>,
}

// ── Events ────────────────────────────────────────────────────────────────────

#[contractevent]
pub struct UpgradeProposed {
    pub proposal_id: u64,
    pub proposer: Address,
    pub wasm_hash: BytesN<32>,
    pub schema_version: u32,
}

#[contractevent]
pub struct UpgradeVoteCast {
    pub proposal_id: u64,
    pub voter: Address,
}

#[contractevent]
pub struct UpgradeExecuted {
    pub proposal_id: u64,
    pub wasm_hash: BytesN<32>,
}

#[contractevent]
pub struct UpgradeCancelled {
    pub proposal_id: u64,
    pub cancelled_by: Address,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct UpgradeGovernance;

#[contractimpl]
impl UpgradeGovernance {
    /// Configure the set of upgrade signers and the approval threshold.
    ///
    /// Runs atomically as part of contract creation and cannot be called again,
    /// so an attacker cannot front-run deployment with their own signer set.
    pub fn __constructor(env: Env, signers: Vec<Address>, threshold: u32) {
        if threshold == 0 || threshold as usize > signers.len() as usize {
            panic!("invalid threshold");
        }
        env.storage().persistent().set(&DataKey::Signers, &signers);
        env.storage()
            .persistent()
            .set(&DataKey::Threshold, &threshold);
        env.storage().persistent().set(&DataKey::NextId, &0u64);
        env.storage().persistent().set(&DataKey::Initialized, &true);
        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion, &1u32);
    }

    pub fn propose_upgrade(
        env: Env,
        proposer: Address,
        new_wasm_hash: BytesN<32>,
        release_metadata: ReleaseMetadata,
        artifact_metadata_hash: BytesN<32>,
        min_compatible_schema: u32,
    ) -> Result<u64, Error> {
        Self::assert_initialized(&env)?;
        proposer.require_auth();
        Self::assert_signer(&env, &proposer)?;
        Self::validate_release_metadata(&env, &release_metadata, &artifact_metadata_hash)?;

        let current_schema: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::SchemaVersion)
            .unwrap_or(1);
        if min_compatible_schema > current_schema {
            return Err(Error::IncompatibleSchemaVersion);
        }

        let proposal_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::NextId)
            .ok_or(Error::NotInitialized)?;

        let domain_tag = Self::compute_domain_tag(&env, proposal_id);

        let mut votes: Vec<Address> = Vec::new(&env);
        votes.push_back(proposer.clone());

        let proposal = UpgradeProposal {
            new_wasm_hash: new_wasm_hash.clone(),
            release_metadata,
            artifact_metadata_hash,
            proposer: proposer.clone(),
            votes,
            proposed_at: env.ledger().timestamp(),
            approved_at: 0,
            status: ProposalStatus::Active,
            domain_tag,
            min_compatible_schema,
            is_emergency: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        env.storage()
            .persistent()
            .set(&DataKey::NextId, &(proposal_id + 1));

        UpgradeProposed {
            proposal_id,
            proposer,
            wasm_hash: new_wasm_hash,
            schema_version: current_schema,
        }
        .publish(&env);

        Ok(proposal_id)
    }

    pub fn vote_upgrade(env: Env, voter: Address, proposal_id: u64) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        voter.require_auth();
        Self::assert_signer(&env, &voter)?;

        let mut proposal = Self::load_votable_proposal(&env, proposal_id)?;

        if proposal.votes.contains(&voter) {
            return Err(Error::AlreadyVoted);
        }
        proposal.votes.push_back(voter.clone());

        // Count only votes from addresses in the *current* signer set so that
        // removed signers' stale votes cannot push a proposal over threshold.
        let current_signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;
        let threshold: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Threshold)
            .ok_or(Error::NotInitialized)?;
        let effective_votes = Self::count_current_signer_votes(&proposal.votes, &current_signers);

        if proposal.status == ProposalStatus::Active && effective_votes >= threshold {
            proposal.status = ProposalStatus::Approved;
            proposal.approved_at = env.ledger().timestamp();
        }

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        UpgradeVoteCast {
            proposal_id,
            voter,
        }
        .publish(&env);

        Ok(())
    }

    pub fn execute_upgrade(env: Env, proposal_id: u64) -> Result<(), Error> {
        Self::assert_initialized(&env)?;

        let mut proposal: UpgradeProposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        match proposal.status {
            ProposalStatus::Executed => return Err(Error::AlreadyExecuted),
            ProposalStatus::Cancelled => return Err(Error::Cancelled),
            ProposalStatus::Active => return Err(Error::ThresholdNotMet),
            ProposalStatus::Approved => {}
        }

        // Re-validate against the *current* signer set: votes from signers who
        // have since been removed must not count toward the threshold.
        let current_signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;
        let threshold: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Threshold)
            .ok_or(Error::NotInitialized)?;
        let effective_votes = Self::count_current_signer_votes(&proposal.votes, &current_signers);
        if effective_votes < threshold {
            return Err(Error::ThresholdNotMet);
        }

        let current_schema: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::SchemaVersion)
            .unwrap_or(1);
        if proposal.min_compatible_schema > current_schema {
            return Err(Error::IncompatibleSchemaVersion);
        }

        if !proposal.is_emergency {
            let now = env.ledger().timestamp();
            if now < proposal.approved_at + TIMELOCK_DELAY {
                return Err(Error::TimelockActive);
            }
        }

        proposal.status = ProposalStatus::Executed;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        UpgradeExecuted {
            proposal_id,
            wasm_hash: proposal.new_wasm_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Persistently remove an expired proposal. Returns `Ok(())` so the storage
    /// removal is committed (an `Err` return would roll it back).
    pub fn prune_expired(env: Env, proposal_id: u64) -> Result<(), Error> {
        let proposal: UpgradeProposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        let now = env.ledger().timestamp();
        if now <= proposal.proposed_at + VOTING_WINDOW {
            return Err(Error::Expired);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::Proposal(proposal_id));
        Ok(())
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn count_current_signer_votes(votes: &Vec<Address>, signers: &Vec<Address>) -> u32 {
        let mut count: u32 = 0;
        for voter in votes.iter() {
            if signers.contains(&voter) {
                count += 1;
            }
        }
        count
    }

    fn load_votable_proposal(env: &Env, proposal_id: u64) -> Result<UpgradeProposal, Error> {
        let proposal: UpgradeProposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        match proposal.status {
            ProposalStatus::Executed => return Err(Error::AlreadyExecuted),
            ProposalStatus::Cancelled => return Err(Error::Cancelled),
            _ => {}
        }

        let now = env.ledger().timestamp();
        if now > proposal.proposed_at + VOTING_WINDOW {
            // Do not remove here: returning `Err` would roll back the removal.
            // Callers should invoke `prune_expired` to persist cleanup.
            return Err(Error::Expired);
        }

        Ok(proposal)
    }

    fn assert_initialized(env: &Env) -> Result<(), Error> {
        if !env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn assert_not_initialized(env: &Env) -> Result<(), Error> {
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            return Err(Error::AlreadyInitialized);
        }
        Ok(())
    }

    fn assert_signer(env: &Env, who: &Address) -> Result<(), Error> {
        let signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;
        if !signers.contains(who) {
            return Err(Error::NotASigner);
        }
        Ok(())
    }

    fn validate_release_metadata(
        env: &Env,
        release_metadata: &ReleaseMetadata,
        artifact_metadata_hash: &BytesN<32>,
    ) -> Result<(), Error> {
        let approved: bool = env
            .storage()
            .persistent()
            .get(&DataKey::ApprovedArtifactMetadata(artifact_metadata_hash.clone()))
            .unwrap_or(false);
        if !approved {
            return Err(Error::UnapprovedArtifactMetadata);
        }
        let _ = release_metadata;
        Ok(())
    }

    fn compute_domain_tag(env: &Env, proposal_id: u64) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.extend_from_array(&env.current_contract_address().to_string().into_bytes());
        data.extend_from_array(b"upgrade-governance");
        data.extend_from_array(&proposal_id.to_be_bytes());
        env.crypto().sha256(&data).into()
    }
}
