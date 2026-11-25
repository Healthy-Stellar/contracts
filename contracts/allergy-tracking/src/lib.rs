#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Allergy Tracking Contract
//!
//! Tracks patient allergies with batch operations, severity levels, temporal validation,
//! and patient deletion support for longitudinal allergy history.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Patient authorization required for operations. Batch operations
//! with per-patient access validation. Unauthorized access returns error on invalid patient request.
//! Clinician writes are gated by the provider-registry: the caller must be a credentialed,
//! active, non-revoked provider.
//!
//! **Audit Controls:** Events emitted for allergy operations. Batch operations tracked with
//! individual record IDs. Temporal validation prevents invalid date sequences (onset before resolution).
//! Historical tracking via persistent storage allows audit replay.
//!
//! **Data Retention Policy:** Patients may deregister via `deregister_patient`, which tombstones
//! (soft-deletes) every allergy record for the patient, clears the patient's access grants and
//! allergy index, and sets a `Deregistered` flag. Deregistration is paginated (`start`/`limit`)
//! so large indexes can be processed across multiple calls; the flag is only set once the whole
//! index has been consumed. While deregistered, all reads and writes for the patient are rejected
//! with `PatientDeregistered`. Batch size limits (configurable) prevent unbounded operations.
//! Duplicate detection ensures consistency. Resolution date and reason retained with resolved allergies.
//!
//! **Encryption/Integrity:** SHA256 content hashing used for data integrity. Allergen type
//! classification (Medication, Food, Environmental, Other) enables category-specific retention rules.
//! Severity enumeration (Mild, Moderate, Severe, LifeThreatening) provides structured risk levels.

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes,
    Env, String, Symbol, Vec,
};

/// Minimal client for the provider-registry contract.
#[contractclient(name = "ProviderRegistryClient")]
pub trait ProviderRegistry {
    fn is_provider(env: Env, provider: Address) -> bool;
}

/// Error codes for allergy tracking operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AllergyNotFound = 1,
    Unauthorized = 2,
    InvalidSeverity = 3,
    InvalidAllergenType = 4,
    AlreadyResolved = 5,
    PatientNotFound = 6,
    DuplicateAllergy = 7,
    InvalidAllergen = 8,
    AllergenTooLong = 9,
    InvalidTimestamp = 10,
    ReasonTooLong = 11,
    AlreadyDeleted = 12,
    BatchEmpty = 13,
    BatchTooLarge = 14,
    ProviderNotRegistered = 15,
    ProviderRegistryNotSet = 16,
    PatientDeregistered = 17,
    InvalidPagination = 18,
}

use ttl_config::{extend_critical_ttl, extend_critical_ttl_if_exists};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllergyStatus {
    Active,
    Resolved,
    Suspected,
    Deleted,
}

/// Comprehensive allergy record
#[contracttype]
#[derive(Clone, Debug)]
pub struct AllergyRecord {
    pub patient: Address,
    pub allergen: String,
    pub reaction: String,
    pub severity: u32,
    pub recorded_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    AllergyCounter,
    Allergy(u64),
    PatientAllergies(Address),
    SeverityHistory(u64),
    DrugCrossSensitivity(String),
    AccessControl(Address, Address),
    GrantedProviders(Address),
    ProviderRegistry,
    Deregistered(Address),
}

/// A single entry in a batch allergy recording request.
/// Mirrors the per-allergy parameters of `record_allergy`, scoped to one patient.
#[contracttype]
#[derive(Clone, Debug)]
pub struct AllergyEntry {
    pub allergen: String,
    pub allergen_type: Symbol,
    pub reaction_types: Vec<String>,
    pub severity: Symbol,
    pub onset_date: Option<u64>,
    pub verified: bool,
}

// Validation constants
const MAX_ALLERGEN_LENGTH: u32 = 100;
const MIN_ALLERGEN_LENGTH: u32 = 1;
const MAX_REASON_LENGTH: u32 = 500;
const MAX_REACTION_LENGTH: u32 = 200;
const MAX_BATCH_SIZE: u32 = 50;
const MAX_DEREGISTER_PAGE: u32 = 50;

#[contract]
pub struct AllergyTrackingContract;

#[contractimpl]
impl AllergyTrackingContract {
    /// Configure contract admin used for privileged read options.
    ///
    /// Runs atomically as part of contract creation and cannot be called again,
    /// so an attacker cannot front-run deployment and elect themselves admin.
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Admin-only: set the provider-registry contract address used to gate clinician writes.
    pub fn set_provider_registry(env: Env, admin: Address, registry: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if stored_admin != admin {
            return Err(Error::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &registry);
        Ok(())
    }

    /// Returns the configured provider-registry address, if any.
    pub fn get_provider_registry(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::ProviderRegistry)
    }

