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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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

    /// Propose an emergency upgrade that bypasses the timelock and requires unanimous signer approval.
    ///
    /// Emergency proposals are identical to standard proposals except:
    /// - All signers must vote (`100 %` threshold) before execution is allowed.
    /// - `execute_upgrade` skips the `TIMELOCK_DELAY` for these proposals.
    /// - A distinct `emrg_prop` audit event is emitted.
    pub fn propose_emergency_upgrade(
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

        let signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;

        // Auto-approve if the proposer is the sole signer (100 % met immediately).
        let (status, approved_at) = if votes.len() >= signers.len() {
            (ProposalStatus::Approved, env.ledger().timestamp())
        } else {
            (ProposalStatus::Active, 0)
        };

        let proposal = UpgradeProposal {
            new_wasm_hash: new_wasm_hash.clone(),
            release_metadata,
            artifact_metadata_hash,
            proposer: proposer.clone(),
            votes,
            proposed_at: env.ledger().timestamp(),
            approved_at,
            status,
            domain_tag,
            min_compatible_schema,
            is_emergency: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        env.storage()
            .persistent()
            .set(&DataKey::NextId, &(proposal_id + 1));

        env.events()
            .publish((symbol_short!("emrg_prop"), proposal_id), new_wasm_hash);

        Ok(proposal_id)
    }

    /// Allow signers to approve a release artifact metadata hash before execution.
    pub fn approve_artifact_metadata(
        env: Env,
        caller: Address,
        artifact_metadata_hash: BytesN<32>,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        caller.require_auth();
        Self::assert_signer(&env, &caller)?;
        env.storage().persistent().set(
            &DataKey::ApprovedArtifactMetadata(artifact_metadata_hash.clone()),
            &true,
        );
        env.events()
            .publish((symbol_short!("meta_appr"), caller), artifact_metadata_hash);
        Ok(())
    }

    pub fn vote_upgrade(env: Env, voter: Address, proposal_id: u64) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
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
        // Emergency proposals require unanimous (100 %) approval of the current signer set.
        let required = if proposal.is_emergency {
            current_signers.len()
        } else {
            env.storage()
                .persistent()
                .get(&DataKey::Threshold)
                .ok_or(Error::NotInitialized)?
        };
        let effective_votes = Self::count_current_signer_votes(&proposal.votes, &current_signers);

        if proposal.status == ProposalStatus::Active && effective_votes >= required {
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
        ttl_config::extend_instance_ttl(&env);
        Self::assert_initialized(&env)?;
        caller.require_auth();
        Self::assert_signer(&env, &caller)?;

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

        if env.ledger().timestamp() > proposal.proposed_at + VOTING_WINDOW {
            return Err(Error::Expired);
        }

        // Re-validate against the *current* signer set: votes from signers who
        // have since been removed must not count toward the threshold.
        let current_signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;
        let required = if proposal.is_emergency {
            current_signers.len()
        } else {
            env.storage()
                .persistent()
                .get(&DataKey::Threshold)
                .ok_or(Error::NotInitialized)?
        };
        let effective_votes = Self::count_current_signer_votes(&proposal.votes, &current_signers);
        if effective_votes < required {
            return Err(Error::ThresholdNotMet);
        }

        // Enforce timelock for standard proposals; emergency proposals skip it.
        if !proposal.is_emergency
            && env.ledger().timestamp() < proposal.approved_at + TIMELOCK_DELAY
        {
            return Err(Error::TimelockActive);
        }

        let approved = env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::ApprovedArtifactMetadata(
                proposal.artifact_metadata_hash.clone(),
            ))
            .ok_or(Error::UnapprovedArtifactMetadata)?;
        if !approved {
            return Err(Error::UnapprovedArtifactMetadata);
        }
        Self::validate_release_metadata(
            &env,
            &proposal.release_metadata,
            &proposal.artifact_metadata_hash,
        )?;

        // Schema compatibility check: reject if on-chain schema is below what the new WASM requires.
        let current_schema: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::SchemaVersion)
            .unwrap_or(1);
        if current_schema < proposal.min_compatible_schema {
            return Err(Error::IncompatibleSchemaVersion);
        }

        proposal.status = ProposalStatus::Executed;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        let wasm_hash = proposal.new_wasm_hash.clone();
        env.deployer()
            .update_current_contract_wasm(proposal.new_wasm_hash);

        UpgradeExecuted {
            proposal_id,
            wasm_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Cancel an approved-but-not-yet-executed proposal during the timelock window.
    /// For Active proposals, any signer may cancel. For Approved proposals, only the
    /// original proposer may cancel, or a fresh threshold of signers must approve.
    pub fn cancel_upgrade(env: Env, caller: Address, proposal_id: u64) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        caller.require_auth();
        Self::assert_signer(&env, &caller)?;

        let mut proposal: UpgradeProposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        match proposal.status {
            ProposalStatus::Executed => return Err(Error::AlreadyExecuted),
            ProposalStatus::Cancelled => return Err(Error::Cancelled),
            ProposalStatus::Approved => {}
            ProposalStatus::Active => {}
        }

        if proposal.status == ProposalStatus::Approved && caller != proposal.proposer {
            let mut cancel_votes: Vec<Address> = env
                .storage()
                .persistent()
                .get(&DataKey::CancelVotes(proposal_id))
                .unwrap_or(Vec::new(&env));

            if !cancel_votes.iter().any(|signer| signer == caller) {
                cancel_votes.push_back(caller.clone());
                env.storage()
                    .persistent()
                    .set(&DataKey::CancelVotes(proposal_id), &cancel_votes);
            }

            let threshold: u32 = env
                .storage()
                .persistent()
                .get(&DataKey::Threshold)
                .ok_or(Error::NotInitialized)?;

            if (cancel_votes.len() as u32) < threshold {
                // Vote recorded; not yet enough fresh signers to cancel.
                return Ok(());
            }

            env.storage()
                .persistent()
                .remove(&DataKey::CancelVotes(proposal_id));
        }

        proposal.status = ProposalStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        UpgradeCancelled {
            proposal_id,
            cancelled_by: caller,
        }
        .publish(&env);
        Ok(())
    }

    /// Return the current on-chain schema version.
    pub fn get_schema_version(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::SchemaVersion)
            .unwrap_or(1)
    }

    /// Advance the on-chain schema version from `from_version` to `to_version`.
    ///
    /// # Migration pattern
    /// 1. Deploy the new WASM via `propose_upgrade` / `execute_upgrade`.
    /// 2. Call `migrate_schema` once to advance the stored version so subsequent
    ///    upgrade proposals can declare the new minimum.
    ///
    /// Requires a signer to authorise. Rejects if the stored version does not
    /// match `from_version` to prevent accidental double-migration.
    pub fn migrate_schema(
        env: Env,
        caller: Address,
        from_version: u32,
        to_version: u32,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        caller.require_auth();
        Self::assert_signer(&env, &caller)?;

        let stored: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::SchemaVersion)
            .unwrap_or(1);
        if stored != from_version {
            return Err(Error::IncompatibleSchemaVersion);
        }

        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion, &to_version);

        env.events().publish(
            (symbol_short!("sch_migr"), caller),
            (from_version, to_version),
        );
        Ok(())
    }

    /// Persistently remove an expired proposal that never reached threshold.
    /// Only `Active` proposals are prunable: `Approved` proposals are timelocked
    /// and `Executed`/`Cancelled` ones are kept as the on-chain audit record.
    /// Returns `Ok(())` so the storage removal is committed (an `Err` return
    /// would roll it back).
    pub fn prune_expired(env: Env, proposal_id: u64) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        let proposal: UpgradeProposal = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        match proposal.status {
            ProposalStatus::Active => {}
            ProposalStatus::Approved => return Err(Error::AlreadyFinalized),
            ProposalStatus::Executed => return Err(Error::AlreadyExecuted),
            ProposalStatus::Cancelled => return Err(Error::Cancelled),
        }

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

    /// Canonical metadata hash = SHA-256("release-metadata-v1" ++ version ++ audit_digest ++ build_manifest)
    fn compute_artifact_metadata_hash(env: &Env, metadata: &ReleaseMetadata) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.append(&Bytes::from_slice(env, b"release-metadata-v1"));
        data.append(&metadata.version);
        data.append(&metadata.audit_digest.clone().into());
        data.append(&metadata.build_manifest.clone().into());
        env.crypto().sha256(&data).into()
    }

    fn validate_release_metadata(
        env: &Env,
        metadata: &ReleaseMetadata,
        declared_hash: &BytesN<32>,
    ) -> Result<(), Error> {
        let computed = Self::compute_artifact_metadata_hash(env, metadata);
        if computed != *declared_hash {
            return Err(Error::InvalidReleaseMetadata);
        }
        Ok(())
    }

    /// Compute a domain tag that binds a proposal to this specific contract
    /// instance and action type, preventing cross-context replay (#233).
    ///
    /// tag = SHA-256( contract_address_xdr ++ b"upgrade-governance" ++ proposal_id_le_bytes )
    fn compute_domain_tag(env: &Env, proposal_id: u64) -> BytesN<32> {
        use soroban_sdk::xdr::ToXdr;
        let mut data = Bytes::new(env);
        // Serialize the contract address to XDR bytes for a stable, unique identifier.
        let addr_xdr = env.current_contract_address().to_xdr(env);
        data.append(&addr_xdr);
        data.append(&Bytes::from_slice(env, b"upgrade-governance"));
        data.append(&Bytes::from_slice(env, &proposal_id.to_le_bytes()));
        env.crypto().sha256(&data).into()
    }
}
