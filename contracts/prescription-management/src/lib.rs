#![no_std]

use shared::temporal;
use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Vec, contract, contractclient, contracterror,
    contractimpl, contracttype,
};

// ── Allergy-management client ─────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllergyInteraction {
    pub allergy_id: u64,
    pub allergen: String,
    pub severity: Symbol,
    pub reaction_type: Vec<String>,
    pub interaction_type: Symbol,
}

#[contractclient(name = "AllergyManagementClient")]
pub trait AllergyManagementInterface {
    fn check_drug_allergy_interaction(
        env: Env,
        patient_id: Address,
        drug_name: String,
    ) -> Vec<AllergyInteraction>;
}

// ── Provider-registry client ──────────────────────────────────────────────────

#[contractclient(name = "ProviderRegistryClient")]
pub trait ProviderRegistryInterface {
    fn is_provider(env: Env, provider: Address) -> bool;
}

/// Maximum number of transfer records retained per prescription.
/// Attempting to exceed this returns `Error::TransferHistoryFull`.
pub const MAX_TRANSFER_HISTORY: u32 = 100;

/// Maximum number of concurrently active prescriptions a single patient may
/// hold. "Active" means status is Issued, Active, or PartiallyDispensed
/// *and* not yet past `valid_until`. Attempting to issue prescription
/// N+1 beyond this limit returns `Error::TooManyActivePrescriptions`.
pub const MAX_ACTIVE_PRESCRIPTIONS: u32 = 50;

pub const SECONDS_PER_HOUR: u64 = 3600;
/// 30-day window used when extending a prescription's validity on refill.
pub const REFILL_WINDOW_SECS: u64 = 30 * 24 * SECONDS_PER_HOUR;
/// Divisor applied to a prescription's total quantity for schedule-2 per-dispense limits.
pub const MIN_REFILL_QUANTITY_DIVISOR: u32 = 2;
/// Default max prescriptions a provider may issue in a 24-hour window.
pub const DEFAULT_PRESCRIPTION_LIMIT: u32 = 100;
/// Duration of the per-provider rate-limit window.
pub const RATE_LIMIT_WINDOW_SECS: u64 = 24 * SECONDS_PER_HOUR;

// ── DEA number validation ─────────────────────────────────────────────────────

/// Valid DEA registrant-type letters (first character of a DEA number).
/// Source: DEA registrant type codes — A/B (practitioners), C (practitioners–military),
/// F/G (teaching practitioners), M (mid-level practitioners), P/R/S/T/U (distributors),
/// X (DATA-waivered practitioners).
const DEA_REGISTRANT_TYPES: &[u8] = b"ABCFGMPRSTUX";

