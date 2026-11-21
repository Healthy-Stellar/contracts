use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

/// Maximum number of audit log entries retained per patient (ring buffer bound).
pub const MAX_AUDIT_LOG_ENTRIES: u32 = 100;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmergencyType {
    Medical,
    Trauma,
    Cardiac,
    Allergic,
    Other,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyAccessLog {
    pub responder: Address,
    pub patient: Address,
    pub emergency_type: EmergencyType,
    pub justification_hash: BytesN<32>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyProfile {
    pub patient: Address,
    pub blood_type: String,
    pub allergies: Vec<String>,
    pub medications: Vec<String>,
    pub conditions: Vec<String>,
    pub emergency_contacts: Vec<Address>,
    pub dnr: bool,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditLogPage {
    pub entries: Vec<EmergencyAccessLog>,
    pub next_seq: u64,
}
