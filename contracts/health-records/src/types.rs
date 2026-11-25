use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

/// A single medical record stored on-chain.
///
/// The actual clinical content lives off-chain; `encrypted_ref` points to it.
/// `integrity_hash` binds the record metadata (including `last_modified_by`)
/// so any tampering is detectable.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedicalRecord {
    pub record_id: BytesN<32>,
    pub patient: Address,
    pub provider: Address,
    pub category: String,
    pub encrypted_ref: String,
    pub integrity_hash: BytesN<32>,
    /// Address of the account that authored the current version of the record.
    /// Included in `integrity_hash` so a rewrite cannot be attributed to the
    /// original provider.
    pub last_modified_by: Address,
    pub created_at: u64,
    pub updated_at: u64,
    pub version: u32,
    pub is_active: bool,
}

/// An archived snapshot of a record, kept for audit purposes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordVersion {
    pub record_id: BytesN<32>,
    pub patient: Address,
    pub provider: Address,
    pub category: String,
    pub encrypted_ref: String,
    pub integrity_hash: BytesN<32>,
    /// Address of the account that authored this version.
    pub last_modified_by: Address,
    pub created_at: u64,
    pub updated_at: u64,
    pub version: u32,
}

/// Consent granted by a patient to a provider.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Consent {
    pub patient: Address,
    pub grantee: Address,
    pub can_read: bool,
    pub can_write: bool,
    /// Allows the grantee to amend records authored by another provider.
    pub can_amend: bool,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
    pub is_active: bool,
}

/// Per-category record counter for a patient.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryCount {
    pub patient: Address,
    pub category: String,
    pub count: u32,
}

/// Structured, versioned event payload emitted on every record mutation.
/// Contains no PHI: only identifiers, actor and version.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordEvent {
    pub record_id: BytesN<32>,
    pub patient: Address,
    pub actor: Address,
    pub version: u32,
    pub timestamp: u64,
}

/// Structured event payload for consent mutations.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentEvent {
    pub patient: Address,
    pub grantee: Address,
    pub actor: Address,
    pub timestamp: u64,
}

/// Structured event payload for patient deregistration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatientEvent {
    pub patient: Address,
    pub actor: Address,
    pub timestamp: u64,
}

/// Aggregated view of a patient's records.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatientRecords {
    pub patient: Address,
    pub record_ids: Vec<BytesN<32>>,
}
