use soroban_sdk::{contracttype, contracterror, Address, BytesN, String, Symbol, Vec};

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
}

#[contracttype]
pub enum DataKey {
    StudyCounter,
    CdCounter,
    Study(u64),
    SeriesList(u64),
    Report(u64),
    AccessList(u64),
    PatientStudies(Address),
    ViewLog(u64),
    QcReview(u64),
    AnonymizedStudy(u64),
    CdRecord(u64),
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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessGrant {
    pub viewer_id: Address,
    pub access_type: Symbol,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRecord {
    pub viewer_id: Address,
    pub view_timestamp: u64,
    pub view_duration: u32,
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
