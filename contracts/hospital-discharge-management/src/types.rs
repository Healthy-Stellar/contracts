use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    InvalidDates = 1,
    PlanNotFound = 2,
    InvalidStatus = 3,
    Unauthorized = 4,
    AlreadyInitialized = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    HospitalRegistry,
    ProviderRegistry,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DischargeStatus {
    Planning,
    ReadinessAssessed,
    OrdersCreated,
    Completed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadinessLevel {
    Ready,
    NeedsPreparation,
    NotReady,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DischargePlan {
    pub plan_id: u64,
    pub patient_id: BytesN<32>,
    pub hospital_id: Address,
    pub admission_date: u64,
    pub expected_discharge_date: u64,
    pub actual_discharge_date: Option<u64>,
    pub status: DischargeStatus,
    pub created_by: Address,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DischargeMedication {
    pub medication_name: String,
    pub dosage: String,
    pub frequency: String,
    pub duration: String,
    pub instructions: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FollowUpAppointment {
    pub provider_id: BytesN<32>,
    pub appointment_type: String,
    pub scheduled_date: u64,
    pub location: String,
    pub notes: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadinessScore {
    pub discharge_plan_id: u64,
    pub medical_stability_score: u32,
    pub functional_status_score: u32,
    pub support_system_score: u32,
    pub overall_score: u32,
    pub readiness_level: ReadinessLevel,
    pub assessed_by: Address,
    pub assessed_at: u64,
    pub notes: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DischargeOrders {
    pub discharge_plan_id: u64,
    pub medications: Vec<DischargeMedication>,
    pub instructions: String,
    pub restrictions: String,
    pub created_by: Address,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomeHealthArrangement {
    pub discharge_plan_id: u64,
    pub agency_id: BytesN<32>,
    pub service_type: String,
    pub frequency: String,
    pub start_date: u64,
    pub arranged_by: Address,
    pub arranged_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DMEOrder {
    pub discharge_plan_id: u64,
    pub equipment_list: Vec<String>,
    pub supplier_id: BytesN<32>,
    pub delivery_date: u64,
    pub ordered_by: Address,
    pub ordered_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DischargeEducation {
    pub discharge_plan_id: u64,
    pub topics_covered: Vec<String>,
    pub materials_provided: Vec<String>,
    pub patient_understanding_level: u32,
    pub provided_by: Address,
    pub provided_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SNFCoordination {
    pub discharge_plan_id: u64,
    pub snf_id: BytesN<32>,
    pub transfer_date: u64,
    pub care_requirements: String,
    pub coordinated_by: Address,
    pub coordinated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DischargeCompletion {
    pub discharge_plan_id: u64,
    pub actual_discharge_date: u64,
    pub discharge_destination: String,
    pub completed_by: Address,
    pub completed_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadmissionRisk {
    pub discharge_plan_id: u64,
    pub risk_factors: Vec<String>,
    pub risk_score: u32,
    pub risk_level: RiskLevel,
    pub mitigation_plan: String,
    pub tracked_by: Address,
    pub tracked_at: u64,
}
