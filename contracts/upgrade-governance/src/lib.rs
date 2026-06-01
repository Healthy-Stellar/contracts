#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    Vec,
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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseMetadata {
    pub version: Bytes,
    pub audit_digest: BytesN<32>,
    pub build_manifest: BytesN<32>,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct UpgradeGovernance;

#[contractimpl]
impl UpgradeGovernance {
    pub fn initialize(env: Env, signers: Vec<Address>, threshold: u32) -> Result<(), Error> {
        Self::assert_not_initialized(&env)?;
        if threshold == 0 || threshold as usize > signers.len() as usize {
            return Err(Error::InvalidThreshold);
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
        Ok(())
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
            votes,
            proposed_at: env.ledger().timestamp(),
            approved_at: 0,
            status: ProposalStatus::Active,
            domain_tag,
            min_compatible_schema,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        env.storage()
            .persistent()
            .set(&DataKey::NextId, &(proposal_id + 1));

        env.events()
            .publish((symbol_short!("proposed"), proposal_id), new_wasm_hash);

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
        Self::assert_initialized(&env)?;
        voter.require_auth();
        Self::assert_signer(&env, &voter)?;

        let mut proposal = Self::load_votable_proposal(&env, proposal_id)?;

        for vote in proposal.votes.iter() {
            if vote == voter {
                return Err(Error::AlreadyVoted);
            }
        }

        proposal.votes.push_back(voter.clone());

        // Check whether threshold is now reached and start the timelock.
        let threshold: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Threshold)
            .ok_or(Error::NotInitialized)?;

        if proposal.status == ProposalStatus::Active && proposal.votes.len() >= threshold {
            proposal.status = ProposalStatus::Approved;
            proposal.approved_at = env.ledger().timestamp();
            env.events().publish(
                (symbol_short!("approved"), proposal_id),
                proposal.votes.len(),
            );
        }

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((symbol_short!("voted"), proposal_id), voter);
        Ok(())
    }

    /// Execute an upgrade after the timelock has elapsed.
    pub fn execute_upgrade(env: Env, caller: Address, proposal_id: u64) -> Result<(), Error> {
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

        // Enforce timelock: must wait TIMELOCK_DELAY after approval.
        if env.ledger().timestamp() < proposal.approved_at + TIMELOCK_DELAY {
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

        proposal.status = ProposalStatus::Executed;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.deployer()
            .update_current_contract_wasm(proposal.new_wasm_hash.clone());

        env.events().publish(
            (symbol_short!("ct_upgrad"), proposal_id),
            proposal.new_wasm_hash,
        );
        Ok(())
    }

    /// Cancel an approved-but-not-yet-executed proposal during the timelock window.
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
            // Allow cancellation of both Active and Approved proposals.
            _ => {}
        }

        proposal.status = ProposalStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((symbol_short!("cancelled"), proposal_id), caller);
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

    pub fn get_proposal(env: Env, proposal_id: u64) -> Result<UpgradeProposal, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)
    }

    // ── signer lifecycle ──────────────────────────────────────────────────────

    /// Propose adding or removing a signer.  Only one signer-change proposal
    /// may be active at a time.  The proposer's approval is counted immediately.
    pub fn propose_signer_change(
        env: Env,
        proposer: Address,
        kind: SignerChangeKind,
        target: Address,
    ) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        proposer.require_auth();
        Self::assert_signer(&env, &proposer)?;

        if env.storage().persistent().has(&DataKey::SignerProposal) {
            return Err(Error::ProposalExists);
        }

        let signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;

        match kind {
            SignerChangeKind::Add => {
                for s in signers.iter() {
                    if s == target {
                        return Err(Error::AlreadySigner);
                    }
                }
            }
            SignerChangeKind::Remove => {
                let threshold: u32 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::Threshold)
                    .ok_or(Error::NotInitialized)?;
                if signers.len() <= threshold {
                    return Err(Error::ThresholdBreached);
                }
                let mut found = false;
                for s in signers.iter() {
                    if s == target {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(Error::NotASigner);
                }
            }
        }

        let mut approvals: Vec<Address> = Vec::new(&env);
        approvals.push_back(proposer.clone());

        let proposal = SignerProposal {
            kind: kind.clone(),
            target: target.clone(),
            approvals,
            proposed_at: env.ledger().timestamp(),
            status: SignerProposalStatus::Pending,
        };

        env.storage()
            .persistent()
            .set(&DataKey::SignerProposal, &proposal);
        env.events()
            .publish((symbol_short!("sg_prop"), kind), (proposer, target));
        Ok(())
    }

    /// Approve the active signer-change proposal.  Executes immediately when
    /// the approval threshold is reached.
    pub fn approve_signer_change(env: Env, signer: Address) -> Result<(), Error> {
        Self::assert_initialized(&env)?;
        signer.require_auth();
        Self::assert_signer(&env, &signer)?;

        let mut proposal: SignerProposal = env
            .storage()
            .persistent()
            .get(&DataKey::SignerProposal)
            .ok_or(Error::ProposalNotFound)?;

        if proposal.status != SignerProposalStatus::Pending {
            return Err(Error::AlreadyFinalized);
        }

        if env.ledger().timestamp() > proposal.proposed_at + VOTING_WINDOW {
            return Err(Error::Expired);
        }

        for i in 0..proposal.approvals.len() {
            if proposal.approvals.get(i).unwrap() == signer {
                return Err(Error::AlreadyVoted);
            }
        }

        proposal.approvals.push_back(signer.clone());

        let threshold: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Threshold)
            .ok_or(Error::NotInitialized)?;

        if proposal.approvals.len() >= threshold {
            let mut signers: Vec<Address> = env
                .storage()
                .persistent()
                .get(&DataKey::Signers)
                .ok_or(Error::NotInitialized)?;

            match proposal.kind {
                SignerChangeKind::Add => {
                    signers.push_back(proposal.target.clone());
                }
                SignerChangeKind::Remove => {
                    let mut new_signers: Vec<Address> = Vec::new(&env);
                    for s in signers.iter() {
                        if s != proposal.target {
                            new_signers.push_back(s);
                        }
                    }
                    signers = new_signers;
                }
            }

            env.storage().persistent().set(&DataKey::Signers, &signers);
            proposal.status = SignerProposalStatus::Executed;
            env.storage().persistent().remove(&DataKey::SignerProposal);
            env.events().publish(
                (symbol_short!("sg_exec"), proposal.kind.clone()),
                proposal.target.clone(),
            );
        } else {
            env.storage()
                .persistent()
                .set(&DataKey::SignerProposal, &proposal);
        }

        env.events()
            .publish((symbol_short!("sg_appr"), signer.clone()), signer);
        Ok(())
    }

    pub fn get_signer_proposal(env: Env) -> Result<SignerProposal, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::SignerProposal)
            .ok_or(Error::ProposalNotFound)
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Compute a domain tag that binds a proposal to this specific contract
    /// instance and action type, preventing cross-context replay (#233).
    ///
    /// tag = SHA-256( contract_address_xdr ++ b"upgrade-governance" ++ proposal_id_le_bytes )
    fn compute_domain_tag(env: &Env, proposal_id: u64) -> BytesN<32> {
        use soroban_sdk::{xdr::ToXdr, Bytes};
        let mut data = Bytes::new(env);
        // Serialize the contract address to XDR bytes for a stable, unique identifier.
        let addr_xdr = env.current_contract_address().to_xdr(env);
        data.append(&addr_xdr);
        data.append(&Bytes::from_slice(env, b"upgrade-governance"));
        data.append(&Bytes::from_slice(env, &proposal_id.to_le_bytes()));
        env.crypto().sha256(&data).into()
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

    // ── guards ────────────────────────────────────────────────────────────────

    fn assert_initialized(env: &Env) -> Result<(), Error> {
        if !env.storage().persistent().has(&DataKey::Initialized) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn assert_not_initialized(env: &Env) -> Result<(), Error> {
        if env.storage().persistent().has(&DataKey::Initialized) {
            return Err(Error::AlreadyInitialized);
        }
        Ok(())
    }

    fn assert_signer(env: &Env, caller: &Address) -> Result<(), Error> {
        let signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;
        for signer in signers.iter() {
            if signer == *caller {
                return Ok(());
            }
        }
        Err(Error::NotASigner)
    }

    /// Load a proposal that can still receive votes (Active or Approved, not expired).
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
        if env.ledger().timestamp() > proposal.proposed_at + VOTING_WINDOW {
            env.storage()
                .persistent()
                .remove(&DataKey::Proposal(proposal_id));
            return Err(Error::Expired);
        }
        Ok(proposal)
    }
}