/// Validate a DEA registration number.
///
/// Rules:
/// 1. Exactly 9 characters: 2 ASCII uppercase letters followed by 7 ASCII digits.
/// 2. The first letter must be a recognised DEA registrant-type code:
///    A, B, C, F, G, M, P, R, S, T, U, X (mid-level practitioners use M or other letters;
///    we accept the full set used by the DEA).
/// 3. Check-digit: let the 7 digits be d1…d7.
///    `(d1 + d3 + d5) + 2*(d2 + d4 + d6)` — the units digit of this sum must equal d7.
///
/// Returns `true` only when all three conditions hold.
fn validate_dea_number(dea: &[u8]) -> bool {
    // Must be exactly 9 bytes
    if dea.len() != 9 {
        return false;
    }

    // First two bytes must be uppercase ASCII letters
    let (b0, b1) = (dea[0], dea[1]);
    if !b0.is_ascii_uppercase() || !b1.is_ascii_uppercase() {
        return false;
    }

    // First letter must be a known registrant-type code
    if !DEA_REGISTRANT_TYPES.contains(&b0) {
        return false;
    }

    // Bytes 2-8 must all be ASCII digits; collect as u32 values
    let mut digits = [0u32; 7];
    for i in 0..7 {
        let byte = dea[2 + i];
        if !byte.is_ascii_digit() {
            return false;
        }
        digits[i] = (byte - b'0') as u32;
    }

    // Check-digit validation
    let odd_sum = digits[0] + digits[2] + digits[4];   // d1 + d3 + d5
    let even_sum = digits[1] + digits[3] + digits[5];  // d2 + d4 + d6
    let check = (odd_sum + 2 * even_sum) % 10;
    check == digits[6]
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Expired = 1,
    Unauthorized = 2,
    InvalidPrescription = 3,
    AlreadyExists = 4,
    NotFound = 5,
    InvalidSeverity = 6,
    InteractionNotFound = 7,
    MissingOverrideReason = 8,
    InvalidStatusTransition = 9,
    InvalidTransfer = 10,
    QuantityExceeded = 11,
    RefillExceeded = 12,
    PharmacyNotAuthorized = 13,
    TransferChainBroken = 14,
    MissingTransferReason = 15,
    ControlledSubstanceViolation = 16,
    RegistryGoverned = 17,
    HighImpactRequiresProposal = 18,
    ProposalNotFound = 19,
    ProposalAlreadyFinalized = 20,
    /// valid_until must be in the future and within MAX_VALIDITY_WINDOW_SECS of issue time
    InvalidValidityWindow = 21,
    /// Timestamp arithmetic would overflow u64
    TimestampOverflow = 22,
    /// Provider is not registered or active in the provider-registry
    ProviderNotRegistered = 23,
    /// Transfer history has reached the maximum allowed entries
    TransferHistoryFull = 24,
    /// Allergy interaction detected and strict mode is enabled
    AllergyInteractionDetected = 25,
    /// bypass_allergy_check requires admin role
    AllergyBypassRequiresAdmin = 26,
    /// Prescription has already been dispensed or transferred, cannot be recalled
    CannotRecallDispensed = 27,
    /// Recall reason is required for documentation
    MissingRecallReason = 28,
    /// Patient already has MAX_ACTIVE_PRESCRIPTIONS active prescriptions
    TooManyActivePrescriptions = 29,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Medication {
    pub ndc_code: String,
    pub generic_name: String,
    pub brand_names: Vec<String>,
    pub drug_class: Symbol,
    pub interaction_profile_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Interaction {
    pub id: u64,
    pub drug1_ndc: String,
    pub drug2_ndc: String,
    pub severity: Symbol,
    pub interaction_type: Symbol,
    pub clinical_effects: String,
    pub management_strategy: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionWarning {
    pub drug1: String,
    pub drug2: String,
    pub severity: Symbol,
    pub interaction_type: Symbol,
    pub clinical_effects: String,
    pub management: String,
    pub documentation_required: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionOverride {
    pub provider_id: Address,
    pub patient_id: Address,
    pub medication: String,
    pub interaction_id: u64,
    pub override_reason: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryProposalAction {
    Medication(Medication),
    Interaction(Interaction),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryProposal {
    pub id: u64,
    pub proposer: Address,
    pub action: RegistryProposalAction,
    pub created_at: u64,
    pub approved: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogSnapshot {
    pub version: u64,
    pub created_by: Address,
    pub created_at: u64,
    pub medication_count: u32,
    pub interaction_count: u32,
}

#[contracttype]
pub enum DataKey {
    Medication(String),
    MedicationCatalog,
    InteractionCounter,
    InteractionById(u64),
    InteractionPair(String, String),
    InteractionCatalog,
    PatientAllergies(Address),
    PatientConditions(Address),
    MedicationContraindications(String),
    InteractionOverride(u64, Address),
    RegistryAdmin,
    RegistryWriter(Address),
    RegistryProposalCounter,
    RegistryProposal(u64),
    SnapshotCounter,
    CatalogSnapshot(u64),
    /// Address of the provider-registry contract used for cross-contract verification.
    ProviderRegistry,
    /// Address of the allergy-management contract for cross-contract allergy checks.
    AllergyRegistry,
    /// Admin address for elevated operations (e.g. allergy bypass).
    Admin,
    /// If true, allergy interactions block prescription issuance; if false, only alert.
    AllergyStrictMode,
    RecallCounter,
    RecallRecord(u64),
    PrescriptionRecall(u64),
    /// patient -> Vec<u64> of prescription ids issued to them, used to
    /// enforce MAX_ACTIVE_PRESCRIPTIONS. Pruned of no-longer-active ids
    /// every time a new prescription is issued (see
    /// `count_and_prune_active_prescriptions`), so it stays close to the
    /// patient's actual active count rather than growing unboundedly.
    PatientPrescriptions(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrescriptionStatus {
    Issued,
    Active,
    Dispensed,
    PartiallyDispensed,
    Expired,
    Transferred,
    Cancelled,
    Suspended,
    Recalled,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Prescription {
    pub provider_id: Address,
    pub patient_id: Address,
    pub medication_name: String,
    pub quantity: u32,
    pub quantity_dispensed: u32,
    pub refills_allowed: u32,
    pub refills_remaining: u32,
    pub refills_used: u32,
    pub is_controlled: bool,
    pub schedule: Option<u32>, // Controlled substance schedule
    pub current_pharmacy: Option<Address>,
    pub issuing_pharmacy: Option<Address>,
    pub status: PrescriptionStatus,
    pub issued_at: u64,
    pub valid_until: u64,
    pub last_dispensed: Option<u64>,
    pub transfer_count: u32,
    pub transfer_history: Vec<TransferRecord>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TransferRecord {
    pub from_pharmacy: Address,
    pub to_pharmacy: Address,
    pub transfer_reason: String,
    pub transferred_at: u64,
    pub transferred_by: Address,
}

// Struct to bypass the 10-parameter limit
#[contracttype]
pub struct IssueRequest {
    pub medication_name: String,
    pub ndc_code: String,
    pub dosage: String,
    pub quantity: u32,
    pub days_supply: u32,
    pub refills_allowed: u32,
    pub instructions_hash: BytesN<32>,
    pub is_controlled: bool,
    pub schedule: Option<u32>,
    pub valid_until: u64,
    pub substitution_allowed: bool,
    pub pharmacy_id: Option<Address>,
    /// When true, skip allergy check. Requires caller to be the configured admin.
    pub bypass_allergy_check: bool,
    /// DEA registration number of the prescribing provider.
    /// Required when `is_controlled == true` and a `schedule` is present.
    /// Format: 2 uppercase letters followed by 7 digits (e.g. "AB1234563").
    pub dea_number: Option<String>,
    /// Required (non-None) when `bypass_allergy_check == true`.
    /// Hash of the documented clinical justification for the override.
    pub bypass_reason_hash: Option<BytesN<32>>,
}

/// Template storing all IssueRequest fields except patient and valid_until.
/// Used by `create_template` / `issue_from_template`.
#[contracttype]
pub struct PrescriptionTemplate {
    pub medication_name: String,
    pub ndc_code: String,
    pub dosage: String,
    pub quantity: u32,
    pub days_supply: u32,
    pub refills_allowed: u32,
    pub instructions_hash: BytesN<32>,
    pub is_controlled: bool,
    pub schedule: Option<u32>,
    pub substitution_allowed: bool,
    pub pharmacy_id: Option<Address>,
    pub bypass_allergy_check: bool,
    pub dea_number: Option<String>,
    pub bypass_reason_hash: Option<BytesN<32>>,
}

/// Persisted template record with ownership metadata.
#[contracttype]
pub struct StoredTemplate {
    pub template_id: u64,
    pub provider: Address,
    pub medication_name: String,
    pub ndc_code: String,
    pub dosage: String,
    pub quantity: u32,
    pub days_supply: u32,
    pub refills_allowed: u32,
    pub instructions_hash: BytesN<32>,
    pub is_controlled: bool,
    pub schedule: Option<u32>,
    pub substitution_allowed: bool,
    pub pharmacy_id: Option<Address>,
    pub bypass_allergy_check: bool,
    pub dea_number: Option<String>,
    pub bypass_reason_hash: Option<BytesN<32>>,
}

/// Sliding-window counter for per-provider prescription rate limiting.
#[contracttype]
pub struct PrescriptionWindow {
    pub count: u32,
    pub window_start: u64,
}

#[contracttype]
pub struct TransferRequest {
    pub prescription_id: u64,
    pub to_pharmacy: Address,
    pub transfer_reason: String,
    pub urgency: Symbol,
}

#[contracttype]
pub struct DispenseRequest {
    pub prescription_id: u64,
    pub quantity: u32,
    pub lot: String,
    pub expires_at: u64,
    pub ndc_code: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecallRecord {
    pub recall_id: u64,
    pub prescription_id: u64,
    pub recalled_by: Address,
    pub recall_reason: String,
    pub recall_timestamp: u64,
    pub clinical_justification: String,
}

/// Whether a prescription still counts as "active" toward
/// `MAX_ACTIVE_PRESCRIPTIONS`: its status is still actionable (not
/// dispensed/cancelled/recalled/transferred-away) *and* it hasn't passed
/// its `valid_until` timestamp. Expiry in this contract is passive (no
/// stored state transition marks a prescription `Expired`), so it must be
/// checked against the current ledger time here rather than by status alone.
fn is_prescription_active(prescription: &Prescription, now: u64) -> bool {
    matches!(
        prescription.status,
        PrescriptionStatus::Issued | PrescriptionStatus::Active | PrescriptionStatus::PartiallyDispensed
    ) && prescription.valid_until > now
}

/// Returns the patient's current active-prescription count, and rewrites
/// `DataKey::PatientPrescriptions(patient)` to drop ids that are no longer
/// active (dispensed, cancelled, recalled, transferred away, or expired) --
/// so the index stays close to bounded rather than growing for the
/// patient's entire history. Called from `issue_prescription` before
/// enforcing `MAX_ACTIVE_PRESCRIPTIONS`.
fn count_and_prune_active_prescriptions(env: &Env, patient: &Address) -> u32 {
    let key = DataKey::PatientPrescriptions(patient.clone());
    let ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    let now = env.ledger().timestamp();

    let mut still_active: Vec<u64> = Vec::new(env);
    for id in ids.iter() {
        if let Some(prescription) = env.storage().persistent().get::<u64, Prescription>(&id) {
            if is_prescription_active(&prescription, now) {
                still_active.push_back(id);
            }
        }
    }

    let count = still_active.len();
    env.storage().persistent().set(&key, &still_active);
    count
}

/// Records `prescription_id` as belonging to `patient` in the
/// `PatientPrescriptions` index, used by `count_and_prune_active_prescriptions`.
fn add_patient_prescription(env: &Env, patient: &Address, prescription_id: u64) {
    let key = DataKey::PatientPrescriptions(patient.clone());
    let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    ids.push_back(prescription_id);
    env.storage().persistent().set(&key, &ids);
}

#[contract]
pub struct PrescriptionContract;

#[contractimpl]
impl PrescriptionContract {
    /// Store the provider-registry contract address for cross-contract verification.
    /// Must be called once before issue_prescription is used.
    pub fn initialize(env: Env, provider_registry: Address) -> Result<(), Error> {
        if env.storage().persistent().has(&DataKey::ProviderRegistry) {
            return Err(Error::AlreadyExists);
        }
        env.storage()
            .persistent()
            .set(&DataKey::ProviderRegistry, &provider_registry);
        Ok(())
    }

    /// Configure the allergy-management contract and admin for allergy checks.
    /// `strict_mode`: if true, detected interactions block issuance; if false, only emit alert.
    pub fn configure_allergy_check(
        env: Env,
        admin: Address,
        allergy_registry: Address,
        strict_mode: bool,
    ) -> Result<(), Error> {
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::AllergyRegistry, &allergy_registry);
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::AllergyStrictMode, &strict_mode);
        Ok(())
    }

    pub fn issue_prescription(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        req: IssueRequest,
    ) -> Result<u64, Error> {
        provider_id.require_auth();
        do_issue_prescription(&env, provider_id, patient_id, req)
    }

    /// Adjust the rate-limit cap for a specific provider (admin only).
    pub fn set_provider_limit(
        env: Env,
        admin: Address,
        provider: Address,
        limit: u32,
    ) -> Result<(), Error> {
        admin.require_auth();
        let configured: Option<Address> = env.storage().persistent().get(&DataKey::Admin);
        if configured.as_ref().map_or(true, |a| *a != admin) {
            return Err(Error::Unauthorized);
        }
        env.storage()
            .persistent()
            .set(&DataKey::ProviderPrescriptionLimit(provider), &limit);
        Ok(())
    }

    /// Create a reusable prescription template owned by the calling provider.
    /// Returns the new template_id.
    pub fn create_template(
        env: Env,
        provider: Address,
        template: PrescriptionTemplate,
    ) -> Result<u64, Error> {
        provider.require_auth();

        if let Some(registry_addr) = env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::ProviderRegistry)
        {
            let client = ProviderRegistryClient::new(&env, &registry_addr);
            if !client.is_provider(&provider) {
                return Err(Error::ProviderNotRegistered);
            }
        }

        // Allergy cross-check against allergy-management contract.
        if req.bypass_allergy_check {
            // Bypass requires admin role.
            let admin: Option<Address> = env.storage().persistent().get(&DataKey::Admin);
            match admin {
                Some(ref a) if *a == provider_id => {}
                _ => return Err(Error::AllergyBypassRequiresAdmin),
            }
        } else if let Some(allergy_addr) = env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::AllergyRegistry)
        {
            let allergy_client = AllergyManagementClient::new(&env, &allergy_addr);
            let interactions =
                allergy_client.check_drug_allergy_interaction(&patient_id, &req.medication_name);
            if !interactions.is_empty() {
                // Emit alert for every detected interaction.
                for interaction in interactions.iter() {
                    env.events().publish(
                        (Symbol::new(&env, "allergy_interaction_alert"),),
                        (
                            patient_id.clone(),
                            req.medication_name.clone(),
                            interaction.allergen.clone(),
                            interaction.severity.clone(),
                        ),
                    );
                }
                let strict: bool = env
                    .storage()
                    .persistent()
                    .get(&DataKey::AllergyStrictMode)
                    .unwrap_or(false);
                if strict {
                    return Err(Error::AllergyInteractionDetected);
                }
            }
        }

        // DEA registration check: controlled substances require a valid DEA number.
        if req.is_controlled && req.schedule.is_some() {
            match &req.dea_number {
                None => return Err(Error::ControlledSubstanceViolation),
                Some(dea) => {
                    // A valid DEA number is always exactly 9 ASCII characters.
                    // Reject anything that isn't 9 bytes long before byte extraction.
                    // soroban_sdk::String::len() returns u32.
                    if dea.len() != 9u32 {
                        return Err(Error::ControlledSubstanceViolation);
                    }
                    let mut buf = [0u8; 9];
                    dea.copy_into_slice(&mut buf);
                    if !validate_dea_number(&buf) {
                        return Err(Error::ControlledSubstanceViolation);
                    }
                }
            }
        }

        // #215 – valid_until must be in the future and within a 1-year window
        temporal::must_be_future(&env, req.valid_until)
            .map_err(|_| Error::InvalidValidityWindow)?;
        temporal::within_validity_window(
            env.ledger().timestamp(),
            req.valid_until,
            shared::temporal::MAX_VALIDITY_WINDOW_SECS,
        )
        .map_err(|_| Error::InvalidValidityWindow)?;

        // #478 – cap concurrent active prescriptions per patient.
        if count_and_prune_active_prescriptions(&env, &patient_id) >= MAX_ACTIVE_PRESCRIPTIONS {
            return Err(Error::TooManyActivePrescriptions);
        }

        let id = env
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::TemplateCounter)
            .unwrap_or(0);

        let stored = StoredTemplate {
            template_id,
            provider: provider.clone(),
            medication_name: template.medication_name,
            ndc_code: template.ndc_code,
            dosage: template.dosage,
            quantity: template.quantity,
            days_supply: template.days_supply,
            refills_allowed: template.refills_allowed,
            instructions_hash: template.instructions_hash,
            is_controlled: template.is_controlled,
            schedule: template.schedule,
            substitution_allowed: template.substitution_allowed,
            pharmacy_id: template.pharmacy_id,
            bypass_allergy_check: template.bypass_allergy_check,
            dea_number: template.dea_number,
            bypass_reason_hash: template.bypass_reason_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Template(template_id), &stored);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "ID_COUNTER"), &(id + 1));
        add_patient_prescription(&env, &prescription.patient_id, id);

        do_issue_prescription(&env, provider, patient, req)
    }

    pub fn dispense_prescription(
        env: Env,
        req: DispenseRequest,
        pharmacy_id: Address,
    ) -> Result<(), Error> {
        pharmacy_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&req.prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate prescription is in dispensible state
        if !matches!(
            p.status,
            PrescriptionStatus::Issued
                | PrescriptionStatus::Active
                | PrescriptionStatus::PartiallyDispensed
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Check expiration.
        // Semantics: valid_until is an EXCLUSIVE upper bound — a prescription
        // is considered expired when ledger timestamp >= valid_until.
        // This ensures consistent behaviour at UTC midnight boundaries
        // regardless of sub-second ledger close timing.
        if env.ledger().timestamp() >= p.valid_until {
            return Err(Error::Expired);
        }

        // Validate pharmacy authorization
        if let Some(ref current_pharmacy) = p.current_pharmacy {
            if current_pharmacy != &pharmacy_id {
                return Err(Error::PharmacyNotAuthorized);
            }
        } else {
            // First dispense sets the pharmacy
            p.current_pharmacy = Some(pharmacy_id.clone());
        }

        // Validate quantity constraints
        if p.quantity_dispensed + req.quantity > p.quantity {
            return Err(Error::QuantityExceeded);
        }

        // Controlled substance checks
        if p.is_controlled {
            if let Some(schedule) = p.schedule {
                if schedule == 2 && req.quantity > p.quantity / MIN_REFILL_QUANTITY_DIVISOR {
                    return Err(Error::ControlledSubstanceViolation);
                }
            }
        }

        // Update prescription state
        p.quantity_dispensed += req.quantity;
        p.last_dispensed = Some(env.ledger().timestamp());

        // Update status based on remaining quantity
        if p.quantity_dispensed >= p.quantity {
            p.status = PrescriptionStatus::Dispensed;
        } else {
            p.status = PrescriptionStatus::PartiallyDispensed;
        }

        env.storage().persistent().set(&req.prescription_id, &p);

        // Emit dispense event — quantity omitted to prevent clinical PII exposure on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_dispensed"),),
            (req.prescription_id, pharmacy_id),
        );

        Ok(())
    }

    pub fn transfer_prescription(
        env: Env,
        req: TransferRequest,
        from_pharmacy: Address,
    ) -> Result<(), Error> {
        from_pharmacy.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&req.prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate transfer reason
        if req.transfer_reason.is_empty() {
            return Err(Error::MissingTransferReason);
        }

        // Verify current pharmacy ownership
        if let Some(current_pharmacy) = p.current_pharmacy {
            if current_pharmacy != from_pharmacy {
                return Err(Error::PharmacyNotAuthorized);
            }
        } else {
            return Err(Error::TransferChainBroken);
        }

        // Validate prescription is transferable
        if !matches!(
            p.status,
            PrescriptionStatus::Issued
                | PrescriptionStatus::Active
                | PrescriptionStatus::PartiallyDispensed
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Check expiration.
        // Semantics: valid_until is an EXCLUSIVE upper bound — a prescription
        // is considered expired when ledger timestamp >= valid_until.
        // This ensures consistent behaviour at UTC midnight boundaries
        // regardless of sub-second ledger close timing.
        if env.ledger().timestamp() >= p.valid_until {
            return Err(Error::Expired);
        }

        // Transfer limits for controlled substances
        if p.is_controlled && p.transfer_count >= 1 {
            return Err(Error::ControlledSubstanceViolation);
        }

        // Enforce the transfer-history storage cap before appending.
        if p.transfer_history.len() >= MAX_TRANSFER_HISTORY {
            return Err(Error::TransferHistoryFull);
        }

        // Create transfer record
        let transfer_record = TransferRecord {
            from_pharmacy: from_pharmacy.clone(),
            to_pharmacy: req.to_pharmacy.clone(),
            transfer_reason: req.transfer_reason.clone(),
            transferred_at: env.ledger().timestamp(),
            transferred_by: from_pharmacy.clone(),
        };

        // Update prescription
        p.transfer_history.push_back(transfer_record);
        p.transfer_count += 1;
        p.current_pharmacy = Some(req.to_pharmacy.clone());
        p.status = PrescriptionStatus::Transferred;

        env.storage().persistent().set(&req.prescription_id, &p);

        // Emit transfer event — transfer_reason omitted to avoid free-text PII on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_transferred"),),
            (req.prescription_id, from_pharmacy, req.to_pharmacy),
        );

        Ok(())
    }

    pub fn accept_transfer(
        env: Env,
        prescription_id: u64,
        pharmacy_id: Address,
    ) -> Result<(), Error> {
        pharmacy_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&prescription_id)
            .ok_or(Error::NotFound)?;

        // Verify pharmacy is the destination
        if let Some(ref current_pharmacy) = p.current_pharmacy {
            if current_pharmacy != &pharmacy_id {
                return Err(Error::PharmacyNotAuthorized);
            }
        } else {
            return Err(Error::TransferChainBroken);
        }

        // Validate status
        if !matches!(p.status, PrescriptionStatus::Transferred) {
            return Err(Error::InvalidStatusTransition);
        }

        // Accept transfer and activate prescription
        p.status = PrescriptionStatus::Active;
        env.storage().persistent().set(&prescription_id, &p);

        // Emit acceptance event
        env.events().publish(
            (Symbol::new(&env, "transfer_accepted"),),
            (prescription_id, pharmacy_id),
        );

        Ok(())
    }

    pub fn register_medication(
        env: Env,
        ndc_code: String,
        generic_name: String,
        brand_names: Vec<String>,
        drug_class: Symbol,
        interaction_profile_hash: BytesN<32>,
    ) -> Result<(), Error> {
        if is_registry_governed(&env) {
            return Err(Error::RegistryGoverned);
        }
        put_medication(
            &env,
            Medication {
                ndc_code,
                generic_name,
                brand_names,
                drug_class,
                interaction_profile_hash,
            },
        )
    }

    pub fn initialize_registry_governance(env: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        if is_registry_governed(&env) {
            return Err(Error::AlreadyExists);
        }
        env.storage()
            .persistent()
            .set(&DataKey::RegistryAdmin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::RegistryWriter(admin.clone()), &true);
        Ok(())
    }

    pub fn add_registry_writer(env: Env, admin: Address, writer: Address) -> Result<(), Error> {
        require_registry_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::RegistryWriter(writer), &true);
        Ok(())
    }

    pub fn register_medication_by(
        env: Env,
        writer: Address,
        ndc_code: String,
        generic_name: String,
        brand_names: Vec<String>,
        drug_class: Symbol,
        interaction_profile_hash: BytesN<32>,
    ) -> Result<(), Error> {
        require_registry_writer(&env, &writer)?;
        put_medication(
            &env,
            Medication {
                ndc_code,
                generic_name,
                brand_names,
                drug_class,
                interaction_profile_hash,
            },
        )
    }

    pub fn add_interaction(
        env: Env,
        drug1_ndc: String,
        drug2_ndc: String,
        severity: Symbol,
        interaction_type: Symbol,
        clinical_effects: String,
        management_strategy: String,
    ) -> Result<(), Error> {
        if is_registry_governed(&env) {
            return Err(Error::RegistryGoverned);
        }
        put_interaction(
            &env,
            drug1_ndc,
            drug2_ndc,
            severity,
            interaction_type,
            clinical_effects,
            management_strategy,
        )
    }

    pub fn add_interaction_by(
        env: Env,
        writer: Address,
        drug1_ndc: String,
        drug2_ndc: String,
        severity: Symbol,
        interaction_type: Symbol,
        clinical_effects: String,
        management_strategy: String,
    ) -> Result<(), Error> {
        require_registry_writer(&env, &writer)?;
        if requires_documentation(&env, &severity) {
            return Err(Error::HighImpactRequiresProposal);
        }
        put_interaction(
            &env,
            drug1_ndc,
            drug2_ndc,
            severity,
            interaction_type,
            clinical_effects,
            management_strategy,
        )
    }

    pub fn propose_interaction_update(
        env: Env,
        writer: Address,
        drug1_ndc: String,
        drug2_ndc: String,
        severity: Symbol,
        interaction_type: Symbol,
        clinical_effects: String,
        management_strategy: String,
    ) -> Result<u64, Error> {
        require_registry_writer(&env, &writer)?;
        if !is_valid_severity(&env, &severity) {
            return Err(Error::InvalidSeverity);
        }
        if !medications_exist(&env, &drug1_ndc, &drug2_ndc) {
            return Err(Error::NotFound);
        }
        if env.storage().persistent().has(&DataKey::InteractionPair(
            drug1_ndc.clone(),
            drug2_ndc.clone(),
        )) {
            return Err(Error::AlreadyExists);
        }

        create_registry_proposal(
            &env,
            writer,
            RegistryProposalAction::Interaction(Interaction {
                id: 0,
                drug1_ndc: drug1_ndc.clone(),
                drug2_ndc: drug2_ndc.clone(),
                severity,
                interaction_type,
                clinical_effects,
                management_strategy,
            }),
        )
    }

    pub fn propose_medication_update(
        env: Env,
        writer: Address,
        ndc_code: String,
        generic_name: String,
        brand_names: Vec<String>,
        drug_class: Symbol,
        interaction_profile_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        require_registry_writer(&env, &writer)?;
        let key = DataKey::Medication(ndc_code.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }
        create_registry_proposal(
            &env,
            writer,
            RegistryProposalAction::Medication(Medication {
                ndc_code,
                generic_name,
                brand_names,
                drug_class,
                interaction_profile_hash,
            }),
        )
    }

    pub fn approve_registry_proposal(
        env: Env,
        admin: Address,
        proposal_id: u64,
    ) -> Result<(), Error> {
        require_registry_admin(&env, &admin)?;
        let mut proposal: RegistryProposal = env
            .storage()
            .persistent()
            .get(&DataKey::RegistryProposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;
        if proposal.approved {
            return Err(Error::ProposalAlreadyFinalized);
        }
        match proposal.action.clone() {
            RegistryProposalAction::Medication(medication) => put_medication(&env, medication)?,
            RegistryProposalAction::Interaction(interaction) => put_interaction(
                &env,
                interaction.drug1_ndc,
                interaction.drug2_ndc,
                interaction.severity,
                interaction.interaction_type,
                interaction.clinical_effects,
                interaction.management_strategy,
            )?,
        }
        proposal.approved = true;
        env.storage()
            .persistent()
            .set(&DataKey::RegistryProposal(proposal_id), &proposal);
        Ok(())
    }

    pub fn create_catalog_snapshot(env: Env, admin: Address) -> Result<u64, Error> {
        require_registry_admin(&env, &admin)?;
        let version = env
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::SnapshotCounter)
            .unwrap_or(0)
            + 1;
        let medication_count = medication_catalog_len(&env);
        let interaction_count = interaction_catalog_len(&env);
        let snapshot = CatalogSnapshot {
            version,
            created_by: admin,
            created_at: env.ledger().timestamp(),
            medication_count,
            interaction_count,
        };
        env.storage()
            .persistent()
            .set(&DataKey::CatalogSnapshot(version), &snapshot);
        env.storage()
            .instance()
            .set(&DataKey::SnapshotCounter, &version);
        Ok(version)
    }

    pub fn get_catalog_snapshot(env: Env, version: u64) -> Result<CatalogSnapshot, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::CatalogSnapshot(version))
            .ok_or(Error::NotFound)
    }

    pub fn check_interactions(
        env: Env,
        _patient_id: Address,
        new_medication: String,
        current_medications: Vec<String>,
    ) -> Result<Vec<InteractionWarning>, Error> {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Medication(new_medication.clone()))
        {
            return Err(Error::NotFound);
        }

        let mut warnings = Vec::new(&env);
        for current in current_medications {
            let pair_key = DataKey::InteractionPair(new_medication.clone(), current.clone());
            if let Some(interaction_id) = env.storage().persistent().get::<_, u64>(&pair_key) {
                let interaction: Interaction = env
                    .storage()
                    .persistent()
                    .get(&DataKey::InteractionById(interaction_id))
                    .ok_or(Error::InteractionNotFound)?;

                // #302: Validate severity against the explicit allowlist.
                // Silently-invalid severities would bypass requires_documentation(),
                // which always returns false for unrecognised symbols.
                if !is_valid_severity(&env, &interaction.severity) {
                    return Err(Error::InvalidSeverity);
                }

                warnings.push_back(InteractionWarning {
                    drug1: interaction.drug1_ndc,
                    drug2: interaction.drug2_ndc,
                    severity: interaction.severity.clone(),
                    interaction_type: interaction.interaction_type,
                    clinical_effects: interaction.clinical_effects,
                    management: interaction.management_strategy,
                    documentation_required: requires_documentation(&env, &interaction.severity),
                });
            }
        }

        Ok(warnings)
    }

    pub fn check_allergy_interaction(
        env: Env,
        patient_id: Address,
        medication: String,
    ) -> Result<Vec<InteractionWarning>, Error> {
        let med: Medication = env
            .storage()
            .persistent()
            .get(&DataKey::Medication(medication.clone()))
            .ok_or(Error::NotFound)?;

        let allergies: Vec<String> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientAllergies(patient_id))
            .unwrap_or(Vec::new(&env));

        let mut warnings = Vec::new(&env);
        for allergy in allergies {
            let is_brand_match = contains_string(&med.brand_names, &allergy);
            if med.generic_name == allergy || med.ndc_code == allergy || is_brand_match {
                warnings.push_back(InteractionWarning {
                    drug1: med.ndc_code.clone(),
                    drug2: allergy,
                    severity: Symbol::new(&env, "contraindicated"),
                    interaction_type: Symbol::new(&env, "allergy"),
                    clinical_effects: String::from_str(
                        &env,
                        "Potential hypersensitivity or allergic reaction.",
                    ),
                    management: String::from_str(
                        &env,
                        "Avoid medication and prescribe a non-cross-reactive alternative.",
                    ),
                    documentation_required: true,
                });
            }
        }

        Ok(warnings)
    }

    pub fn get_contraindications(
        env: Env,
        patient_id: Address,
        medication: String,
        conditions: Vec<String>,
    ) -> Result<Vec<String>, Error> {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Medication(medication.clone()))
        {
            return Err(Error::NotFound);
        }

        let mut all_conditions = conditions;
        let patient_conditions: Vec<String> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientConditions(patient_id))
            .unwrap_or(Vec::new(&env));

        for condition in patient_conditions {
            if !contains_string(&all_conditions, &condition) {
                all_conditions.push_back(condition);
            }
        }

        let contraindications: Vec<String> = env
            .storage()
            .persistent()
            .get(&DataKey::MedicationContraindications(medication))
            .unwrap_or(Vec::new(&env));

        let mut matched = Vec::new(&env);
        for contraindication in contraindications {
            if contains_string(&all_conditions, &contraindication) {
                matched.push_back(contraindication);
            }
        }

        Ok(matched)
    }

    pub fn override_interaction_warning(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        medication: String,
        interaction_id: u64,
        override_reason: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        if is_blank(&override_reason) {
            return Err(Error::MissingOverrideReason);
        }

        if !env
            .storage()
            .persistent()
            .has(&DataKey::InteractionById(interaction_id))
        {
            return Err(Error::InteractionNotFound);
        }

        let override_record = InteractionOverride {
            provider_id,
            patient_id: patient_id.clone(),
            medication,
            interaction_id,
            override_reason,
            timestamp: env.ledger().timestamp(),
        };

        env.storage().persistent().set(
            &DataKey::InteractionOverride(interaction_id, patient_id),
            &override_record,
        );

        Ok(())
    }

    pub fn set_patient_allergies(
        env: Env,
        patient_id: Address,
        allergies: Vec<String>,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::PatientAllergies(patient_id), &allergies);
        Ok(())
    }

    pub fn set_patient_conditions(
        env: Env,
        patient_id: Address,
        conditions: Vec<String>,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::PatientConditions(patient_id), &conditions);
        Ok(())
    }

    pub fn set_medication_contraindications(
        env: Env,
        medication: String,
        contraindications: Vec<String>,
    ) -> Result<(), Error> {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Medication(medication.clone()))
        {
            return Err(Error::NotFound);
        }

        env.storage().persistent().set(
            &DataKey::MedicationContraindications(medication),
            &contraindications,
        );
        Ok(())
    }

    pub fn refill_prescription(
        env: Env,
        prescription_id: u64,
        pharmacy_id: Address,
        provider_id: Address,
    ) -> Result<(), Error> {
        pharmacy_id.require_auth();
        provider_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate prescription allows refills
        if p.refills_allowed == 0 {
            return Err(Error::RefillExceeded);
        }

        // Check remaining refills
        if p.refills_remaining == 0 {
            return Err(Error::RefillExceeded);
        }

        // Validate prescription is in refillable state
        if !matches!(
            p.status,
            PrescriptionStatus::Active
                | PrescriptionStatus::PartiallyDispensed
                | PrescriptionStatus::Dispensed
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Check expiration.
        // Semantics: valid_until is an EXCLUSIVE upper bound — a prescription
        // is considered expired when ledger timestamp >= valid_until.
        // This ensures consistent behaviour at UTC midnight boundaries
        // regardless of sub-second ledger close timing.
        if env.ledger().timestamp() >= p.valid_until {
            return Err(Error::Expired);
        }

        // Validate pharmacy authorization
        if let Some(ref current_pharmacy) = p.current_pharmacy {
            if current_pharmacy != &pharmacy_id {
                return Err(Error::PharmacyNotAuthorized);
            }
        } else {
            return Err(Error::PharmacyNotAuthorized);
        }

        // Validate provider authorization
        if p.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        // Decrement refills and reset quantity for new fill
        p.refills_remaining -= 1;
        p.refills_used += 1;
        p.quantity_dispensed = 0;
        p.status = PrescriptionStatus::Active;
        p.last_dispensed = None;

        // Extend validity if needed (30 days from refill). Use checked_add to
        // guard against overflow when ledger timestamp is near u64::MAX.
        let new_valid_until = env
            .ledger()
            .timestamp()
            .checked_add(REFILL_WINDOW_SECS)
            .ok_or(Error::InvalidValidityWindow)?;
        if new_valid_until > p.valid_until {
            p.valid_until = new_valid_until;
        }

        env.storage().persistent().set(&prescription_id, &p);

        // Emit refill event — refills_remaining omitted to avoid clinical detail on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_refilled"),),
            (prescription_id, pharmacy_id, provider_id),
        );

        Ok(())
    }

    pub fn cancel_prescription(
        env: Env,
        prescription_id: u64,
        provider_id: Address,
        reason: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate provider authorization
        if p.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        // Only active or issued prescriptions can be cancelled
        if !matches!(
            p.status,
            PrescriptionStatus::Issued
                | PrescriptionStatus::Active
                | PrescriptionStatus::PartiallyDispensed
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Cannot cancel if already partially dispensed (unless for safety reasons)
        if matches!(p.status, PrescriptionStatus::PartiallyDispensed) && p.quantity_dispensed > 0 {
            if reason != String::from_str(&env, "safety_concern")
                && reason != String::from_str(&env, "adverse_reaction")
            {
                return Err(Error::InvalidStatusTransition);
            }
        }

        p.status = PrescriptionStatus::Cancelled;
        env.storage().persistent().set(&prescription_id, &p);

        // Emit cancellation event — reason omitted to avoid free-text PII on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_cancelled"),),
            (prescription_id, provider_id),
        );

        Ok(())
    }

    /// Recall a prescription before it is dispensed.
    /// This is used when a provider discovers a dosage error, drug interaction, or other safety issue.
    pub fn recall_prescription(
        env: Env,
        prescription_id: u64,
        provider_id: Address,
        recall_reason: String,
        clinical_justification: String,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        if recall_reason == String::from_str(&env, "") {
            return Err(Error::MissingRecallReason);
        }

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate provider authorization
        if p.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        // Can only recall prescriptions that haven't been dispensed
        if matches!(
            p.status,
            PrescriptionStatus::Dispensed | PrescriptionStatus::PartiallyDispensed
        ) {
            if p.quantity_dispensed > 0 {
                return Err(Error::CannotRecallDispensed);
            }
        }

        // Cannot recall if already cancelled, expired, or recalled
        if matches!(
            p.status,
            PrescriptionStatus::Cancelled
                | PrescriptionStatus::Expired
                | PrescriptionStatus::Recalled
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Generate recall record
        let recall_id = env
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::RecallCounter)
            .unwrap_or(0)
            + 1;
        env.storage()
            .instance()
            .set(&DataKey::RecallCounter, &recall_id);

        let recall = RecallRecord {
            recall_id,
            prescription_id,
            recalled_by: provider_id.clone(),
            recall_reason: recall_reason.clone(),
            recall_timestamp: env.ledger().timestamp(),
            clinical_justification,
        };

        // Store the recall record
        env.storage()
            .persistent()
            .set(&DataKey::RecallRecord(recall_id), &recall);

        // Link recall to prescription
        env.storage()
            .persistent()
            .set(&DataKey::PrescriptionRecall(prescription_id), &recall_id);

        // Update prescription status to Recalled
        p.status = PrescriptionStatus::Recalled;
        env.storage().persistent().set(&prescription_id, &p);

        // Emit recall event — clinical_justification omitted to avoid PII on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_recalled"),),
            (prescription_id, provider_id, recall_reason),
        );

        Ok(recall_id)
    }

    /// Retrieve recall information for a prescription.
    pub fn get_prescription_recall(env: Env, prescription_id: u64) -> Result<RecallRecord, Error> {
        let recall_id = env
            .storage()
            .persistent()
            .get::<_, u64>(&DataKey::PrescriptionRecall(prescription_id))
            .ok_or(Error::NotFound)?;

        env.storage()
            .persistent()
            .get(&DataKey::RecallRecord(recall_id))
            .ok_or(Error::NotFound)
    }

    /// Check if a prescription has been recalled.
    pub fn is_prescription_recalled(env: Env, prescription_id: u64) -> bool {
        if let Some(recall_id) = env
            .storage()
            .persistent()
            .get::<_, u64>(&DataKey::PrescriptionRecall(prescription_id))
        {
            return env
                .storage()
                .persistent()
                .has(&DataKey::RecallRecord(recall_id));
        }
        false
    }

    /// Retrieve the medication names of all active prescriptions for a patient.
    /// Used by cross-contract callers (e.g. nutrition-care-management) to check
    /// for drug-nutrient contraindications.
    pub fn get_patient_active_prescriptions(env: Env, patient: Address) -> Vec<String> {
        let now = env.ledger().timestamp();
        let key = DataKey::PatientPrescriptions(patient);
        let ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(&env));

        let mut active_meds: Vec<String> = Vec::new(&env);
        for id in ids.iter() {
            if let Some(prescription) = env.storage().persistent().get::<_, Prescription>(&id) {
                if is_prescription_active(&prescription, now) {
                    active_meds.push_back(prescription.medication_name);
                }
            }
        }
        active_meds
    }
}

