#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, xdr::ToXdr, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

mod test;

// ── Error types ──────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized     = 2,
    InvalidThreshold   = 3,
    NotASigner         = 4,
    ProposalExists     = 5,
    ProposalNotFound   = 6,
    AlreadyExecuted    = 7,
    Expired            = 8,
    AlreadyVoted       = 9,
    /// Quorum was not reached (too few non-abstaining votes).
    QuorumNotMet       = 10,
    /// Proposal was already finalized (executed or failed).
    AlreadyFinalized   = 11,
    /// The address is already a signer.
    AlreadySigner      = 12,
    /// Removing this signer would make the threshold unreachable.
    ThresholdBreached  = 13,
}

#[contracttype]
pub enum DataKey {
    Initialized,
    Signers,
    Threshold,
    Ttl,
    /// Minimum fraction of eligible signers that must participate (approve or
    /// abstain) for a result to be valid.  Stored as a u32 count.
    QuorumMin,
    Proposal(Symbol),
    /// Pending signer-change proposal (only one active at a time).
    SignerProposal,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Executed,
    /// Finalized as rejected: quorum reached but threshold not met, or voting
    /// window closed without enough approvals.
    Failed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub payload: Bytes,
    pub approvals: Vec<Address>,
    /// Signers who explicitly abstained (counted toward quorum, not threshold).
    pub abstentions: Vec<Address>,
    pub proposed_at: u64,
    pub status: ProposalStatus,
    /// Eligible signer set snapshotted at proposal time.
    pub eligible_signers: Vec<Address>,
    /// Domain tag for replay protection.
    pub domain_tag: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignerChangeKind {
    Add,
    Remove,
}

/// A threshold-gated proposal to add or remove a signer.
/// Only one signer-change proposal may be active at a time.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignerProposal {
    pub kind: SignerChangeKind,
    pub target: Address,
    pub approvals: Vec<Address>,
    pub proposed_at: u64,
    pub status: ProposalStatus,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MultisigGovernance;

#[contractimpl]
impl MultisigGovernance {
    /// Initialize with a set of admin signers, an approval threshold, a
    /// proposal TTL in seconds, and the minimum quorum count.
    pub fn initialize(
        env: Env,
        signers: Vec<Address>,
        threshold: u32,
        ttl_seconds: u64,
        quorum_min: u32,
    ) -> Result<(), Error> {
        if env.storage().persistent().has(&DataKey::Signers) {
            return Err(Error::AlreadyInitialized);
        }
        if threshold == 0 || threshold as usize > signers.len() as usize {
            return Err(Error::InvalidThreshold);
        }
        env.storage().persistent().set(&DataKey::Signers, &signers);
        env.storage()
            .persistent()
            .set(&DataKey::Threshold, &threshold);
        env.storage().persistent().set(&DataKey::Ttl, &ttl_seconds);
        env.storage()
            .persistent()
            .set(&DataKey::QuorumMin, &quorum_min);
        Ok(())
    }

    /// Any admin signer may open a new proposal.
    pub fn propose_multisig_action(
        env: Env,
        signer: Address,
        action_id: Symbol,
        payload: Bytes,
    ) -> Result<(), Error> {
        signer.require_auth();
        Self::assert_signer(&env, &signer)?;

        let key = DataKey::Proposal(action_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::ProposalExists);
        }

        // Snapshot the eligible signer set at proposal time (#232).
        let eligible_signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;

        let domain_tag = Self::compute_domain_tag(&env, &action_id);

        let mut approvals: Vec<Address> = Vec::new(&env);
        approvals.push_back(signer.clone());

        let proposal = Proposal {
            payload,
            approvals,
            abstentions: Vec::new(&env),
            proposed_at: env.ledger().timestamp(),
            status: ProposalStatus::Pending,
            eligible_signers,
            domain_tag,
        };

        env.storage().persistent().set(&key, &proposal);
        env.events()
            .publish((symbol_short!("proposed"), action_id), signer);
        Ok(())
    }

    /// An admin signer approves an existing proposal. Once the approval count
    /// reaches the threshold the proposal is marked Executed and an event is
    /// emitted. Expired or already-executed proposals are rejected.
    pub fn approve_multisig_action(
        env: Env,
        signer: Address,
        action_id: Symbol,
    ) -> Result<(), Error> {
        signer.require_auth();
        Self::assert_signer(&env, &signer)?;

        let key = DataKey::Proposal(action_id.clone());
        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ProposalNotFound)?;

