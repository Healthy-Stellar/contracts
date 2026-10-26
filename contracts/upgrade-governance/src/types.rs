use soroban_sdk::{contracttype, Address, BytesN, Vec};

/// A single upgrade proposal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeProposal {
    pub id: u32,
    pub proposer: Address,
    pub wasm_hash: BytesN<32>,
    pub min_compatible_schema: u32,
    /// Schema version that will be applied atomically on execute.
    /// Must equal the current `SchemaVersion + 1` at execution time.
    pub to_schema: u32,
    pub approvals: Vec<Address>,
    pub created_at: u64,
    pub executed: bool,
}

/// Tracks distinct signer approvals for an artifact metadata hash.
/// The proposer is excluded from counting toward the threshold.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactApproval {
    pub hash: BytesN<32>,
    pub approvers: Vec<Address>,
}
