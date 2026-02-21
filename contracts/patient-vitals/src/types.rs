use soroban_sdk::{contracterror, contracttype, Address, String, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    NotFound = 2,
    InvalidParameter = 3,
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

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    VitalsHistory(Address),            // map to Vec<VitalReading>
    MonitoringParams(Address, Symbol), // map to MonitoringParameters
    DeviceReg(Address, String),        // map to DeviceRegistration
    VitalsAlerts(Address, Symbol),     // map to Vec<VitalAlert>
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
