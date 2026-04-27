use soroban_sdk::{contracterror, contracttype, Address, String, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    NotFound = 2,
    InvalidParameter = 3,
    InvalidPage = 4,
    CalibrationExpired = 5,
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

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    VitalsHistory(Address),            // map to Vec<VitalReading>
    MonitoringParams(Address, Symbol), // map to MonitoringParameters
    DeviceReg(Address, String),        // map to DeviceRegistration
    VitalsAlerts(Address, Symbol),     // map to Vec<VitalAlert>
    /// Raw readings bucketed by hour window index
    RawWindow(Address, u64),
    /// Aggregated stats bucketed by day window index
    AggWindow(Address, u64),
    /// Tracks the latest raw window index written for a patient
    LatestRawWindow(Address),
    /// Prevents a device signer from being bound to more than one patient
    DeviceSignerBound(Address),
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
    pub device_signer: Address,
    pub calibration_expiry: u64,
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
