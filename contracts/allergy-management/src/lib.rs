#![no_std]

//! # Allergy Management Contract
//!
//! Records, tracks, and manages patient allergies with severity levels, resolution tracking,
//! drug-allergy interaction checking, and provider access control.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Provider registration verification via external provider registry.
//! Patient access grants to authorized providers only. Access permission checks on allergy retrieval.
//! Patient auth required for access grant/revoke operations.
//!
//! **Audit Controls:** Events emitted for all allergy operations (AllergyRecorded, AllergyUpdated,
//! AllergyResolved, AccessGranted, AccessRevoked, IncidentCaptured). Severity history maintained
//! for each allergy with timestamp and update reason. Incident capture with structured evidence
//! attachment (error_log, state_snapshot, stack_trace, context).
//!
//! **Data Retention Policy:** Active allergies marked with AllergyStatus::Active; resolved allergies
//! retain resolution date and reason. Deregister_patient marks all patient allergies as Deleted,
//! removes PatientAllergies index, and clears access control grants.
//!
//! **Encryption/Integrity:** Allergen and reaction data stored in persistent storage with versioning.
//! Incident tracking with SHA256 hashing for evidence integrity. Provider registry validation
//! ensures only authorized providers access allergy records.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, symbol_short, vec, Address, Bytes, Env,
    IntoVal, String, Symbol, Vec,
};
use shared::{events::EVENT_VERSION, temporal, incident_tracking};

mod storage;
mod types;
mod validation;

pub use storage::*;
pub use types::*;

/// Events for allergy management operations
/// All events carry `version: EVENT_VERSION` for deterministic schema identification.
#[contractevent]
pub struct AllergyRecorded {
    pub version: u32,
    pub patient_id: Address,
    pub allergy_id: u64,
}

#[contractevent]
pub struct AllergyUpdated {
    pub version: u32,
    pub allergy_id: u64,
    pub new_severity: Symbol,
}

#[contractevent]
pub struct AllergyResolved {
    pub version: u32,
    pub allergy_id: u64,
    pub resolution_date: u64,
}

#[contractevent]
pub struct AccessGranted {
    pub version: u32,
    pub patient_id: Address,
    pub provider_id: Address,
}

#[contractevent]
pub struct AccessRevoked {
    pub version: u32,
    pub patient_id: Address,
    pub provider_id: Address,
}

#[contractevent]
pub struct IncidentCaptured {
    pub version: u32,
    pub incident_id: u64,
    pub severity: Symbol,
    pub contract: String,
}

/// Error codes for allergy management operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AllergyNotFound = 1,
    Unauthorized = 2,
    InvalidSeverity = 3,
    InvalidAllergenType = 4,
    AlreadyResolved = 5,
    InvalidDate = 6,
    DuplicateAllergy = 7,
    AccessDenied = 8,
    AlreadyInitialized = 9,
}

#[contract]
pub struct AllergyManagement;

