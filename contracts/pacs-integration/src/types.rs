use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    Unauthorized = 2,
    InvalidInput = 3,
    AlreadyExists = 4,
    AccessExpired = 5,
    ReportAlreadyExists = 6,
    GrantRevoked = 7,
    TimestampOutOfBounds = 8,
    NonMonotonicViewTimestamp = 9,
    NotInitialized = 10,
    AlreadyInitialized = 11,
    ProviderNotRegistered = 12,
}

#[contracttype]
pub enum DataKey {
    Admin,
    /// provider-registry contract used to verify ordering providers
    ProviderRegistry,
    StudyCounter,
    CdCounter,
    Study(u64),
    SeriesList(u64),
    Report(u64),
    AccessList(u64),
    /// (patient, seq) -> study_id; paged per-patient study index
    PatientStudy(Address, u64),
    /// patient -> number of studies indexed under PatientStudy
    PatientStudyCount(Address),
    /// (study_id, seq) -> ViewRecord; one ledger entry per view
    ViewLog(u64, u64),
    /// study_id -> ViewLogHead (next seq + last entry hash)
    ViewLogHead(u64),
    ViewerLastViewTs(u64, Address),
    QcReview(u64),
    /// (study_id, rotation_epoch) -> anonymized UID string
    AnonymizedStudy(u64, u32),
    CdRecord(u64),
    /// Global per-contract anonymization salt (instance storage)
    AnonSalt,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagingStudy {
    pub study_id: u64,
    pub patient_id: Address,
    pub ordering_provider: Address,
    pub study_uid: String,
    pub modality: Symbol,
    pub body_part: String,
    pub study_date: u64,
    pub study_description: String,
    pub series_count: u32,
    pub image_count: u32,
    pub storage_location_hash: BytesN<32>,
    pub has_report: bool,
    pub critical_findings: bool,
    pub registered_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesInfo {
    pub series_uid: String,
    pub series_number: u32,
    pub series_description: String,
    pub image_count: u32,
    pub acquisition_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagingReport {
    pub study_id: u64,
    pub radiologist_id: Address,
    pub report_type: Symbol,
    pub report_hash: BytesN<32>,
    pub critical_findings: bool,
    pub reported_at: u64,
    pub crit_find_ack_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessGrant {
    pub viewer_id: Address,
    pub access_type: Symbol,
    pub purpose: String,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
    pub revoked_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRecord {
    pub viewer_id: Address,
    pub purpose: String,
    pub view_timestamp: u64,
    pub view_duration: u32,
    pub recorded_at: u64,
    pub previous_entry_hash: Option<BytesN<32>>,
    pub entry_hash: BytesN<32>,
}

/// Head pointer of a study's hash-chained view log. Lets each append run in
/// O(1) without loading earlier entries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewLogHead {
    /// Number of entries written so far; also the seq of the next entry.
    pub count: u64,
    pub last_hash: Option<BytesN<32>>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QcReview {
    pub study_id: u64,
    pub reviewer_id: Address,
    pub quality_score: u32,
    pub technical_issues: Vec<String>,
    pub repeat_required: bool,
    pub reviewed_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CdRecord {
    pub cd_id: u64,
    pub study_ids: Vec<u64>,
    pub patient_id: Address,
    pub requesting_provider: Address,
    pub cd_token: String,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonCriteria {
    pub modality: Option<Symbol>,
    pub body_part: String,
    pub max_age_days: u32,
    pub same_side: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagingFilters {
    pub modality: Option<Symbol>,
    pub body_part: Option<String>,
    pub start_date: Option<u64>,
    pub end_date: Option<u64>,
    pub has_critical_findings: Option<bool>,
}
