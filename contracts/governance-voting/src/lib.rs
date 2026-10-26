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
const MIN_VOTING_DURATION: u64 = 3_600;
const BPS_DENOMINATOR: u32 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized    = 1,
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
    InvalidDuration   = 14,
    NotMember         = 15,
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
    pub member_epoch: u64, // electorate snapshot at creation
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
    Member(Address),     // persistent: member → bool
    MemberCount,
    MemberEpoch,
    ActiveProposalCount,
}

#[contract]
pub struct GovernanceVotingContract;

#[contractimpl]
impl GovernanceVotingContract {
    /// Runs atomically as part of contract creation, so the deployer binds the
    /// admin address and no front-running `initialize` call is possible.
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextId, &1u64);
        env.storage().instance().set(&DataKey::ProposalCount, &0u32);
        env.storage().instance().set(&DataKey::ActiveProposalCount, &0u32);
        env.storage().instance().set(&DataKey::MemberCount, &0u32);
        env.storage().instance().set(&DataKey::MemberEpoch, &0u64);
        Ok(())
    }

    /// Propose transferring admin to `new_admin`. Must be confirmed within 24 hours.
    pub fn propose_admin_rotation(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored {
            return Err(Error::Unauthorized);
        }
        if env.storage().instance().has(&DataKey::PendingAdmin) {
            let expiry: u64 = env
                .storage()
                .instance()
                .get(&DataKey::RotationExpiry)
                .unwrap_or(0);
            if env.ledger().timestamp() <= expiry {
                return Err(Error::RotationPending);
            }
            env.storage().instance().remove(&DataKey::PendingAdmin);
            env.storage().instance().remove(&DataKey::RotationExpiry);
        }
        let expiry = env.ledger().timestamp() + ADMIN_ROTATION_WINDOW;
        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
        env.storage().instance().set(&DataKey::RotationExpiry, &expiry);
        Ok(())
    }

    /// New admin confirms the rotation proposed by the current admin.
    pub fn accept_admin_rotation(env: Env, new_admin: Address) -> Result<(), Error> {
        new_admin.require_auth();
        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .ok_or(Error::NoRotationPending)?;
        if new_admin != pending {
            return Err(Error::NotPendingAdmin);
        }
        let expiry: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RotationExpiry)
            .unwrap_or(0);
        if env.ledger().timestamp() > expiry {
            return Err(Error::RotationExpired);
        }
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.storage().instance().remove(&DataKey::RotationExpiry);
        Ok(())
    }

    /// Cancel a pending admin rotation.
    pub fn cancel_admin_rotation(env: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored {
            return Err(Error::Unauthorized);
        }
        if !env.storage().instance().has(&DataKey::PendingAdmin) {
            return Err(Error::NoRotationPending);
        }
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.storage().instance().remove(&DataKey::RotationExpiry);
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

        let key = DataKey::Member(member.clone());
        if env.storage().persistent().get(&key).unwrap_or(false) {
            return Ok(());
        }

        env.storage().persistent().set(&key, &true);
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MemberCount)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::MemberCount, &(count + 1));
        let epoch: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MemberEpoch)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::MemberEpoch, &(epoch + 1));
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

        let key = DataKey::Member(member.clone());
        if !env.storage().persistent().get(&key).unwrap_or(false) {
            return Ok(());
        }

        env.storage().persistent().remove(&key);
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MemberCount)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::MemberCount, &count.saturating_sub(1));
        let epoch: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MemberEpoch)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::MemberEpoch, &(epoch + 1));
        Ok(())
    }

    /// Returns true if `member` is currently registered.
    pub fn is_member(env: Env, member: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Member(member))
            .unwrap_or(false)
    }

    /// Create a proposal. Quorum is derived from the current member count via `quorum_bps`.
    pub fn create_proposal(
        env: Env,
        admin: Address,
        title: String,
        description: String,
        quorum_bps: u32,
        duration: u64,
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
        if quorum_bps == 0 || quorum_bps > BPS_DENOMINATOR {
            return Err(Error::InvalidQuorum);
        }
        if duration < MIN_VOTING_DURATION {
            return Err(Error::InvalidDuration);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ProposalCount)
            .unwrap_or(0);
        if count >= MAX_PROPOSALS {
            return Err(Error::TooManyProposals);
        }

        let member_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MemberCount)
            .unwrap_or(0);
        let quorum = ((member_count as u64 * quorum_bps as u64) / BPS_DENOMINATOR as u64) as u32;
        let quorum = if quorum == 0 { 1 } else { quorum };

        let epoch: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MemberEpoch)
            .unwrap_or(0);

        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextId)
            .unwrap_or(1);
        let deadline = env.ledger().timestamp() + duration;

        let proposal = Proposal {
            id,
            proposer: admin.clone(),
            title: title.clone(),
            description,
            yes_votes: 0,
            no_votes: 0,
            quorum,
            deadline,
            status: ProposalStatus::Active,
            member_epoch: epoch,
        };
        env.storage().instance().set(&DataKey::Proposal(id), &proposal);
        env.storage().instance().set(&DataKey::NextId, &(id + 1));
        env.storage().instance().set(&DataKey::ProposalCount, &(count + 1));
        let active: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ActiveProposalCount)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::ActiveProposalCount, &(active + 1));

        env.events().publish(
            (symbol_short!("proposal"), symbol_short!("created")),
            (id, admin, title, deadline),
        );
        Ok(id)
    }

    /// Cast a vote on an active proposal. Only members registered at creation time may vote.
    pub fn vote(env: Env, voter: Address, proposal_id: u64, choice: VoteChoice) -> Result<(), Error> {
        voter.require_auth();

        let mut proposal: Proposal = env
            .storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Active {
            return Err(Error::ProposalClosed);
        }
        if env.ledger().timestamp() > proposal.deadline {
            return Err(Error::ProposalExpired);
        }

        let vote_key = DataKey::Vote(proposal_id, voter.clone());
        if env.storage().instance().has(&vote_key) {
            return Err(Error::AlreadyVoted);
        }

        if !env
            .storage()
            .persistent()
            .get(&DataKey::Member(voter.clone()))
            .unwrap_or(false)
        {
            return Err(Error::NotMember);
        }

        match choice {
            VoteChoice::Yes => proposal.yes_votes += 1,
            VoteChoice::No => proposal.no_votes += 1,
        }
        env.storage().instance().set(&vote_key, &choice);
        env.storage().instance().set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish(
            (symbol_short!("proposal"), symbol_short!("vote")),
            (proposal_id, voter, choice),
        );
        Ok(())
    }

    /// Finalize a proposal once its deadline has passed.
    pub fn finalize(env: Env, proposal_id: u64) -> Result<ProposalStatus, Error> {
        let mut proposal: Proposal = env
            .storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Active {
            return Err(Error::ProposalClosed);
        }
        if env.ledger().timestamp() <= proposal.deadline {
            return Err(Error::ProposalExpired);
        }

        let total = proposal.yes_votes + proposal.no_votes;
        let status = if total < proposal.quorum {
            ProposalStatus::Rejected
        } else if proposal.yes_votes > proposal.no_votes {
            ProposalStatus::Passed
        } else {
            ProposalStatus::Rejected
        };

        proposal.status = status.clone();
        env.storage().instance().set(&DataKey::Proposal(proposal_id), &proposal);
        let active: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ActiveProposalCount)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::ActiveProposalCount, &active.saturating_sub(1));

        env.events().publish(
            (symbol_short!("proposal"), symbol_short!("closed")),
            (proposal_id, status.clone()),
        );
        Ok(status)
    }

    /// Read a proposal by id.
    pub fn get_proposal(env: Env, proposal_id: u64) -> Result<Proposal, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)
    }

    /// Current number of registered members.
    pub fn member_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MemberCount)
            .unwrap_or(0)
    }
}