        if proposal.status == ProposalStatus::Executed {
            return Err(Error::AlreadyExecuted);
        }

        let ttl: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Ttl)
            .ok_or(Error::NotInitialized)?;

        if env.ledger().timestamp() > proposal.proposed_at + ttl {
            return Err(Error::Expired);
        }

        // Reject duplicate approvals from the same signer.
        for i in 0..proposal.approvals.len() {
            if proposal.approvals.get(i).ok_or(Error::NotASigner)? == signer {
                return Err(Error::AlreadyVoted);
            }
        }

        proposal.approvals.push_back(signer.clone());

        Self::try_finalize(&env, &mut proposal)?;

        env.storage().persistent().set(&key, &proposal);

        env.events()
            .publish((symbol_short!("approved"), action_id), signer);
        Ok(())
    }

    pub fn get_proposal(env: Env, action_id: Symbol) -> Result<Proposal, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(action_id))
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

        // #305: Atomic compare-and-set guard.
        //
        // The previous code used a two-step has() → set() pattern.  If a
        // proposal expired between those two operations two concurrent
        // transactions could both pass the has() check and write duplicate
        // proposals.
        //
        // Fix: read the full proposal in one operation, then decide:
        //   • Pending + still within TTL  → block; return ProposalExists
        //   • Pending + TTL elapsed       → allow; overwrite the stale entry
        //   • Executed / Failed           → allow; the slot is logically free
        //   • No entry                   → allow
        if let Some(existing) = env
            .storage()
            .persistent()
            .get::<_, SignerProposal>(&DataKey::SignerProposal)
        {
            if existing.status == ProposalStatus::Pending {
                let ttl: u64 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::Ttl)
                    .ok_or(Error::NotInitialized)?;
                if env.ledger().timestamp() <= existing.proposed_at + ttl {
                    // Active proposal still within its TTL — reject.
                    return Err(Error::ProposalExists);
                }
                // Expired pending proposal — fall through and overwrite.
            }
            // Finalized (Executed / Failed) proposals do not block new ones.
        }

        let signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .ok_or(Error::NotInitialized)?;

        // Validate the change is meaningful.
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
                // After removal there must still be enough signers to meet threshold.
                if signers.len() <= threshold {
                    return Err(Error::ThresholdBreached);
                }
                // Target must be a current signer.
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
            status: ProposalStatus::Pending,
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

        if proposal.status != ProposalStatus::Pending {
            return Err(Error::AlreadyFinalized);
        }

        let ttl: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Ttl)
            .ok_or(Error::NotInitialized)?;
        if env.ledger().timestamp() > proposal.proposed_at + ttl {
            return Err(Error::Expired);
        }

        // Deduplicate.
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
            // Execute the signer change.
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
            proposal.status = ProposalStatus::Executed;
            env.storage()
                .persistent()
                .remove(&DataKey::SignerProposal);
            env.events()
                .publish((symbol_short!("sg_exec"), proposal.kind.clone()), proposal.target.clone());
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

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Attempt to finalize the proposal after a vote is recorded.
    /// Executes if threshold is met and quorum is satisfied; marks Failed if
    /// all eligible signers have voted and threshold is still not met.
    fn try_finalize(env: &Env, proposal: &mut Proposal) -> Result<(), Error> {
        let threshold: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Threshold)
            .ok_or(Error::NotInitialized)?;

        let quorum_min: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::QuorumMin)
            .unwrap_or(0);

        let approvals = proposal.approvals.len();
        let participation = proposal.approvals.len() + proposal.abstentions.len();

        if approvals >= threshold {
            if participation < quorum_min {
                return Err(Error::QuorumNotMet);
            }
            proposal.status = ProposalStatus::Executed;
            return Ok(());
        }
        Ok(())
    }

    fn assert_initialized(env: &Env) -> Result<(), Error> {
        if !env.storage().persistent().has(&DataKey::Signers) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn compute_domain_tag(env: &Env, action_id: &Symbol) -> BytesN<32> {
        let data: Bytes = action_id.clone().to_xdr(env);
        env.crypto().sha256(&data).into()
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
        for i in 0..signers.len() {
            if signers.get(i).ok_or(Error::NotASigner)? == *caller {
                return Ok(());
            }
        }
        Err(Error::NotASigner)
    }
}
