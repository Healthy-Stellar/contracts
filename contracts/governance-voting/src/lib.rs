#![no_std]

//! Governance voting contract: proposal lifecycle with yes/no voting and quorum tracking.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short,
    Address, Env, String, Vec,
};

const MAX_PROPOSALS: u32 = 100;

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
}

#[contract]
pub struct GovernanceVotingContract;

#[contractimpl]
impl GovernanceVotingContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextId, &1u64);
        Ok(())
    }

    /// Create a new governance proposal.
    pub fn create_proposal(
        env:         Env,
        proposer:    Address,
        title:       String,
        description: String,
        quorum:      u32,
        duration:    u64,  // seconds from now
    ) -> Result<u64, Error> {
        proposer.require_auth();
        if quorum == 0 { return Err(Error::InvalidQuorum); }

        let id: u64 = env.storage().instance().get(&DataKey::NextId).unwrap_or(1);
        let deadline = env.ledger().timestamp() + duration;

        let proposal = Proposal {
            id,
            proposer: proposer.clone(),
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

        env.events().publish((symbol_short!("PROPOSE"), proposer), id);
        Ok(id)
    }

    /// Cast a yes or no vote on an active proposal.
    pub fn vote(
        env:         Env,
        voter:       Address,
        proposal_id: u64,
        choice:      VoteChoice,
    ) -> Result<(), Error> {
        voter.require_auth();

        let vote_key = DataKey::Vote(proposal_id, voter.clone());
        if env.storage().persistent().has(&vote_key) {
            return Err(Error::AlreadyVoted);
        }

        let mut proposal: Proposal = env.storage().persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Active {
            return Err(Error::ProposalClosed);
        }
        if env.ledger().timestamp() > proposal.deadline {
            proposal.status = ProposalStatus::Expired;
            env.storage().persistent().set(&DataKey::Proposal(proposal_id), &proposal);
            return Err(Error::ProposalExpired);
        }

        match choice {
            VoteChoice::Yes => proposal.yes_votes += 1,
            VoteChoice::No  => proposal.no_votes  += 1,
        }

        env.storage().persistent().set(&vote_key, &choice);
        env.storage().persistent().set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish((symbol_short!("VOTE"), voter), (proposal_id, proposal.yes_votes, proposal.no_votes));
        Ok(())
    }

    /// Finalize a proposal after deadline: Passed if quorum met and yes > no, else Rejected.
    pub fn finalize(env: Env, proposal_id: u64) -> Result<ProposalStatus, Error> {
        let mut proposal: Proposal = env.storage().persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Active {
            return Ok(proposal.status.clone());
        }

        let total = proposal.yes_votes + proposal.no_votes;
        proposal.status = if env.ledger().timestamp() < proposal.deadline {
            ProposalStatus::Active
        } else if total < proposal.quorum || proposal.yes_votes <= proposal.no_votes {
            ProposalStatus::Rejected
        } else {
            ProposalStatus::Passed
        };

        env.storage().persistent().set(&DataKey::Proposal(proposal_id), &proposal);
        Ok(proposal.status.clone())
    }

    pub fn get_proposal(env: Env, id: u64) -> Result<Proposal, Error> {
        env.storage().persistent()
            .get(&DataKey::Proposal(id))
            .ok_or(Error::ProposalNotFound)
    }

    pub fn has_voted(env: Env, proposal_id: u64, voter: Address) -> bool {
        env.storage().persistent().has(&DataKey::Vote(proposal_id, voter))
    }
}

#[cfg(test)]
mod test;
