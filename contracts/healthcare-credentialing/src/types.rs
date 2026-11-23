use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

/// Status of an individual privilege. Replaces the previous magic-string
/// approach where consumers had to string-match `"SUSPENDED"` inside
/// `Privilege.restrictions`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrivilegeStatus {
    Active,
    Suspended,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Privilege {
    pub privilege_type: String,
    pub status: PrivilegeStatus,
    pub granted_date: u64,
    pub expiry_date: u64,
    pub restrictions: Vec<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credential {
    pub provider: Address,
    pub credential_type: String,
    pub license_number: String,
    pub issuing_body: String,
    pub issue_date: u64,
    pub expiry_date: u64,
    pub verified: bool,
    pub verified_by: Option<Address>,
    pub verified_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitteeMember {
    pub address: Address,
    pub role: String,
    pub added_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecredentialingCase {
    pub provider: Address,
    pub initiated_date: u64,
    pub deadline: u64,
    pub completed: bool,
    pub completed_date: Option<u64>,
}

/// A committee decision that requires an M-of-N approval before it takes
/// effect. A single committee member can propose, but the action is only
/// executed once `approvals.len() >= required_approvals`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionKind {
    Grant,
    Suspend,
    Reinstate,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decision {
    pub id: u64,
    pub kind: DecisionKind,
    pub provider: Address,
    pub proposed_by: Address,
    pub proposed_at: u64,
    pub approvals: Vec<Address>,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialingConfig {
    pub required_approvals: u32,
    pub provider_registry: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuspensionRecord {
    pub provider: Address,
    pub reason: String,
    pub suspension_date: u64,
    pub suspended_by: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEntry {
    pub action: String,
    pub actor: Address,
    pub timestamp: u64,
    pub details: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSummary {
    pub provider: Address,
    pub credential_count: u32,
    pub active_privilege_count: u32,
    pub suspended: bool,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationProof {
    pub credential_hash: BytesN<32>,
    pub verifier: Address,
    pub verified_at: u64,
}
