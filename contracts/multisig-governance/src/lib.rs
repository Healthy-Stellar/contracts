#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, Symbol, Vec,
};

mod test;

// ── Storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Signers,
    Threshold,
    Ttl,
    Proposal(Symbol),
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Executed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub payload: Bytes,
    pub approvals: Vec<Address>,
    pub proposed_at: u64,
    pub status: ProposalStatus,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MultisigGovernance;

#[contractimpl]
impl MultisigGovernance {
    /// Initialize with a set of admin signers, an approval threshold, and a
    /// proposal TTL in seconds.
    pub fn initialize(env: Env, signers: Vec<Address>, threshold: u32, ttl_seconds: u64) {
        if env.storage().persistent().has(&DataKey::Signers) {
            panic!("Already initialized");
        }
        if threshold == 0 || threshold as usize > signers.len() as usize {
            panic!("Invalid threshold");
        }
        env.storage().persistent().set(&DataKey::Signers, &signers);
        env.storage()
            .persistent()
            .set(&DataKey::Threshold, &threshold);
        env.storage()
            .persistent()
            .set(&DataKey::Ttl, &ttl_seconds);
    }

    /// Any admin signer may open a new proposal. Panics if one already exists
    /// for the same action_id.
    pub fn propose_multisig_action(
        env: Env,
        signer: Address,
        action_id: Symbol,
        payload: Bytes,
    ) {
        signer.require_auth();
        Self::assert_signer(&env, &signer);

        let key = DataKey::Proposal(action_id.clone());
        if env.storage().persistent().has(&key) {
            panic!("Proposal already exists");
        }

        let mut approvals: Vec<Address> = Vec::new(&env);
        approvals.push_back(signer.clone());

        let proposal = Proposal {
            payload,
            approvals,
            proposed_at: env.ledger().timestamp(),
            status: ProposalStatus::Pending,
        };

        env.storage().persistent().set(&key, &proposal);
        env.events()
            .publish((symbol_short!("proposed"), action_id), signer);
    }

    /// An admin signer approves an existing proposal. Once the approval count
    /// reaches the threshold the proposal is marked Executed and an event is
    /// emitted. Expired or already-executed proposals are rejected.
    pub fn approve_multisig_action(env: Env, signer: Address, action_id: Symbol) {
        signer.require_auth();
        Self::assert_signer(&env, &signer);

        let key = DataKey::Proposal(action_id.clone());
        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Proposal not found");

        if proposal.status == ProposalStatus::Executed {
            panic!("Proposal already executed");
        }

        let ttl: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Ttl)
            .expect("Not initialized");

        if env.ledger().timestamp() > proposal.proposed_at + ttl {
            panic!("Proposal expired");
        }

        // Reject duplicate approvals from the same signer.
        for i in 0..proposal.approvals.len() {
            if proposal.approvals.get(i).unwrap() == signer {
                panic!("Already approved");
            }
        }

        proposal.approvals.push_back(signer.clone());

        let threshold: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Threshold)
            .expect("Not initialized");

        if proposal.approvals.len() >= threshold {
            proposal.status = ProposalStatus::Executed;
            env.storage().persistent().set(&key, &proposal);
            env.events()
                .publish((symbol_short!("executed"), action_id), proposal.payload);
        } else {
            env.storage().persistent().set(&key, &proposal);
            env.events()
                .publish((symbol_short!("approved"), action_id), signer);
        }
    }

    /// Read a proposal by action_id.
    pub fn get_proposal(env: Env, action_id: Symbol) -> Proposal {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(action_id))
            .expect("Proposal not found")
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn assert_signer(env: &Env, caller: &Address) {
        let signers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Signers)
            .expect("Not initialized");
        for i in 0..signers.len() {
            if signers.get(i).unwrap() == *caller {
                return;
            }
        }
        panic!("Unauthorized: not an admin signer");
    }
}
