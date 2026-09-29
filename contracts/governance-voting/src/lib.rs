#![no_std]

//! # Governance Voting Contract
//!
//! Proposal lifecycle management with yes/no voting, quorum tracking, and admin rotation control
//! for decentralized governance of healthcare network policies.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Admin role required for proposal creation. Member voting restricted
//! to registered network participants. Vote authorization via require_auth. Quorum requirements ensure
//! legitimate governance decisions. Admin rotation with pending window prevents unauthorized takeover.
//!
//! **Audit Controls:** Proposal creation events with proposer, title, and vote window. Vote cast
//! events tracking voter, proposal ID, and choice. Proposal closure events with pass/reject status.
//! Admin rotation events log admin transitions. Full event trail enables governance auditing.
//!
//! **Data Retention Policy:** Proposals retained indefinitely for governance history. Completed
//! proposals archived with final status and vote counts. Admin rotation window (24 hours) enforced
//! before rotation takes effect. Expiry timestamps prevent indefinite voting windows.
//!
//! **Encryption/Integrity:** Proposal storage keyed by immutable proposal ID. Vote counts tracked
//! per proposal with yes/no separation. Admin address validated via Soroban auth. Proposal status
//! enum (Active, Passed, Rejected, Expired) prevents vote tampering post-closure.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short,
    Address, Env, String, Vec,
};

const MAX_PROPOSALS: u32 = 100;
const ADMIN_ROTATION_WINDOW: u64 = 86_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized    = 1,
    AlreadyInitialized = 2,
    Unauthorized      = 3,
    ProposalNotFound  = 4,
    AlreadyVoted      = 5,
    ProposalClosed    = 6,
    ProposalExpired   = 7,
    InvalidQuorum     = 8,
    RotationPending   = 9,
    NoRotationPending = 10,
    RotationExpired   = 11,
    NotPendingAdmin   = 12,
    TooManyProposals  = 13,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteChoice { Yes, No }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus { Active, Passed, Rejected, Expired }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id:          u64,
    pub proposer:    Address,
    pub title:       String,
    pub description: String,
    pub yes_votes:   u32,
    pub no_votes:    u32,
    pub quorum:      u32,  // minimum total votes for result to be valid
    pub deadline:    u64,  // ledger timestamp
    pub status:      ProposalStatus,
}

#[contracttype]
pub enum DataKey {
    Admin,
    NextId,
    Proposal(u64),
    Vote(u64, Address),  // (proposal_id, voter) → VoteChoice
    PendingAdmin,
    RotationExpiry,
    ProposalCount,
    Members,
    ActiveProposalCount,
}

#[contract]
pub struct GovernanceVotingContract;

#[contractimpl]
impl GovernanceVotingContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextId, &1u64);
        env.storage().instance().set(&DataKey::ProposalCount, &0u32);
        env.storage().instance().set(&DataKey::ActiveProposalCount, &0u32);
        let empty_members: Vec<Address> = Vec::new(&env);
        env.storage().instance().set(&DataKey::Members, &empty_members);
        Ok(())
    }

    /// Register a member eligible to vote.
    pub fn register_member(env: Env, admin: Address, member: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored {
            return Err(Error::Unauthorized);
        }

        let mut members: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Members)
            .unwrap_or(Vec::new(&env));

        let mut i = 0u32;
        while i < members.len() {
            if let Some(m) = members.get(i) {
                if m == member {
                    return Ok(());
                }
            }
            i += 1;
        }

        members.push_back(member);
        env.storage().instance().set(&DataKey::Members, &members);
        Ok(())
    }

    /// Unregister a member.
    pub fn unregister_member(env: Env, admin: Address, member: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored {
            return Err(Error::Unauthorized);
        }

        let mut members: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Members)
            .unwrap_or(Vec::new(&env));

        let mut found_idx = None;
        let mut i = 0u32;
        while i < members.len() {
            if let Some(m) = members.get(i) {
                if m == member {
                    found_idx = Some(i);
                    break;
                }
            }
            i += 1;
        }

        if let Some(idx) = found_idx {
            members.remove(idx);
            env.storage().instance().set(&DataKey::Members, &members);
        }

        Ok(())
    }

    /// Create a new governance proposal. Only admin can create proposals.
    pub fn create_proposal(
        env:         Env,
        admin:       Address,
        title:       String,
        description: String,
        quorum:      u32,
        duration:    u64,  // seconds from now
    ) -> Result<u64, Error> {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored {
            return Err(Error::Unauthorized);
        }
        if quorum == 0 { return Err(Error::InvalidQuorum); }

        let active_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ActiveProposalCount)
            .unwrap_or(0);
        if active_count >= MAX_PROPOSALS {
            return Err(Error::TooManyProposals);
        }

        let id: u64 = env.storage().instance().get(&DataKey::NextId).unwrap_or(1);
        let deadline = env.ledger().timestamp() + duration;

        let proposal = Proposal {
            id,
            proposer: admin.clone(),
            title,
            description,
            yes_votes: 0,
            no_votes:  0,
            quorum,
            deadline,
            status: ProposalStatus::Active,
        };
        env.storage().persistent().set(&DataKey::Proposal(id), &proposal);
        env.storage().instance().set(&DataKey::NextId, &(id + 1));
        let total_count: u32 = env.storage().instance().get(&DataKey::ProposalCount).unwrap_or(0);
        env.storage().instance().set(&DataKey::ProposalCount, &(total_count + 1));
        env.storage().instance().set(&DataKey::ActiveProposalCount, &(active_count + 1));

        env.events().publish((symbol_short!("PROPOSE"), admin), id);
        Ok(id)
    }

    /// Cast a yes or no vote on an active proposal. Voter must be a registered member.
    pub fn vote(
        env:         Env,
        voter:       Address,
        proposal_id: u64,
        choice:      VoteChoice,
    ) -> Result<(), Error> {
        voter.require_auth();

        let members: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::Members)
            .unwrap_or(Vec::new(&env));

        let mut is_member = false;
        let mut i = 0u32;
        while i < member

/* … truncated 5171 chars — edit only what you need near the top … */
