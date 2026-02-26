use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    DeviceCounter,
    ImplantCounter,
    DmeCounter,
    RecallCounter,
    MaintenanceCounter,
    DeviceRecord(u64),
    ImplantRecord(u64),
    DmeRecord(u64),
    RecallInfo(u64),
    MaintenanceRecord(u64),
    PatientImplants(Address),
    DeviceImplants(u64),
    DeviceRecalls(u64),
    PerformanceReports(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    RecordNotFound = 2,
    DeviceNotActive = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceRecord {
    pub device_id: u64,
    pub device_udi: String,
    pub device_type: Symbol,
    pub manufacturer: String,
    pub model_number: String,
    pub lot_number: String,
    pub manufacturing_date: u64,
    pub expiration_date: Option<u64>,
    pub device_specs_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplantRecord {
    pub implant_record_id: u64,
    pub patient_id: Address,
    pub device_id: u64,
    pub implant_date: u64,
    pub implant_location: String,
    pub implanting_provider: Address,
    pub surgical_notes_hash: BytesN<32>,
    pub is_active: bool,
    pub removal_date: Option<u64>,
    pub removal_reason: Option<String>,
    pub explant_analysis_hash: Option<BytesN<32>>,
    pub maintenance_history: Vec<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DmePrescription {
    pub prescription_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub device_type: Symbol,
    pub device_id: u64,
    pub prescription_date: u64,
    pub duration_days: Option<u64>,
    pub instructions_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaintenanceRecord {
    pub maintenance_id: u64,
    pub implant_record_id: u64,
    pub maintenance_date: u64,
    pub maintenance_type: Symbol,
    pub performed_by: Address,
    pub notes_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceReport {
    pub implant_record_id: u64,
    pub patient_id: Address,
    pub performance_data_hash: BytesN<32>,
    pub reported_date: u64,
    pub complications: Option<Vec<String>>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecallInfo {
    pub recall_id: u64,
    pub device_ids: Vec<u64>,
    pub recall_reason: String,
    pub severity: Symbol,
    pub recall_date: u64,
    pub action_required: String,
    pub resolution_deadline: Option<u64>,
}