#[contractimpl]
impl AllergyManagement {
    /// Configure the contract with an admin address and the registry addresses
    /// it depends on.
    ///
    /// Runs atomically as part of contract creation and cannot be called again,
    /// so an attacker cannot front-run deployment and elect themselves admin.
    pub fn __constructor(
        env: Env,
        admin: Address,
        patient_registry: Address,
        provider_registry: Address,
        hospital_registry: Address,
        insurer_registry: Address,
    ) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PatientRegistry, &patient_registry);
        env.storage().instance().set(&DataKey::ProviderRegistry, &provider_registry);
        env.storage().instance().set(&DataKey::HospitalRegistry, &hospital_registry);
        env.storage().instance().set(&DataKey::InsurerRegistry, &insurer_registry);
        env.storage()
            .instance()
            .set(&DataKey::AllergyCounter, &0u64);
    }

    /// Record a new allergy for a patient
    pub fn record_allergy(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        request: RecordAllergyRequest,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        // Verify provider is registered
        if !Self::is_registered_provider(&env, &provider_id) {
            return Err(Error::Unauthorized);
        }

        // Validate inputs
        validation::validate_allergen_type(&request.allergen_type)?;
        validation::validate_severity(&request.severity)?;

        // #215 – onset_date must not be in the future (it records a past event)
        if let Some(onset) = request.onset_date {
            temporal::not_future(&env, onset).map_err(|_| Error::InvalidDate)?;
        }

        // Check for duplicate allergy
        if storage::check_duplicate_allergy(
            &env,
            &patient_id,
            &request.allergen,
            &request.allergen_type,
        ) {
            return Err(Error::DuplicateAllergy);
        }

        // Generate unique allergy ID
        let allergy_id = storage::get_next_allergy_id(&env);

        let allergy = AllergyRecord {
            allergy_id,
            patient_id: patient_id.clone(),
            provider_id: provider_id.clone(),
            allergen: request.allergen.clone(),
            allergen_type: request.allergen_type.clone(),
            reaction_type: request.reaction_type.clone(),
            severity: request.severity.clone(),
            onset_date: request.onset_date,
            recorded_date: env.ledger().timestamp(),
            verified: request.verified,
            status: AllergyStatus::Active,
            resolution_date: None,
            resolution_reason: None,
            severity_history: Vec::new(&env),
        };

        // Store allergy record
        storage::save_allergy(&env, &allergy);
        storage::add_patient_allergy(&env, &patient_id, allergy_id);

        // Emit event
        AllergyRecorded {
            version: EVENT_VERSION,
            patient_id: patient_id.clone(),
            allergy_id,
        }
        .publish(&env);

        Ok(allergy_id)
    }

    /// Grant another address (provider, guardian or contract such as
    /// prescription-management) read access to a patient's allergy records.
    /// Authorized by the patient.
    pub fn grant_access(env: Env, patient_id: Address, provider_id: Address) -> Result<(), Error> {
        patient_id.require_auth();

        storage::grant_access(&env, &patient_id, &provider_id);

        AccessGranted {
            version: EVENT_VERSION,
            patient_id,
            provider_id,
        }
        .publish(&env);

        Ok(())
    }

    /// Revoke a previously granted access. Authorized by the patient.
    pub fn revoke_access(env: Env, patient_id: Address, provider_id: Address) -> Result<(), Error> {
        patient_id.require_auth();

        storage::revoke_access(&env, &patient_id, &provider_id);

        AccessRevoked {
            version: EVENT_VERSION,
            patient_id,
            provider_id,
        }
        .publish(&env);

        Ok(())
    }

    /// Check a drug against a patient's active medication allergies and return
    /// the detected interactions.
    ///
    /// The caller must authorize as `requester` and `requester` must already
    /// hold read access to the patient's records (the patient, the admin, or an
    /// explicitly granted address — e.g. the prescription-management contract).
    pub fn check_drug_allergy_interaction(
        env: Env,
        patient_id: Address,
        requester: Address,
        drug_name: String,
    ) -> Result<Vec<AllergyInteraction>, Error> {
        requester.require_auth();

        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::Unauthorized);
        }

        if !storage::check_access_permission(&env, &patient_id, &requester) {
            return Err(Error::Unauthorized);
        }

        let mut interactions: Vec<AllergyInteraction> = Vec::new(&env);

        for allergy_id in storage::get_patient_allergies(&env, &patient_id).iter() {
            let allergy = match storage::get_allergy(&env, allergy_id) {
                Ok(allergy) => allergy,
                Err(_) => continue,
            };

            // Only active medication allergies can interact with a drug.
            if !allergy.status.is_active() || allergy.allergen_type != symbol_short!("med") {
                continue;
            }

            let interaction_type = if validation::check_drug_match(&allergy.allergen, &drug_name) {
                symbol_short!("direct")
            } else if validation::check_cross_sensitivity(&env, &allergy.allergen, &drug_name) {
                symbol_short!("cross")
            } else {
                continue;
            };

            interactions.push_back(AllergyInteraction {
                allergy_id,
                allergen: allergy.allergen.clone(),
                severity: allergy.severity.clone(),
                reaction_type: allergy.reaction_type.clone(),
                interaction_type,
            });
        }

        Ok(interactions)
    }

    /// Check if an address is a registered provider
    fn is_registered_provider(env: &Env, provider_id: &Address) -> bool {
        let provider_registry: Address = env.storage().instance().get(&DataKey::ProviderRegistry).unwrap();
        let args = vec![&env, provider_id.clone().into_val(env)];
        env.invoke_contract(&provider_registry, &Symbol::new(env, "is_provider"), args)
    }

    /// Verify that `provider_id` is authorized to mutate the given allergy record.
    ///
    /// A provider may mutate an allergy only if they recorded it themselves or the
    /// patient has explicitly granted them access, matching the read-path access model
    /// (`storage::check_access_permission`). This prevents any globally-registered
    /// provider from altering or resolving another patient's allergy record.
    fn require_allergy_access(
        env: &Env,
        allergy: &AllergyRecord,
        provider_id: &Address,
    ) -> Result<(), Error> {
        if provider_id == &allergy.provider_id {
            return Ok(());
        }
        if storage::check_access_permission(env, &allergy.patient_id, provider_id) {
            return Ok(());
        }
        Err(Error::AccessDenied)
    }

    /// Capture an incident for troubleshooting (structured evidence capture)
    pub fn capture_incident(
        env: Env,
        error_code: u32,
        description: String,
        severity_level: Symbol, // "low", "medium", "high", "critical"
        reporter: Address,
    ) -> Result<u64, Error> {
        reporter.require_auth();

        let severity = if severity_level == symbol_short!("critical") {
            incident_tracking::IncidentSeverity::Critical
        } else if severity_level == symbol_short!("high") {
            incident_tracking::IncidentSeverity::High
        } else if severity_level == symbol_short!("medium") {
            incident_tracking::IncidentSeverity::Medium
        } else {
            incident_tracking::IncidentSeverity::Low
        };

        let incident_id = incident_tracking::capture_incident(
            &env,
            error_code,
            description,
            severity,
            reporter,
        );

        IncidentCaptured {
            version: EVENT_VERSION,
            incident_id,
            severity: severity_level,
            contract: String::from_str(&env, "allergy-management"),
        }
        .publish(&env);

        Ok(incident_id)
    }

    /// Attach structured evidence to an existing incident
    pub fn attach_incident_evidence(
        env: Env,
        incident_id: u64,
        evidence_type: Symbol,
        data: Bytes,
        reporter: Address,
    ) -> Result<(), Error> {
        reporter.require_auth();
        incident_tracking::attach_evidence(&env, incident_id, evidence_type, data, reporter);
        Ok(())
    }

    /// Retrieve details for an incident
    pub fn get_incident_details(
        env: Env,
        incident_id: u64,
    ) -> Result<incident_tracking::Incident, Error> {
        incident_tracking::get_incident(&env, incident_id).ok_or(Error::AllergyNotFound)
    }

    /// Resolve an incident
    pub fn resolve_incident(
        env: Env,
        incident_id: u64,
        resolver: Address,
        resolution_notes: String,
    ) -> Result<(), Error> {
        resolver.require_auth();
        incident_tracking::resolve_incident(&env, incident_id, resolver, resolution_notes);
        Ok(())
    }

    /// Check whether a drug interacts with any of a patient's active allergies
    pub fn check_drug_allergy_interaction(
        env: Env,
        patient_id: Address,
        drug_name: String,
    ) -> Result<bool, Error> {
        let allergy_ids = storage::get_patient_allergies(&env, &patient_id);
        for allergy_id in allergy_ids.iter() {
            if let Ok(allergy) = storage::get_allergy(&env, allergy_id) {
                if allergy.status == AllergyStatus::Active
                    && allergy.allergen == drug_name
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Get all active allergies for a patient
    pub fn get_active_allergies(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<Vec<AllergyRecord>, Error> {
        requester.require_auth();
        if !storage::check_access_permission(&env, &patient_id, &requester) {
            return Err(Error::AccessDenied);
        }
        let allergy_ids = storage::get_patient_allergies(&env, &patient_id);
        let mut result = Vec::new(&env);
        for allergy_id in allergy_ids.iter() {
            if let Ok(allergy) = storage::get_allergy(&env, allergy_id) {
                if allergy.status == AllergyStatus::Active {
                    result.push_back(allergy);
                }
            }
        }
        Ok(result)
    }

    /// Get all allergies (any status) for a patient
    pub fn get_all_allergies(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<Vec<AllergyRecord>, Error> {
        requester.require_auth();
        if !storage::check_access_permission(&env, &patient_id, &requester) {
            return Err(Error::AccessDenied);
        }
        let allergy_ids = storage::get_patient_allergies(&env, &patient_id);
        let mut result = Vec::new(&env);
        for allergy_id in allergy_ids.iter() {
            if let Ok(allergy) = storage::get_allergy(&env, allergy_id) {
                result.push_back(allergy);
            }
        }
        Ok(result)
    }

    /// Retrieve a single allergy record by ID
    pub fn get_allergy(
        env: Env,
        allergy_id: u64,
        requester: Address,
    ) -> Result<AllergyRecord, Error> {
        requester.require_auth();
        let allergy = storage::get_allergy(&env, allergy_id)?;
        if !storage::check_access_permission(&env, &allergy.patient_id, &requester) {
            return Err(Error::AccessDenied);
        }
        Ok(allergy)
    }

    /// Grant a provider access to a patient's allergy records
    pub fn grant_access(
        env: Env,
        patient_id: Address,
        provider_id: Address,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        storage::grant_access(&env, &patient_id, &provider_id);
        AccessGranted {
            version: EVENT_VERSION,
            patient_id,
            provider_id,
        }
        .publish(&env);
        Ok(())
    }

    /// Revoke a provider's access to a patient's allergy records
    pub fn revoke_access(
        env: Env,
        patient_id: Address,
        provider_id: Address,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        storage::revoke_access(&env, &patient_id, &provider_id);
        AccessRevoked {
            version: EVENT_VERSION,
            patient_id,
            provider_id,
        }
        .publish(&env);
        Ok(())
    }

    /// Deregister a patient, marking all allergies as deleted and clearing access grants
    pub fn deregister_patient(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<(), Error> {
        requester.require_auth();
        if requester != patient_id {
            let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
            if requester != admin {
                return Err(Error::Unauthorized);
            }
        }
        let allergy_ids = storage::get_patient_allergies(&env, &patient_id);
        for allergy_id in allergy_ids.iter() {
            if let Ok(mut allergy) = storage::get_allergy(&env, allergy_id) {
                allergy.status = AllergyStatus::Deleted;
                storage::save_allergy(&env, &allergy);
            }
        }
        storage::remove_patient_allergies(&env, &patient_id);
        storage::clear_access_grants(&env, &patient_id);
        Ok(())
    }

    /// Update the severity of an existing allergy record
    pub fn update_allergy_severity(
        env: Env,
        allergy_id: u64,
        provider_id: Address,
        new_severity: Symbol,
        reason: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        if !Self::is_registered_provider(&env, &provider_id) {
            return Err(Error::Unauthorized);
        }

        validation::validate_severity(&new_severity)?;

        let mut allergy = storage::get_allergy(&env, allergy_id)?;

        // #847 – only the recording provider or a patient-granted provider may mutate
        Self::require_allergy_access(&env, &allergy, &provider_id)?;

        if allergy.status == AllergyStatus::Resolved {
            return Err(Error::AlreadyResolved);
        }

        let update = SeverityUpdate {
            previous_severity: allergy.severity.clone(),
            new_severity: new_severity.clone(),
            updated_by: provider_id.clone(),
            updated_at: env.ledger().timestamp(),
            reason,
        };
        allergy.severity_history.push_back(update);
        allergy.severity = new_severity.clone();

        storage::save_allergy(&env, &allergy);

        AllergyUpdated {
            version: EVENT_VERSION,
            allergy_id,
            new_severity,
        }
        .publish(&env);

        Ok(())
    }

    /// Resolve an allergy record
    pub fn resolve_allergy(
        env: Env,
        allergy_id: u64,
        provider_id: Address,
        resolution_reason: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        if !Self::is_registered_provider(&env, &provider_id) {
            return Err(Error::Unauthorized);
        }

        let mut allergy = storage::get_allergy(&env, allergy_id)?;

        // #847 – only the recording provider or a patient-granted provider may resolve
        Self::require_allergy_access(&env, &allergy, &provider_id)?;

        if allergy.status == AllergyStatus::Resolved {
            return Err(Error::AlreadyResolved);
        }

        let now = env.ledger().timestamp();
        allergy.status = AllergyStatus::Resolved;
        allergy.resolution_date = Some(now);
        allergy.resolution_reason = Some(resolution_reason);

        storage::save_allergy(&env, &allergy);

        AllergyResolved {
            version: EVENT_VERSION,
            allergy_id,
            resolution_date: now,
        }
        .publish(&env);

        Ok(())
    }
}

#[cfg(test)]
mod test;
