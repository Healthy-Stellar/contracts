use soroban_sdk::{contracttype, Address, String, Symbol, Vec};

/// Reference to an encrypted envelope stored off-chain under the patient's key.
///
/// Clinical free text and coded clinical values MUST NOT be stored in plaintext
/// in contract state or events: on Stellar all contract storage is world-readable
/// via RPC `getLedgerEntries` and ledger history/archives, regardless of getter
/// `require_auth()` checks. Getters are integrity/UX controls, not confidentiality.
/// Only the opaque envelope reference (and non-PHI metadata) belongs on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedEnvelopeRef {
    /// Content-addressed identifier of the off-chain encrypted envelope.
    pub envelope_id: String,
    /// Salted hash commitment binding the envelope to this record.
    pub commitment: String,
}

/// Allergy status enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllergyStatus {
    Active,
    Resolved,
    Archived,
    Deleted,
}

impl AllergyStatus {
    /// Check if the allergy status is active (should be considered in drug interaction checks)
    pub fn is_active(&self) -> bool {
        matches!(self, AllergyStatus::Active)
    }
}

/// Parameters for recording a new allergy
#[contracttype]
#[derive(Clone, Debug)]
pub struct RecordAllergyRequest {
    /// Encrypted envelope ref for the allergen (PHI kept off-chain).
    pub allergen: EncryptedEnvelopeRef,
    pub allergen_type: Symbol,
    /// Encrypted envelope ref for the reaction details (PHI kept off-chain).
    pub reaction_type: EncryptedEnvelopeRef,
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
    /// Encrypted envelope ref for the allergen (PHI kept off-chain).
    pub allergen: EncryptedEnvelopeRef,
    pub allergen_type: Symbol, // "med" (medication), "food", "env" (environmental)
    /// Encrypted envelope ref for the reaction details (PHI kept off-chain).
    pub reaction_type: EncryptedEnvelopeRef,
    pub severity: Symbol,      // "mild", "moderate", "severe", "critical"
    pub onset_date: Option<u64>,
    pub recorded_date: u64,
    pub verified: bool,
    pub status: AllergyStatus,
    pub resolution_date: Option<u64>,
    /// Encrypted envelope ref for the resolution reason (PHI kept off-chain).
    pub resolution_reason: Option<EncryptedEnvelopeRef>,
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
    /// Encrypted envelope ref for the update reason (PHI kept off-chain).
    pub reason: EncryptedEnvelopeRef,
}

/// Drug-allergy interaction result
#[contracttype]
#[derive(Clone, Debug)]
pub struct AllergyInteraction {
    pub allergy_id: u64,
    /// Encrypted envelope ref for the allergen (PHI kept off-chain).
    pub allergen: EncryptedEnvelopeRef,
    pub severity: Symbol,
    /// Encrypted envelope ref for the reaction details (PHI kept off-chain).
    pub reaction_type: EncryptedEnvelopeRef,
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
    AccessControl(Address, Address),  // (patient, provider)
    GrantedProviders(Address),        // (patient) -> Vec<Address>
    CrossSensitivity(String, String), // (allergen1, allergen2)
    PatientRegistry,
    ProviderRegistry,
    HospitalRegistry,
    InsurerRegistry,
}
