use soroban_sdk::{contracttype, Address, BytesN, Env, Vec};

use crate::types::{Proposal, ProposalStatus};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeProposal {
    pub id: u64,
    pub proposer: Address,
    pub wasm_hash: BytesN<32>,
    pub artifact_hash: BytesN<32>,
    pub min_compatible_schema: u32,
    pub to_schema: u32,
    pub created_at: u64,
    pub status: ProposalStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactApproval {
    pub approvers: Vec<Address>,
}

const PROPOSAL_COUNTER: &str = "proposal_counter";
const SCHEMA_VERSION: &str = "schema_version";
const THRESHOLD: &str = "threshold";

pub fn get_threshold(env: &Env) -> u32 {
    env.storage().instance().get(&THRESHOLD).unwrap_or(0)
}

pub fn set_threshold(env: &Env, threshold: u32) {
    env.storage().instance().set(&THRESHOLD, &threshold);
}

pub fn get_schema_version(env: &Env) -> u32 {
    env.storage().instance().get(&SCHEMA_VERSION).unwrap_or(0)
}

pub fn set_schema_version(env: &Env, version: u32) {
    env.storage().instance().set(&SCHEMA_VERSION, &version);
}

pub fn next_proposal_id(env: &Env) -> u64 {
    let id: u64 = env.storage().instance().get(&PROPOSAL_COUNTER).unwrap_or(0);
    let next = id + 1;
    env.storage().instance().set(&PROPOSAL_COUNTER, &next);
    next
}

pub fn get_proposal(env: &Env, id: u64) -> Option<UpgradeProposal> {
    env.storage().persistent().get(&id)
}

pub fn set_proposal(env: &Env, id: u64, proposal: &UpgradeProposal) {
    env.storage().persistent().set(&id, proposal);
}

pub fn get_artifact_approval(env: &Env, hash: &BytesN<32>) -> ArtifactApproval {
    env.storage()
        .persistent()
        .get(hash)
        .unwrap_or(ArtifactApproval {
            approvers: Vec::new(env),
        })
}

pub fn set_artifact_approval(env: &Env, hash: &BytesN<32>, approval: &ArtifactApproval) {
    env.storage().persistent().set(hash, approval);
}

pub fn is_artifact_approved(env: &Env, hash: &BytesN<32>, proposer: &Address) -> bool {
    let approval = get_artifact_approval(env, hash);
    let threshold = get_threshold(env);
    let mut count: u32 = 0;
    for approver in approval.approvers.iter() {
        if &approver != proposer {
            count += 1;
        }
    }
    count >= threshold
}

pub fn record_artifact_approval(
    env: &Env,
    hash: &BytesN<32>,
    approver: &Address,
) -> ArtifactApproval {
    let mut approval = get_artifact_approval(env, hash);
    if !approval.approvers.contains(approver) {
        approval.approvers.push_back(approver.clone());
        set_artifact_approval(env, hash, &approval);
    }
    approval
}

pub fn get_proposal_status(env: &Env, id: u64) -> Option<ProposalStatus> {
    get_proposal(env, id).map(|p| p.status)
}

pub fn _unused(_p: Proposal) {}
