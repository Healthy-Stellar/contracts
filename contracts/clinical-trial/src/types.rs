use soroban_sdk::{contracttype, Address, BytesN, String, Symbol, Vec};

/// Criteria rule for eligibility checking
#[contracttype]
#[derive(Clone, Debug)]
pub struct CriteriaRule {
    pub criteria_type: Symbol,
    pub parameter: String,
    pub operator: Symbol,
    pub value: String,
    pub mandatory: bool,
}

/// Adverse event record
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdverseEvent {
    pub event_type: Symbol,
    pub severity: Symbol,
    pub onset_date: u64,
    pub resolution_date: Option<u64>,
    pub serious: bool,
    pub related_to_study: bool,
}

/// Eligibility check result
#[contracttype]
#[derive(Clone, Debug)]
pub struct EligibilityResult {
    pub eligible: bool,
    pub met_inclusion: Vec<bool>,
    pub met_exclusion: Vec<bool>,
    pub disqualifying_factors: Vec<String>,
}

/// Clinical trial record
#[contracttype]
#[derive(Clone, Debug)]
pub struct ClinicalTrial {
    pub trial_record_id: u64,
    pub principal_investigator: Address,
    pub trial_id: String,
    pub trial_name: String,
    pub study_phase: Symbol,
    pub protocol_hash: BytesN<32>,
    pub start_date: u64,
    pub estimated_end_date: u64,
    pub enrollment_target: u32,
    pub irb_approval_number: String,
    pub current_enrollment: u32,
    pub status: TrialStatus,
}

/// Trial status enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrialStatus {
    Active,
    Suspended,
    Completed,
}

/// Eligibility criteria for a trial
#[contracttype]
#[derive(Clone, Debug)]
pub struct EligibilityCriteria {
    pub trial_record_id: u64,
    pub inclusion_criteria: Vec<CriteriaRule>,
    pub exclusion_criteria: Vec<CriteriaRule>,
}

/// Participant enrollment record
#[contracttype]
#[derive(Clone, Debug)]
pub struct ParticipantEnrollment {
    pub enrollment_id: u64,
    pub trial_record_id: u64,
    pub patient_id: Address,
    pub study_arm: Symbol,
    pub enrollment_date: u64,
    pub informed_consent_hash: BytesN<32>,
    pub participant_id: String,
    pub status: EnrollmentStatus,
    pub withdrawal_date: Option<u64>,
    pub withdrawal_reason: Option<Symbol>,
    pub data_retention_consent: bool,
}

/// Enrollment status enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnrollmentStatus {
    Active,
    Withdrawn,
    Completed,
}

/// Study visit record
#[contracttype]
#[derive(Clone, Debug)]
pub struct StudyVisit {
    pub enrollment_id: u64,
    pub visit_number: u32,
    pub visit_date: u64,
    pub visit_type: Symbol,
    pub data_collected_hash: BytesN<32>,
    pub adverse_events: Vec<AdverseEvent>,
}

/// Adverse event report
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdverseEventReport {
    pub event_id: u64,
    pub enrollment_id: u64,
    pub event_type: Symbol,
    pub severity: Symbol,
    pub event_description_hash: BytesN<32>,
    pub onset_date: u64,
    pub resolution_date: Option<u64>,
    pub causality_assessment: Symbol,
}

/// Protocol deviation record
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProtocolDeviation {
    pub enrollment_id: u64,
    pub deviation_type: Symbol,
    pub deviation_description: String,
    pub corrective_action: String,
    pub reported_to_irb: bool,
    pub reported_date: u64,
}

/// Safety report
#[contracttype]
#[derive(Clone, Debug)]
pub struct SafetyReport {
    pub trial_record_id: u64,
    pub reporting_period: u64,
    pub safety_data_hash: BytesN<32>,
    pub serious_adverse_events: u32,
    pub submitted_by: Address,
    pub submitted_date: u64,
}

/// Data export filters
#[contracttype]
#[derive(Clone, Debug)]
pub struct DataFilters {
    pub include_withdrawn: bool,
    pub study_arms: Vec<Symbol>,
    pub date_range_start: Option<u64>,
    pub date_range_end: Option<u64>,
}

/// Storage keys for the contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    TrialCounter,
    EnrollmentCounter,
    EventCounter,
    Trial(u64),
    Criteria(u64),
    Enrollment(u64),
    TrialEnrollments(u64),
    PatientEnrollments(Address),
    StudyVisit(u64, u32),
    AdverseEvent(u64),
    ProtocolDeviation(u64, u64),
    SafetyReport(u64, u64),
}