/// Inner implementation shared by `issue_prescription` and `issue_from_template`.
/// Handles: validity window checks, DEA validation, allergy bypass audit (#481),
/// rate-limit enforcement (#480), prescription storage and event emission.
fn do_issue_prescription(
    env: &Env,
    provider_id: Address,
    patient_id: Address,
    req: IssueRequest,
) -> Result<u64, Error> {
    // ── #481: bypass_allergy_check requires a non-empty reason hash ───────────
    if req.bypass_allergy_check {
        if req.bypass_reason_hash.is_none() {
            return Err(Error::MissingOverrideReason);
        }
    }

    // ── Provider registration check ───────────────────────────────────────────
    if let Some(registry_addr) = env
        .storage()
        .persistent()
        .get::<_, Address>(&DataKey::ProviderRegistry)
    {
        let client = ProviderRegistryClient::new(env, &registry_addr);
        if !client.is_provider(&provider_id) {
            return Err(Error::ProviderNotRegistered);
        }
    }

    // ── Validity window ───────────────────────────────────────────────────────
    temporal::must_be_future(env, req.valid_until)
        .map_err(|_| Error::InvalidValidityWindow)?;
    temporal::within_validity_window(
        env.ledger().timestamp(),
        req.valid_until,
        temporal::MAX_VALIDITY_WINDOW_SECS,
    )
    .map_err(|_| Error::InvalidValidityWindow)?;

    // ── DEA validation for scheduled controlled substances ────────────────────
    if req.is_controlled {
        if let Some(_schedule) = req.schedule {
            match &req.dea_number {
                Some(dea) => {
                    if !validate_dea_number(dea.to_bytes().as_slice()) {
                        return Err(Error::ControlledSubstanceViolation);
                    }
                }
                None => return Err(Error::ControlledSubstanceViolation),
            }
        }
    }

    // ── #480: rate-limit enforcement ──────────────────────────────────────────
    let limit: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::ProviderPrescriptionLimit(provider_id.clone()))
        .unwrap_or(DEFAULT_PRESCRIPTION_LIMIT);

    let now = env.ledger().timestamp();
    let window_key = DataKey::ProviderPrescriptionWindow(provider_id.clone());
    let mut pw: PrescriptionWindow = env
        .storage()
        .persistent()
        .get(&window_key)
        .unwrap_or(PrescriptionWindow { count: 0, window_start: now });

    if now.saturating_sub(pw.window_start) >= RATE_LIMIT_WINDOW_SECS {
        pw.count = 0;
        pw.window_start = now;
    }

    if pw.count >= limit {
        return Err(Error::RateLimitExceeded);
    }
    pw.count += 1;
    env.storage().persistent().set(&window_key, &pw);

    // ── Allergy check (cross-contract) ────────────────────────────────────────
    if let Some(allergy_addr) = env
        .storage()
        .persistent()
        .get::<_, Address>(&DataKey::AllergyRegistry)
    {
        let strict: bool = env
            .storage()
            .persistent()
            .get(&DataKey::AllergyStrictMode)
            .unwrap_or(false);

        let allergy_client = AllergyManagementClient::new(env, &allergy_addr);
        let interactions = allergy_client
            .check_drug_allergy_interaction(&patient_id, &req.medication_name);

        if !interactions.is_empty() {
            if req.bypass_allergy_check {
                // ── #481: emit AllergyBypassApproved audit event ──────────────
                let allergen = interactions.get(0).unwrap().allergen.clone();
                let justification_hash = req.bypass_reason_hash.clone().unwrap();
                env.events().publish(
                    (Symbol::new(env, "AllergyBypassApproved"),),
                    (
                        provider_id.clone(),
                        patient_id.clone(),
                        allergen,
                        justification_hash,
                        now,
                    ),
                );
            } else if strict {
                return Err(Error::AllergyInteractionDetected);
            }
        }
    }

    // ── Assign prescription ID and store ─────────────────────────────────────
    let prescription_id = env
        .storage()
        .instance()
        .get::<_, u64>(&DataKey::TemplateCounter) // reuse instance counter slot
        .unwrap_or(0);
    // Use a dedicated prescription counter separate from template counter
    let rx_id: u64 = env
        .storage()
        .instance()
        .get::<_, u64>(&Symbol::new(env, "RxCounter"))
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&Symbol::new(env, "RxCounter"), &(rx_id + 1));

    let prescription = Prescription {
        provider_id: provider_id.clone(),
        patient_id: patient_id.clone(),
        medication_name: req.medication_name.clone(),
        quantity: req.quantity,
        quantity_dispensed: 0,
        refills_allowed: req.refills_allowed,
        refills_remaining: req.refills_allowed,
        refills_used: 0,
        is_controlled: req.is_controlled,
        schedule: req.schedule,
        current_pharmacy: req.pharmacy_id.clone(),
        issuing_pharmacy: req.pharmacy_id,
        status: PrescriptionStatus::Issued,
        issued_at: now,
        valid_until: req.valid_until,
        last_dispensed: None,
        transfer_count: 0,
        transfer_history: Vec::new(env),
    };

    env.storage().persistent().set(&rx_id, &prescription);

    env.events().publish(
        (Symbol::new(env, "prescription_issued"),),
        (rx_id, provider_id, patient_id),
    );

    Ok(rx_id)
}

