use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    ImmunizationCounter,
    PatientImmunizations(Address), // List of IDs (u64)
    ImmunizationRecord(u64),
    AdverseEvents(u64),            // List of AdverseEvent
    PatientVaccineSeries(Address), // List of VaccineSeries
    LotDose(String, u64),          // (normalized lot, seq) -> immunization id
    LotCount(String),              // normalized lot -> number of LotDose entries
    LotPatient(String, Address),   // (normalized lot, patient) -> true; de-dup marker
    LotPatientAt(String, u64),     // (normalized lot, seq) -> distinct patient
    LotPatientCount(String),       // normalized lot -> number of distinct patients
    PatientDose(Address, String, u32), // (patient, cvx_code, dose_number) -> immunization id
    PatientLastDose(Address, String),  // (patient, cvx_code) -> LastDose
    Regulator,                     // Address authorized for public-health/recall queries
    ProviderRegistry,              // Address of the provider-registry contract
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    RecordNotFound = 2,
    InvalidDoseNumber = 3,
    AlreadyInitialized = 4,
    NotInitialized = 5,
    InvalidAdministrationDate = 6,
    VaccineExpired = 7,
    /// The same (patient, cvx_code, dose_number) is already recorded.
    DuplicateDose = 8,
    /// dose_number is not last + 1 (or the dose predates the previous one)
    /// and no override reason was supplied.
    OutOfSequenceDose = 9,
    /// Lot number is empty or too long after normalization.
    InvalidLotNumber = 10,
}

/// Most recent dose recorded for a (patient, cvx_code) pair.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LastDose {
    pub dose_number: u32,
    pub administration_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaccineRecord {
    pub patient_id: Address,
    pub provider_id: Address,
    pub vaccine_name: String,
    pub cvx_code: String,
    pub lot_number: String,
    pub manufacturer: String,
    pub administration_date: u64,
    pub expiration_date: u64,
    pub dose_number: u32,
    pub route: Symbol,
    pub site: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdverseEvent {
    pub reporter: Address,
    pub event_description: String,
    pub severity: Symbol,
    pub onset_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaccineSeries {
    pub series_name: String,
    pub cvx_code: String,
    pub doses_required: u32,
    pub schedule_hash: BytesN<32>,
}