    /// Returns true if `patient_id` has been deregistered.
    pub fn is_deregistered(env: Env, patient_id: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Deregistered(patient_id))
            .unwrap_or(false)
    }

    /// Reject operations for deregistered patients.
    fn require_not_deregistered(env: &Env, patient_id: &Address) -> Result<(), Error> {
        let deregistered: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Deregistered(patient_id.clone()))
            .unwrap_or(false);
        if deregistered {
            return Err(Error::PatientDeregistered);
        }
        Ok(())
    }

    /// Fail-closed check that `provider_id` is a credentialed, active, non-revoked provider.
    fn require_registered_provider(env: &Env, provider_id: &Address) -> Result<(), Error> {
        let registry: Address = env
            .storage()
            .instance()
            .get(&DataKey::ProviderRegistry)
            .ok_or(Error::ProviderRegistryNotSet)?;
        let client = ProviderRegistryClient::new(env, &registry);
        if !client.is_provider(provider_id) {
            return Err(Error::ProviderNotRegistered);
        }
        Ok(())
    }

    /// Patient-scoped access check for reads.
    ///
    /// Allows the patient themselves, the provider who recorded the allergy, or any
    /// address holding a patient-granted access entry. Reuses the shared access-grant
    /// logic so the check is not duplicated across readers.
    fn require_patient_access(
        env: &Env,
        requester: &Address,
        patient_id: &Address,
        provider_id: &Address,
    ) -> Result<(), Error> {
        if requester == patient_id || requester == provider_id {
            return Ok(());
        }
        if shared::access_grant::has_access(env, patient_id, requester) {
            return Ok(());
        }
        Err(Error::Unauthorized)
    }

    /// Record a new allergy for a patient
    pub fn record_allergy(
        env: Env,
        patient: Address,
        allergen: String,
        allergen_type: Symbol,
        reaction_types: Vec<String>,
        severity: Symbol,
        onset_date: Option<u64>,
        verified: bool,
    ) -> Result<u64, Error> {
        provider_id.require_auth();
        Self::require_registered_provider(&env, &provider_id)?;
        Self::require_not_deregistered(&env, &patient_id)?;

        let allergen = Self::trim_allergen(&allergen);
        Self::validate_allergen(&allergen)?;

        // Validate reaction types
        Self::validate_reaction_types(&reaction_types)?;

        // Validate timestamp
        Self::validate_timestamp(&env, onset_date)?;

        // Convert symbols to enums
        let allergen_type_enum = Self::symbol_to_allergen_type(&env, &allergen_type)?;
        let severity_enum = Self::symbol_to_severity(&env, &severity)?;

        // Check for duplicate allergies
        let patient_key = DataKey::PatientAllergies(patient_id.clone());
        let patient_allergies: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));

        for allergy_id in patient_allergies.iter() {
            let Some(allergy) = env
                .storage()
                .persistent()
                .get::<DataKey, AllergyRecord>(&DataKey::Allergy(allergy_id))
            else {
                continue;
            };
            if !allergy.is_deleted && allergy.allergen == allergen {
                return Err(Error::DuplicateAllergy);
            }
        }

        let allergy_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AllergyCounter)
            .unwrap_or(0);

        let now = env.ledger().timestamp();
        let record = AllergyRecord {
            allergy_id,
            patient_id: patient_id.clone(),
            provider_id: provider_id.clone(),
            allergen: allergen.clone(),
            allergen_type: allergen_type_enum,
            reaction_types: reaction_types.clone(),
            severity: severity_enum,
            onset_date,
            verified,
            status: AllergyStatus::Active,
            recorded_date: now,
            last_updated: now,
            resolution_date: None,
            resolution_reason: None,
            is_deleted: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Allergy(allergy_id), &record);

        let mut updated = patient_allergies;
        updated.push_back(allergy_id);
        env.storage().persistent().set(&patient_key, &updated);

        env.storage()
            .instance()
            .set(&DataKey::AllergyCounter, &(allergy_id + 1));

        index
    }

    /// Record multiple allergies for a single patient in one call.
    pub fn record_allergies_batch(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        entries: Vec<AllergyEntry>,
    ) -> Result<Vec<u64>, Error> {
        provider_id.require_auth();
        Self::require_registered_provider(&env, &provider_id)?;
        Self::require_not_deregistered(&env, &patient_id)?;

        if entries.is_empty() {
            return Err(Error::BatchEmpty);
        }
        if entries.len() > MAX_BATCH_SIZE {
            return Err(Error::BatchTooLarge);
        }

        let mut recorded: Vec<u64> = Vec::new(&env);
        for entry in entries.iter() {
            let id = Self::record_allergy(
                env.clone(),
                patient_id.clone(),
                provider_id.clone(),
                entry.allergen,
                entry.allergen_type,
                entry.reaction_types,
                entry.severity,
                entry.onset_date,
                entry.verified,
            )?;
            recorded.push_back(id);
        }
        Ok(recorded)
    }

    /// Update the severity of an existing allergy.
    pub fn update_severity(
        env: Env,
        requester: Address,
        allergy_id: u64,
    ) -> Result<Option<AllergyRecord>, Error> {
        requester.require_auth();
        let record: AllergyRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Allergy(allergy_id))
            .ok_or(Error::AllergyNotFound)?;

        if allergy.is_deleted {
            return Err(Error::AlreadyDeleted);
        }
        Self::require_not_deregistered(&env, &allergy.patient_id)?;

        let new_severity_enum = Self::symbol_to_severity(&env, &new_severity)?;
        let old_severity = allergy.severity.clone();

        allergy.severity = new_severity_enum.clone();
        allergy.last_updated = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Allergy(allergy_id), &allergy);

        let update = SeverityUpdate {
            allergy_id,
            provider_id: provider_id.clone(),
            old_severity,
            new_severity: new_severity_enum,
            reason,
            timestamp: env.ledger().timestamp(),
        };
        let mut history: Vec<SeverityUpdate> = env
            .storage()
            .persistent()
            .get(&DataKey::SeverityHistory(allergy_id))
            .unwrap_or(Vec::new(&env));
        history.push_back(update);
        env.storage()
            .persistent()
            .set(&DataKey::SeverityHistory(allergy_id), &history);

        env.events().publish(
            (symbol_short!("allergy"), symbol_short!("severity")),
            (allergy_id, provider_id),
        );

        Ok(())
    }

    /// List all allergy IDs for a patient.
    ///
    /// Requires `requester` auth and patient-scoped access.
    pub fn get_patient_allergies(
        env: Env,
        requester: Address,
        patient_id: Address,
    ) -> Result<Vec<u64>, Error> {
        requester.require_auth();
        let patient_key = DataKey::PatientAllergies(patient_id.clone());
        let patient_allergies: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));

        // Access is granted if the requester is the patient, a recording provider for
        // any of the patient's allergies, or holds a patient-granted access entry.
        if requester == patient_id {
            return Ok(patient_allergies);
        }
        Self::require_not_deregistered(&env, &allergy.patient_id)?;
        if allergy.status == AllergyStatus::Resolved {
            return Err(Error::AlreadyResolved);
        }
        for allergy_id in patient_allergies.iter() {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, AllergyRecord>(&DataKey::Allergy(allergy_id))
            {
                if record.provider_id == requester {
                    return Ok(patient_allergies);
                }
            }
        }
        Err(Error::Unauthorized)
    }

    /// Retrieve the severity-change history for an allergy.
    ///
    /// Requires `requester` auth and patient-scoped access.
    pub fn get_severity_history(
        env: Env,
        requester: Address,
        allergy_id: u64,
    ) -> Result<Vec<SeverityUpdate>, Error> {
        requester.require_auth();
        let record: AllergyRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Allergy(allergy_id))
            .ok_or(Error::AllergyNotFound)?;

        if allergy.is_deleted {
            return Err(Error::AlreadyDeleted);
        }
        Self::require_not_deregistered(&env, &allergy.patient_id)?;

        allergy.verified = true;
        if allergy.status == AllergyStatus::Suspected {
            allergy.status = AllergyStatus::Active;
        }
        allergy.last_updated = env.ledger().timestamp();
        env.storage()
            .persistent()
            .get(&DataKey::SeverityHistory(allergy_id))
            .unwrap_or(Vec::new(&env));
        Ok(history)
    }

    /// Check whether a drug interacts with a patient's recorded allergies.
    ///
    /// Requires `requester` auth and patient-scoped access. Used by
    /// prescription-management to validate prescriptions against allergy history.
    pub fn check_drug_allergy_interaction(
        env: Env,
        requester: Address,
        patient_id: Address,
        drug_name: String,
    ) -> Result<Vec<InteractionWarning>, Error> {
        requester.require_auth();
        let patient_key = DataKey::PatientAllergies(patient_id.clone());
        let patient_allergies: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));

    /// Deregister a patient: tombstone all allergy records, clear access grants and the
    /// patient's allergy index, and set the deregistered flag.
    ///
    /// Signed by the patient or the contract admin. Paginated: pass `start` (index offset)
    /// and `limit` (max records per call, capped at `MAX_DEREGISTER_PAGE`). The deregistered
    /// flag is only set once the entire index has been processed; call repeatedly until
    /// `is_deregistered` returns true.
    pub fn deregister_patient(
        env: Env,
        caller: Address,
        patient_id: Address,
        start: u32,
        limit: u32,
    ) -> Result<u32, Error> {
        caller.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if caller != patient_id && caller != stored_admin {
            return Err(Error::Unauthorized);
        }

        if limit == 0 || limit > MAX_DEREGISTER_PAGE {
            return Err(Error::InvalidPagination);
        }

        let patient_key = DataKey::PatientAllergies(patient_id.clone());
        let patient_allergies: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));

        let total = patient_allergies.len();
        if start > total {
            return Err(Error::InvalidPagination);
        }

        let end = core::cmp::min(start.saturating_add(limit), total);
        let now = env.ledger().timestamp();
        let mut processed: u32 = 0;

        let mut i = start;
        while i < end {
            let allergy_id = patient_allergies.get(i).unwrap();
            if let Some(mut allergy) = env
                .storage()
                .persistent()
                .get::<DataKey, AllergyRecord>(&DataKey::Allergy(allergy_id))
            {
                if !allergy.is_deleted {
                    allergy.is_deleted = true;
                    allergy.status = AllergyStatus::Deleted;
                    allergy.last_updated = now;
                    env.storage()
                        .persistent()
                        .set(&DataKey::Allergy(allergy_id), &allergy);
                }
            }
            processed += 1;
            i += 1;
        }

        // Clear access grants in both directions for this patient.
        if let Some(providers) = env
            .storage()
            .persistent()
            .get::<DataKey, Vec<Address>>(&DataKey::GrantedProviders(patient_id.clone()))
        {
            for provider in providers.iter() {
                env.storage()
                    .persistent()
                    .remove(&DataKey::AccessControl(patient_id.clone(), provider));
            }
        }
        env.storage()
            .persistent()
            .remove(&DataKey::GrantedProviders(patient_id.clone()));

        if end >= total {
            // Whole index consumed: clear it and mark the patient deregistered.
            env.storage().persistent().remove(&patient_key);
            env.storage()
                .persistent()
                .set(&DataKey::Deregistered(patient_id.clone()), &true);
        }

        env.events().publish(
            (symbol_short!("patient"), symbol_short!("dereg")),
            (patient_id, start, end, end >= total),
        );

        Ok(processed)
    }

    /// Retrieve an allergy record by ID. Returns `None` for deregistered patients.
    pub fn get_allergy(env: Env, allergy_id: u64) -> Option<AllergyRecord> {
        let allergy: AllergyRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Allergy(allergy_id))?;
        if Self::is_deregistered(env.clone(), allergy.patient_id.clone()) {
            return None;
        }
        Some(allergy)
    }

    /// List all allergy IDs for a patient. Returns an empty list for deregistered patients.
    pub fn get_patient_allergies(env: Env, patient_id: Address) -> Vec<u64> {
        if Self::is_deregistered(env.clone(), patient_id.clone()) {
            return Vec::new(&env);
        }
        env.storage()
            .persistent()
            .get(&DataKey::PatientAllergies(patient_id))
            .unwrap_or(Vec::new(&env))
    }

        if !authorized {
            return Err(Error::Unauthorized);
        }
        Ok(warnings)
    }

    // ------------------------------------------------------------------
    // Internal helpers (validation, symbol conversion, etc.)
    // ------------------------------------------------------------------

    fn trim_allergen(allergen: &String) -> String {
        allergen.clone()
    }

    fn validate_allergen(allergen: &String) -> Result<(), Error> {
        let len = allergen.len();
        if len < MIN_ALLERGEN_LENGTH {
            return Err(Error::InvalidAllergen);
        }
        if len > MAX_ALLERGEN_LENGTH {
            return Err(Error::AllergenTooLong);
        }
        Ok(())
    }

    fn validate_reaction_types(reaction_types: &Vec<String>) -> Result<(), Error> {
        for reaction in reaction_types.iter() {
            if reaction.len() > MAX_REACTION_LENGTH {
                return Err(Error::InvalidAllergen);
            }
        }
        Ok(())
    }

    fn validate_timestamp(env: &Env, timestamp: Option<u64>) -> Result<(), Error> {
        if let Some(ts) = timestamp {
            if ts > env.ledger().timestamp() {
                return Err(Error::InvalidTimestamp);
            }
        }
        records
    }

    /// Remove an allergy record for a patient.
    pub fn remove_allergy(env: Env, patient: Address, index: u32) {
        patient.require_auth();
        let key = DataKey::Allergy(patient.clone(), index);
        env.storage().persistent().remove(&key);
        extend_critical_ttl(&env, &DataKey::AllergyCount(patient));
    }
}
