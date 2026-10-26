use soroban_sdk::{contracterror, contracttype, Address, String, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    NotFound = 2,
    InvalidParameter = 3,
    InvalidPage = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VitalSigns {
    pub blood_pressure_systolic: Option<u32>,
    pub blood_pressure_diastolic: Option<u32>,
    pub heart_rate: Option<u32>,
    pub temperature: Option<u32>,
    pub respiratory_rate: Option<u32>,
    pub oxygen_saturation: Option<u32>,
    pub blood_glucose: Option<u32>,
    pub weight: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlertThresholds {
    pub critical_low: Option<u32>,
    pub low: Option<u32>,
    pub high: Option<u32>,
    pub critical_high: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Range {
    pub min: u32,
    pub max: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VitalStatistics {
    pub min_value: u32,
    pub max_value: u32,
    pub average_value: u32,
    pub count: u32,
}

/// Window index = timestamp / WINDOW_SECONDS
pub const RAW_WINDOW_SECONDS: u64 = 3600;   // 1-hour raw buckets
pub const AGG_WINDOW_SECONDS: u64 = 86400;  // 24-hour aggregate buckets
pub const PAGE_SIZE: u32 = 50;
/// Minimum seconds between repeated alerts for the same patient+vital_type.
pub const ALERT_COOLDOWN_SECONDS: u64 = 300; // 5 minutes

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    VitalsHistory(Address),            // map to Vec<VitalReading>
    MonitoringParams(Address, Symbol), // map to MonitoringParameters
    DeviceReg(Address, String),        // map to DeviceRegistration
    /// Reverse mapping: (patient_id, device_address) → () — used for trigger_vital_alert auth.
    DeviceAddress(Address, Address),
    VitalsAlerts(Address, Symbol),     // map to Vec<VitalAlert>
    /// Raw readings bucketed by hour window index
    RawWindow(Address, u64),
    /// Aggregated stats bucketed by day window index
    AggWindow(Address, u64),
    /// Tracks the latest raw window index written for a patient
    LatestRawWindow(Address),
    /// Last alert timestamp for (patient, vital_type) — cooldown tracking
    LastAlertTime(Address, Symbol),
    /// Index of all raw-window and agg-window indices written for a patient.
    /// Stored as a WindowIndex struct; updated on every record_vital_signs call.
    /// Required so deregister_patient can enumerate and purge all buckets.
    PatientWindows(Address),
    /// Monotonically increasing reading counter for stable IDs per patient
    ReadingCounter(Address),
}

/// Tracks which RawWindow and AggWindow bucket indices have been written for
/// a given patient so they can be fully purged on deregistration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowIndex {
    /// All distinct `raw_idx` values (= measurement_time / RAW_WINDOW_SECONDS)
    /// that have ever been written for this patient.
    pub raw_indices: soroban_sdk::Vec<u64>,
    /// All distinct `agg_idx` values (= measurement_time / AGG_WINDOW_SECONDS)
    /// that have ever been written for this patient.
    pub agg_indices: soroban_sdk::Vec<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VitalsAggregate {
    pub window_start: u64,
    pub window_end: u64,
    pub count: u32,
    pub min_heart_rate: Option<u32>,
    pub max_heart_rate: Option<u32>,
    pub avg_heart_rate: Option<u32>,
    pub min_systolic: Option<u32>,
    pub max_systolic: Option<u32>,
    pub avg_systolic: Option<u32>,
    pub min_oxygen_sat: Option<u32>,
    pub max_oxygen_sat: Option<u32>,
    pub avg_oxygen_sat: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagedRawResult {
    pub readings: soroban_sdk::Vec<VitalReading>,
    pub next_page: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagedAggResult {
    pub aggregates: soroban_sdk::Vec<VitalsAggregate>,
    pub next_page: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonitoringParameters {
    pub provider_id: Address,
    pub target_range: Range,
    pub alert_thresholds: AlertThresholds,
    pub monitoring_frequency: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceRegistration {
    pub device_type: Symbol,
    pub serial_number: String,
    pub calibration_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VitalAlert {
    pub value: String,
    pub severity: Symbol,
    pub alert_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VitalReading {
    pub measurement_time: u64,
    pub vitals: VitalSigns,
    pub recorder: Address, // patient, provider, or device
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceReading {
    pub reading_time: u64,
    pub values: VitalSigns, // Can contain the required vital metric(s)
}