fn is_registry_governed(env: &Env) -> bool {
    env.storage().persistent().has(&DataKey::RegistryAdmin)
}

fn require_registry_admin(env: &Env, admin: &Address) -> Result<(), Error> {
    admin.require_auth();
    let configured_admin: Address = env
        .storage()
        .persistent()
        .get(&DataKey::RegistryAdmin)
        .ok_or(Error::Unauthorized)?;
    if configured_admin != *admin {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

fn require_registry_writer(env: &Env, writer: &Address) -> Result<(), Error> {
    writer.require_auth();
    if !is_registry_governed(env) {
        return Err(Error::Unauthorized);
    }
    let authorized = env
        .storage()
        .persistent()
        .get::<_, bool>(&DataKey::RegistryWriter(writer.clone()))
        .unwrap_or(false);
    if !authorized {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

fn put_medication(env: &Env, medication: Medication) -> Result<(), Error> {
    let key = DataKey::Medication(medication.ndc_code.clone());
    if env.storage().persistent().has(&key) {
        return Err(Error::AlreadyExists);
    }

    let mut catalog: Vec<String> = env
        .storage()
        .persistent()
        .get(&DataKey::MedicationCatalog)
        .unwrap_or(Vec::new(env));
    catalog.push_back(medication.ndc_code.clone());

    env.storage().persistent().set(&key, &medication);
    env.storage()
        .persistent()
        .set(&DataKey::MedicationCatalog, &catalog);
    Ok(())
}

fn put_interaction(
    env: &Env,
    drug1_ndc: String,
    drug2_ndc: String,
    severity: Symbol,
    interaction_type: Symbol,
    clinical_effects: String,
    management_strategy: String,
) -> Result<(), Error> {
    if !is_valid_severity(env, &severity) {
        return Err(Error::InvalidSeverity);
    }

    if !medications_exist(env, &drug1_ndc, &drug2_ndc) {
        return Err(Error::NotFound);
    }

    let pair_key = DataKey::InteractionPair(drug1_ndc.clone(), drug2_ndc.clone());
    if env.storage().persistent().has(&pair_key) {
        return Err(Error::AlreadyExists);
    }

    let interaction_id = env
        .storage()
        .instance()
        .get::<_, u64>(&DataKey::InteractionCounter)
        .unwrap_or(0)
        + 1;

    let interaction = Interaction {
        id: interaction_id,
        drug1_ndc: drug1_ndc.clone(),
        drug2_ndc: drug2_ndc.clone(),
        severity,
        interaction_type,
        clinical_effects,
        management_strategy,
    };

    let mut catalog: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::InteractionCatalog)
        .unwrap_or(Vec::new(env));
    catalog.push_back(interaction_id);

    env.storage()
        .persistent()
        .set(&DataKey::InteractionById(interaction_id), &interaction);
    env.storage().persistent().set(
        &DataKey::InteractionPair(drug1_ndc.clone(), drug2_ndc.clone()),
        &interaction_id,
    );
    env.storage().persistent().set(
        &DataKey::InteractionPair(drug2_ndc, drug1_ndc),
        &interaction_id,
    );
    env.storage()
        .persistent()
        .set(&DataKey::InteractionCatalog, &catalog);
    env.storage()
        .instance()
        .set(&DataKey::InteractionCounter, &interaction_id);

    Ok(())
}

fn medications_exist(env: &Env, drug1_ndc: &String, drug2_ndc: &String) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Medication(drug1_ndc.clone()))
        && env
            .storage()
            .persistent()
            .has(&DataKey::Medication(drug2_ndc.clone()))
}

fn create_registry_proposal(
    env: &Env,
    proposer: Address,
    action: RegistryProposalAction,
) -> Result<u64, Error> {
    let id = env
        .storage()
        .instance()
        .get::<_, u64>(&DataKey::RegistryProposalCounter)
        .unwrap_or(0)
        + 1;
    let proposal = RegistryProposal {
        id,
        proposer,
        action,
        created_at: env.ledger().timestamp(),
        approved: false,
    };
    env.storage()
        .persistent()
        .set(&DataKey::RegistryProposal(id), &proposal);
    env.storage()
        .instance()
        .set(&DataKey::RegistryProposalCounter, &id);
    Ok(id)
}

fn medication_catalog_len(env: &Env) -> u32 {
    let catalog: Vec<String> = env
        .storage()
        .persistent()
        .get(&DataKey::MedicationCatalog)
        .unwrap_or(Vec::new(env));
    catalog.len()
}

fn interaction_catalog_len(env: &Env) -> u32 {
    let catalog: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::InteractionCatalog)
        .unwrap_or(Vec::new(env));
    catalog.len()
}

fn is_valid_severity(env: &Env, severity: &Symbol) -> bool {
    *severity == Symbol::new(env, "minor")
        || *severity == Symbol::new(env, "moderate")
        || *severity == Symbol::new(env, "major")
        || *severity == Symbol::new(env, "contraindicated")
}

fn requires_documentation(env: &Env, severity: &Symbol) -> bool {
    *severity == Symbol::new(env, "major") || *severity == Symbol::new(env, "contraindicated")
}

fn is_blank(s: &String) -> bool {
    s.is_empty()
        || s.to_bytes()
            .iter()
            .all(|b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r')
}

fn contains_string(values: &Vec<String>, needle: &String) -> bool {
    for value in values.iter() {
        if value == *needle {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod test;
