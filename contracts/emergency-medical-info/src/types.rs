use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

/// Recovery configuration set by the patient.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryConfig {
    pub guardians: Vec<Address>,
    pub recovery_threshold: u32,
    /// Ledgers that must elapse after the threshold is met before execution.
    pub timelock_ledgers: u32,
}

/// A pending key-loss recovery proposal.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryProposal {
    pub patient: Address,
    pub new_owner: Address,
    pub approvals: Vec<Address>,
    /// Ledger at which the threshold was first reached (0 until reached).
    pub threshold_reached_at: u32,
}

/// Emitted when a guardian approves a recovery proposal.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryProposed {
    pub patient: Address,
    pub new_owner: Address,
    pub guardian: Address,
    pub approvals: u32,
}

/// Emitted when a recovery proposal is cancelled by the patient.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryCancelled {
    pub patient: Address,
    pub new_owner: Address,
}

/// Emitted when a recovery proposal is executed and keys migrate.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryExecuted {
    pub patient: Address,
    pub new_owner: Address,
}

/// Emitted when recovery configuration is updated.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryConfigSet {
    pub patient: Address,
    pub guardians: u32,
    pub recovery_threshold: u32,
    pub timelock_ledgers: u32,
}

/// A medical profile record.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MedicalProfile {
    pub owner: Address,
    pub blood_type: String,
    pub allergies: String,
    pub conditions: String,
    pub medications: String,
    pub updated_at: u32,
}

/// A do-not-resuscitate directive.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DnrDirective {
    pub owner: Address,
    pub active: bool,
    pub note: String,
    pub updated_at: u32,
}

/// An emergency alert record.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct EmergencyAlert {
    pub owner: Address,
    pub message: String,
    pub created_at: u32,
}

/// A critical alert record (allergy, condition, etc.) surfaced to responders.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CriticalAlert {
    pub patient: Address,
    pub kind: String,
    pub severity: u32,
    pub note: String,
    pub author: Address,
    pub created_at: u32,
}

/// An access log entry.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AccessLog {
    pub owner: Address,
    pub actor: Address,
    pub action: String,
    pub ledger: u32,
}

/// Storage keys for the contract.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DataKey {
    RecoveryConfig(Address),
    RecoveryProposal(Address),
    Profile(Address),
    Dnr(Address),
    Alerts(Address),
    CriticalAlerts(Address),
    Logs(Address),
    KeyHash(Address),
}
