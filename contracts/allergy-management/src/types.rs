use soroban_sdk::{contracttype, Address, String, Symbol, Vec};

/// Allergy status enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllergyStatus {
    Active,
    Resolved,
}

/// Parameters for recording a new allergy
#[contracttype]
#[derive(Clone, Debug)]
pub struct RecordAllergyRequest {
    pub allergen: String,
    pub allergen_type: Symbol,
    pub reaction_type: Vec<String>,
    pub severity: Symbol,
    pub onset_date: Option<u64>,
    pub verified: bool,
}

/// Complete allergy record
#[contracttype]
#[derive(Clone, Debug)]
pub struct AllergyRecord {
    pub allergy_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub allergen: String,
    pub allergen_type: Symbol, // "med" (medication), "food", "env" (environmental)
    pub reaction_type: Vec<String>, // e.g., ["rash", "anaphylaxis", "hives"]
    pub severity: Symbol, // "mild", "moderate", "severe", "critical"
    pub onset_date: Option<u64>,
    pub recorded_date: u64,
    pub verified: bool,
    pub status: AllergyStatus,
    pub resolution_date: Option<u64>,
    pub resolution_reason: Option<String>,
    pub severity_history: Vec<SeverityUpdate>,
}

/// Severity update history entry
#[contracttype]
#[derive(Clone, Debug)]
pub struct SeverityUpdate {
    pub previous_severity: Symbol,
    pub new_severity: Symbol,
    pub updated_by: Address,
    pub updated_at: u64,
    pub reason: String,
}

/// Drug-allergy interaction result
#[contracttype]
#[derive(Clone, Debug)]
pub struct AllergyInteraction {
    pub allergy_id: u64,
    pub allergen: String,
    pub severity: Symbol,
    pub reaction_type: Vec<String>,
    pub interaction_type: Symbol, // "direct" or "cross" (cross-sensitivity)
}

/// Storage keys for the contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    AllergyCounter,
    Allergy(u64),
    PatientAllergies(Address),
    AccessControl(Address, Address), // (patient, provider)
    CrossSensitivity(String, String), // (allergen1, allergen2)
}
