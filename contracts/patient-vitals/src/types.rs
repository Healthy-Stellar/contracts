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
/// Number of window indices stored per per-day index page.
/// Bounds the size of each `WindowPage` entry so write cost stays constant
/// regardless of how many windows a patient has accumulated.
pub const WINDOW_PAGE_CAPACITY: u32 = 24;

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
    /// Bounded per-day index page of raw/agg window indices written for a
    /// patient. Keyed by `(patient, day)` where `day = raw_idx / WINDOW_PAGE_CAPACITY`.
    /// Replaces the unbounded `PatientWindows` vector so each write touches at
    /// most one bounded page instead of scanning the whole index.
    WindowPage(Address, u64),
    /// Number of `WindowPage` entries written for a patient (page count).
    /// Used to enumerate pages during paginated deregistration.
    WindowPageCount(Address),
    /// Monotonically increasing reading counter for stable IDs per patient
    ReadingCounter(Address),
}

/// A bounded page of window indices for a single patient.
///
/// Each page holds at most `WINDOW_PAGE_CAPACITY` raw indices and the
/// corresponding aggregate indices, so appending a new window only reads and
/// rewrites one small entry rather than the entire history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowPage {
    /// Distinct `raw_idx` values (= measurement_time / RAW_WINDOW_SECONDS)
    /// stored in this page.
    pub raw_indices: soroban_sdk::Vec<u64>,
    /// Distinct `agg_idx` values (= measurement_time / AGG_WINDOW_SECONDS)
    /// stored in this page.
    pub agg_indices: soroban_sdk::Vec<u64>,
}

impl WindowPage {
    pub fn new() -> Self {
        Self {
            raw_indices: soroban_sdk::Vec::new(),
            agg_indices: soroban_sdk::Vec::new(),
        }
    }

    /// Append a raw index if not already present in this page.
    pub fn push_raw(&mut self, raw_idx: u64) {
        if !self.raw_indices.contains(&raw_idx) {
            self.raw_indices.push_back(raw_idx);
        }
    }

    /// Append an agg index if not already present in this page.
    pub fn push_agg(&mut self, agg_idx: u64) {
        if !self.agg_indices.contains(&agg_idx) {
            self.agg_indices.push_back(agg_idx);
        }
    }
}

/// Per-vital running accumulator used while building a daily aggregate.
///
/// `sum` holds the exact sum of all values seen for this vital (u64 to avoid
/// overflow across a day's readings) and `n` holds the number of readings
/// that actually carried this vital. The average is computed on read as
/// `sum / n`, so readings that omit the vital do not skew the result and
/// there is no per-step integer truncation drift.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VitalAccumulator {
    pub sum: u64,
    pub n: u32,
}

impl VitalAccumulator {
    pub fn new() -> Self {
        Self { sum: 0, n: 0 }
    }

    /// Add a single observation for this vital.
    pub fn add(&mut self, value: u32) {
        self.sum += value as u64;
        self.n += 1;
    }

    /// Compute the average for this vital, or `None` if no readings carried it.
    pub fn average(&self) -> Option<u32> {
        if self.n == 0 {
            None
        } else {
            Some((self.sum / self.n as u64) as u32)
        }
    }
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
