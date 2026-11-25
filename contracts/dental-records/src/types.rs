use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedProcedure {
    pub procedure_id: u64,
    pub procedure_code: String,
    pub tooth_number: Option<String>,
    pub surfaces: Option<Vec<Symbol>>,
    pub description: String,
    pub priority: Symbol,
    pub estimated_cost: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedProcedure {
    pub procedure_code: String,
    pub tooth_number: Option<String>,
    pub surfaces: Option<Vec<Symbol>>,
    pub materials_used: Vec<String>,
    pub technique: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DentalChart {
    pub patient_id: Address,
    pub dentist_id: Address,
    pub chart_date: u64,
    pub tooth_notation_system: Symbol,
}

/// Tooth surface a condition applies to. `WholeTooth` is used for findings
/// that are not surface-specific (missing, implant, crown, ...).
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ToothSurface {
    WholeTooth,
    Occlusal,
    Incisal,
    Mesial,
    Distal,
    Buccal,
    Facial,
    Lingual,
    Palatal,
}

/// Allow-listed tooth conditions.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ToothConditionKind {
    Caries,
    Filling,
    Crown,
    Missing,
    Implant,
    RootCanal,
    Fracture,
    Sealant,
    Bridge,
    Veneer,
}

/// Periodontal probing sites.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PerioSite {
    MesioBuccal,
    Buccal,
    DistoBuccal,
    MesioLingual,
    Lingual,
    DistoLingual,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToothCondition {
    pub surface: ToothSurface,
    pub condition: ToothConditionKind,
    pub condition_details: Option<String>,
    pub recorded_by: Address,
    pub recorded_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeriodontalAssessment {
    pub probing_depth: u32,
    pub recession: u32,
    pub bleeding_on_probing: bool,
    pub mobility: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreatmentPlan {
    pub patient_id: Address,
    pub dentist_id: Address,
    pub plan_date: u64,
    pub procedures: Vec<PlannedProcedure>,
    pub phased_treatment: bool,
    pub estimated_cost: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Appointment {
    pub treatment_plan_id: u64,
    pub procedure_id: u64,
    pub scheduled_date: u64,
    pub estimated_duration: u32,
    pub sedation_required: bool,
    pub is_completed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Radiograph {
    pub patient_id: Address,
    pub image_type: Symbol,
    pub image_date: u64,
    pub teeth_included: Vec<String>,
    pub findings: Vec<String>,
    pub image_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrthodonticTreatment {
    pub patient_id: Address,
    pub orthodontist_id: Address,
    pub treatment_start_date: u64,
    pub appliance_type: Symbol,
    pub treatment_plan_hash: BytesN<32>,
    pub estimated_duration_months: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrthoAdjustment {
    pub adjustment_date: u64,
    pub adjustments_made: Vec<String>,
    pub arch_wire_change: bool,
    pub next_appointment_weeks: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    ChartCount,
    Chart(u64),                 // chart_id
    ToothCond(u64, String, ToothSurface), // chart_id, tooth_number, surface
    Perio(u64, String, PerioSite),        // chart_id, tooth_number, site
    PlanCount,
    Plan(u64), // treatment_plan_id
    AppointmentCount,
    Appt(u64),         // appointment_id
    ProcedureLog(u64), // appointment_id -> log
    RadiographCount,
    Radio(u64), // radiograph_id
    OrthoCount,
    Ortho(u64),         // ortho_treatment_id
    OrthoAdj(u64, u64), // ortho_treatment_id, adjustment_date
    RxCount,
    Rx(u64), // rx_id
    Consent(BytesN<32>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcedureLog {
    pub dentist_id: Address,
    pub procedure_date: u64,
    pub procedures_completed: Vec<CompletedProcedure>,
    pub anesthesia_used: Vec<String>,
    pub complications: Option<Vec<String>>,
    pub post_op_instructions_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedicationPrescription {
    pub patient_id: Address,
    pub dentist_id: Address,
    pub medication: String,
    pub indication: String,
    pub dosage_instructions: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InformedConsent {
    pub procedure: String,
    pub risks_disclosed: Vec<String>,
    pub alternatives_discussed: Vec<String>,
    pub consent_date: u64,
    pub consent_document_hash: BytesN<32>,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NotFound = 1,
    Unauthorized = 2,
    InvalidInput = 3,
    /// scheduled_date must be strictly in the future
    InvalidScheduledDate = 4,
    /// procedure_date / image_date must not be in the future
    InvalidPastDate = 5,
    /// tooth_notation_system is not one of universal, palmer, fdi
    InvalidNotationSystem = 6,
    /// tooth_number is not valid for the chart's notation system
    InvalidToothNumber = 7,
}
