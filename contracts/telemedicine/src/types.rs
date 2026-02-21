use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    VisitNotFound = 2,
    InvalidStatusTransition = 3,
    IneligibleLocation = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisitStatus {
    Scheduled,
    InProgress,
    Completed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualVisit {
    pub visit_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub scheduled_time: u64,
    pub visit_type: Symbol,
    pub platform: Symbol,
    pub status: VisitStatus,
    pub session_start: Option<u64>,
    pub session_end: Option<u64>,
    pub patient_location: String,
    pub consent_documented: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EligibilityResult {
    pub is_eligible: bool,
    pub reason: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrescriptionRequest {
    pub medication_name: String,
    pub dosage: String,
    pub frequency: String,
    pub duration_days: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    VirtualVisit(u64),
    VisitCount,
}
