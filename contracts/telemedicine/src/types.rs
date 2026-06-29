use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    VisitNotFound = 2,
    InvalidStatusTransition = 3,
    IneligibleLocation = 4,
    SessionExpired = 5,
    InvalidSessionToken = 6,
    SessionAlreadyUsed = 7,
    ProviderNotLicensed = 8,
    PolicyNotFound = 9,
    LicenseExpired = 10,
    CrossStateNotPermitted = 11,
    /// Recording metadata cannot be stored without explicit patient consent
    RecordingConsentRequired = 12,
    RateLimitExceeded = 13,
}

/// On-chain record of a provider's license in a given jurisdiction (state/region).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderLicense {
    pub provider_id: Address,
    /// ISO 3166-2 subdivision code, e.g. "US-NY"
    pub jurisdiction: String,
    pub license_number: String,
    /// Unix timestamp; 0 = no expiry
    pub valid_until: u64,
    pub active: bool,
}

/// Jurisdiction policy: whether cross-state practice is permitted and which
/// compact memberships apply.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JurisdictionPolicy {
    /// The patient's jurisdiction
    pub jurisdiction: String,
    /// If true, providers licensed in any compact-member state may practice here
    pub allows_compact: bool,
    /// Comma-separated list of compact-member state codes, e.g. "US-NY,US-CA"
    pub compact_members: String,
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
    /// Explicit per-session recording consent (HIPAA). None = not yet decided.
    pub recording_consent: Option<bool>,
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
pub struct SessionRecord {
    pub token_hash: BytesN<32>,
    pub visit_id: u64,
    pub caller: Address,
    pub expires_at: u64,
    pub used: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSessionWindow {
    /// Number of sessions created in current window
    pub session_count: u32,
    /// Timestamp when the current window started
    pub window_start: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitConfig {
    /// Maximum sessions per provider per window (in seconds)
    pub max_sessions_per_window: u32,
    /// Window duration in seconds (default: 86400 = 24 hours)
    pub window_duration_secs: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    VirtualVisit(u64),
    VisitCount,
    SessionNonce,
    Session(u64),
    /// (provider_id, jurisdiction) -> ProviderLicense
    LicenseRegistry(Address, String),
    /// jurisdiction -> JurisdictionPolicy
    JurisdictionPolicy(String),
    /// Provider session rate limit window: (provider_id) -> ProviderSessionWindow
    ProviderSessionWindow(Address),
    /// Global rate limit configuration
    RateLimitConfig,
}
