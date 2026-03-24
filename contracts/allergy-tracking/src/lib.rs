#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};

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
}

/// Allergen types supported by the system
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllergenType {
    Medication,
    Food,
    Environmental,
    Other,
}

/// Severity levels for allergic reactions
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Severity {
    Mild,
    Moderate,
    Severe,
    LifeThreatening,
}

/// Status of an allergy record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AllergyStatus {
    Active,
    Resolved,
    Suspected,
}

/// Comprehensive allergy record
#[contracttype]
#[derive(Clone, Debug)]
pub struct AllergyRecord {
    pub allergy_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub allergen: String,
    pub allergen_type: AllergenType,
    pub reaction_types: Vec<String>,
    pub severity: Severity,
    pub onset_date: Option<u64>,
    pub verified: bool,
    pub status: AllergyStatus,
    pub recorded_date: u64,
    pub last_updated: u64,
    pub resolution_date: Option<u64>,
    pub resolution_reason: Option<String>,
    pub is_deleted: bool,
}

/// Severity update record for audit trail
#[contracttype]
#[derive(Clone, Debug)]
pub struct SeverityUpdate {
    pub allergy_id: u64,
    pub provider_id: Address,
    pub old_severity: Severity,
    pub new_severity: Severity,
    pub reason: String,
    pub timestamp: u64,
}

/// Drug interaction warning
#[contracttype]
#[derive(Clone, Debug)]
pub struct InteractionWarning {
    pub allergy_id: u64,
    pub allergen: String,
    pub severity: Severity,
    pub reaction_types: Vec<String>,
}

/// Storage keys for the contract
#[contracttype]
pub enum DataKey {
    Admin,
    AllergyCounter,
    Allergy(u64),
    PatientAllergies(Address),
    SeverityHistory(u64),
    DrugCrossSensitivity(String),
}

// Validation constants
const MAX_ALLERGEN_LENGTH: u32 = 100;
const MIN_ALLERGEN_LENGTH: u32 = 1;
const MAX_REASON_LENGTH: u32 = 500;
const MAX_REACTION_LENGTH: u32 = 200;

#[contract]
pub struct AllergyTrackingContract;

#[contractimpl]
impl AllergyTrackingContract {
    /// Configure contract admin used for privileged read options.
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Record a new allergy for a patient
    pub fn record_allergy(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        allergen: String,
        allergen_type: Symbol,
        reaction_types: Vec<String>,
        severity: Symbol,
        onset_date: Option<u64>,
        verified: bool,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        // Validate allergen name
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
            let allergy: AllergyRecord = env
                .storage()
                .persistent()
                .get(&DataKey::Allergy(allergy_id))
                .unwrap();
            if !allergy.is_deleted
                && allergy.allergen == allergen
                && allergy.status != AllergyStatus::Resolved
            {
                return Err(Error::DuplicateAllergy);
            }
        }

        // Generate new allergy ID
        let allergy_id = env
            .storage()
            .instance()
            .get(&DataKey::AllergyCounter)
            .unwrap_or(0u64);

        let current_time = env.ledger().timestamp();

        // Create allergy record
        let allergy = AllergyRecord {
            allergy_id,
            patient_id: patient_id.clone(),
            provider_id: provider_id.clone(),
            allergen: allergen.clone(),
            allergen_type: allergen_type_enum,
            reaction_types: reaction_types.clone(),
            severity: severity_enum.clone(),
            onset_date,
            verified,
            status: if verified {
                AllergyStatus::Active
            } else {
                AllergyStatus::Suspected
            },
            recorded_date: current_time,
            last_updated: current_time,
            resolution_date: None,
            resolution_reason: None,
            is_deleted: false,
        };

        // Store allergy record
        env.storage()
            .persistent()
            .set(&DataKey::Allergy(allergy_id), &allergy);

        // Update patient's allergy list
        let mut patient_allergies: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));
        patient_allergies.push_back(allergy_id);
        env.storage()
            .persistent()
            .set(&patient_key, &patient_allergies);

        // Increment counter
        env.storage()
            .instance()
            .set(&DataKey::AllergyCounter, &(allergy_id + 1));

        // Emit event
        env.events().publish(
            (symbol_short!("allergy"), patient_id, allergy_id),
            allergen,
        );

        Ok(allergy_id)
    }

    /// Update the severity of an existing allergy
    pub fn update_allergy_severity(
        env: Env,
        allergy_id: u64,
        provider_id: Address,
        new_severity: Symbol,
        reason: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        // Validate reason length
        Self::validate_reason(&reason)?;

        let allergy_key = DataKey::Allergy(allergy_id);
        let mut allergy: AllergyRecord = env
            .storage()
            .persistent()
            .get(&allergy_key)
            .ok_or(Error::AllergyNotFound)?;

        if allergy.status == AllergyStatus::Resolved {
            return Err(Error::AlreadyResolved);
        }
        if allergy.is_deleted {
            return Err(Error::AlreadyDeleted);
        }

        let new_severity_enum = Self::symbol_to_severity(&env, &new_severity)?;
        let old_severity = allergy.severity.clone();

        // Create severity update record
        let update = SeverityUpdate {
            allergy_id,
            provider_id: provider_id.clone(),
            old_severity: old_severity.clone(),
            new_severity: new_severity_enum.clone(),
            reason: reason.clone(),
            timestamp: env.ledger().timestamp(),
        };

        // Store severity history
        let history_key = DataKey::SeverityHistory(allergy_id);
        let mut history: Vec<SeverityUpdate> = env
            .storage()
            .persistent()
            .get(&history_key)
            .unwrap_or(Vec::new(&env));
        history.push_back(update);
        env.storage().persistent().set(&history_key, &history);

        // Update allergy record
        allergy.severity = new_severity_enum;
        allergy.last_updated = env.ledger().timestamp();
        env.storage().persistent().set(&allergy_key, &allergy);

        // Emit event
        env.events().publish(
            (symbol_short!("sev_upd"), allergy_id),
            (old_severity, new_severity),
        );

        Ok(())
    }

    /// Resolve an allergy (mark as no longer active)
    pub fn resolve_allergy(
        env: Env,
        allergy_id: u64,
        provider_id: Address,
        resolution_date: u64,
        resolution_reason: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        // Validate reason length
        Self::validate_reason(&resolution_reason)?;
        
        // Validate resolution date
        if resolution_date == 0 || resolution_date > env.ledger().timestamp() {
            return Err(Error::InvalidTimestamp);
        }

        let allergy_key = DataKey::Allergy(allergy_id);
        let mut allergy: AllergyRecord = env
            .storage()
            .persistent()
            .get(&allergy_key)
            .ok_or(Error::AllergyNotFound)?;

        if allergy.status == AllergyStatus::Resolved {
            return Err(Error::AlreadyResolved);
        }
        if allergy.is_deleted {
            return Err(Error::AlreadyDeleted);
        }

        allergy.status = AllergyStatus::Resolved;
        allergy.resolution_date = Some(resolution_date);
        allergy.resolution_reason = Some(resolution_reason.clone());
        allergy.last_updated = env.ledger().timestamp();

        env.storage().persistent().set(&allergy_key, &allergy);

        // Emit event
        env.events().publish(
            (symbol_short!("resolved"), allergy_id),
            resolution_reason,
        );

        Ok(())
    }

    /// Check for drug allergy interactions
    pub fn check_drug_allergy_interaction(
        env: Env,
        patient_id: Address,
        drug_name: String,
    ) -> Result<Vec<InteractionWarning>, Error> {
        let patient_key = DataKey::PatientAllergies(patient_id.clone());
        let patient_allergies: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));

        let mut warnings = Vec::new(&env);

        for allergy_id in patient_allergies.iter() {
            let allergy: AllergyRecord = env
                .storage()
                .persistent()
                .get(&DataKey::Allergy(allergy_id))
                .unwrap();

            // Only check active allergies
            if allergy.is_deleted || allergy.status != AllergyStatus::Active {
                continue;
            }

            // Check for medication allergies
            if matches!(allergy.allergen_type, AllergenType::Medication) {
                // Direct match
                if allergy.allergen == drug_name {
                    let warning = InteractionWarning {
                        allergy_id,
                        allergen: allergy.allergen.clone(),
                        severity: allergy.severity.clone(),
                        reaction_types: allergy.reaction_types.clone(),
                    };
                    warnings.push_back(warning);
                    continue;
                }

                // Check cross-sensitivity
                if Self::check_cross_sensitivity(&env, &allergy.allergen, &drug_name) {
                    let warning = InteractionWarning {
                        allergy_id,
                        allergen: allergy.allergen.clone(),
                        severity: allergy.severity.clone(),
                        reaction_types: allergy.reaction_types.clone(),
                    };
                    warnings.push_back(warning);
                }
            }
        }

        Ok(warnings)
    }

    /// Get all active allergies for a patient
    pub fn get_active_allergies(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<Vec<AllergyRecord>, Error> {
        requester.require_auth();

        let patient_key = DataKey::PatientAllergies(patient_id);
        let patient_allergies: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));

        let mut active_allergies = Vec::new(&env);

        for allergy_id in patient_allergies.iter() {
            let allergy: AllergyRecord = env
                .storage()
                .persistent()
                .get(&DataKey::Allergy(allergy_id))
                .unwrap();

            if !allergy.is_deleted && allergy.status == AllergyStatus::Active {
                active_allergies.push_back(allergy);
            }
        }

        Ok(active_allergies)
    }

    /// Soft-delete a record by ID. Only the record's provider or patient can delete it.
    pub fn delete_record(env: Env, record_id: u64, caller: Address) -> Result<(), Error> {
        caller.require_auth();

        let key = DataKey::Allergy(record_id);
        let mut allergy: AllergyRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::AllergyNotFound)?;

        if caller != allergy.provider_id && caller != allergy.patient_id {
            return Err(Error::Unauthorized);
        }
        if allergy.is_deleted {
            return Err(Error::AlreadyDeleted);
        }

        allergy.is_deleted = true;
        allergy.last_updated = env.ledger().timestamp();
        env.storage().persistent().set(&key, &allergy);

        Ok(())
    }

    /// Get a specific record by ID. Soft-deleted records are treated as not found.
    pub fn get_record(env: Env, record_id: u64) -> Result<AllergyRecord, Error> {
        let allergy: AllergyRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Allergy(record_id))
            .ok_or(Error::AllergyNotFound)?;

        if allergy.is_deleted {
            return Err(Error::AllergyNotFound);
        }

        Ok(allergy)
    }

    /// Get all records for a patient.
    /// By default soft-deleted records are excluded.
    /// include_deleted=true is allowed only for admin.
    pub fn get_all_records(
        env: Env,
        patient_id: Address,
        requester: Address,
        include_deleted: bool,
    ) -> Result<Vec<AllergyRecord>, Error> {
        requester.require_auth();

        if include_deleted && !Self::is_admin(&env, &requester) {
            return Err(Error::Unauthorized);
        }

        let patient_key = DataKey::PatientAllergies(patient_id);
        let patient_allergies: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));

        let mut records = Vec::new(&env);
        for allergy_id in patient_allergies.iter() {
            if let Some(allergy) = env
                .storage()
                .persistent()
                .get::<DataKey, AllergyRecord>(&DataKey::Allergy(allergy_id))
            {
                if !include_deleted && allergy.is_deleted {
                    continue;
                }
                records.push_back(allergy);
            }
        }

        Ok(records)
    }

    /// Get a specific allergy record
    pub fn get_allergy(env: Env, allergy_id: u64) -> Result<AllergyRecord, Error> {
        Self::get_record(env, allergy_id)
    }

    /// Get severity update history for an allergy
    pub fn get_severity_history(env: Env, allergy_id: u64) -> Vec<SeverityUpdate> {
        env.storage()
            .persistent()
            .get(&DataKey::SeverityHistory(allergy_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Register a cross-sensitivity relationship between drugs
    pub fn register_cross_sensitivity(
        env: Env,
        admin: Address,
        drug1: String,
        drug2: String,
    ) -> Result<(), Error> {
        admin.require_auth();

        let key1 = DataKey::DrugCrossSensitivity(drug1.clone());
        let mut related1: Vec<String> = env
            .storage()
            .persistent()
            .get(&key1)
            .unwrap_or(Vec::new(&env));
        
        if !Self::vec_contains(&related1, &drug2) {
            related1.push_back(drug2.clone());
            env.storage().persistent().set(&key1, &related1);
        }

        let key2 = DataKey::DrugCrossSensitivity(drug2.clone());
        let mut related2: Vec<String> = env
            .storage()
            .persistent()
            .get(&key2)
            .unwrap_or(Vec::new(&env));
        
        if !Self::vec_contains(&related2, &drug1) {
            related2.push_back(drug1);
            env.storage().persistent().set(&key2, &related2);
        }

        Ok(())
    }

    // ==================== Helper Functions ====================

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

    fn validate_reaction_types(reactions: &Vec<String>) -> Result<(), Error> {
        for reaction in reactions.iter() {
            if reaction.len() > MAX_REACTION_LENGTH {
                return Err(Error::AllergenTooLong); // Reuse error for simplicity
            }
        }
        Ok(())
    }

    fn validate_reason(reason: &String) -> Result<(), Error> {
        if reason.len() > MAX_REASON_LENGTH {
            return Err(Error::ReasonTooLong);
        }
        Ok(())
    }

    fn validate_timestamp(env: &Env, timestamp: Option<u64>) -> Result<(), Error> {
        if let Some(ts) = timestamp {
            if ts == 0 || ts > env.ledger().timestamp() {
                return Err(Error::InvalidTimestamp);
            }
        }
        Ok(())
    }

    fn symbol_to_allergen_type(env: &Env, symbol: &Symbol) -> Result<AllergenType, Error> {
        if symbol == &Symbol::new(env, "med") || symbol == &Symbol::new(env, "medication") {
            Ok(AllergenType::Medication)
        } else if symbol == &Symbol::new(env, "food") {
            Ok(AllergenType::Food)
        } else if symbol == &Symbol::new(env, "env") || symbol == &Symbol::new(env, "environmental")
        {
            Ok(AllergenType::Environmental)
        } else if symbol == &Symbol::new(env, "other") {
            Ok(AllergenType::Other)
        } else {
            Err(Error::InvalidAllergenType)
        }
    }

    fn symbol_to_severity(env: &Env, symbol: &Symbol) -> Result<Severity, Error> {
        if symbol == &Symbol::new(env, "mild") {
            Ok(Severity::Mild)
        } else if symbol == &Symbol::new(env, "moderate") {
            Ok(Severity::Moderate)
        } else if symbol == &Symbol::new(env, "severe") {
            Ok(Severity::Severe)
        } else if symbol == &Symbol::new(env, "life")
            || symbol == &Symbol::new(env, "life_threatening")
        {
            Ok(Severity::LifeThreatening)
        } else {
            Err(Error::InvalidSeverity)
        }
    }

    fn check_cross_sensitivity(env: &Env, allergen: &String, drug: &String) -> bool {
        let key = DataKey::DrugCrossSensitivity(allergen.clone());
        let related: Vec<String> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(env));

        Self::vec_contains(&related, drug)
    }

    fn vec_contains(vec: &Vec<String>, item: &String) -> bool {
        for v in vec.iter() {
            if v == *item {
                return true;
            }
        }
        false
    }

    fn is_admin(env: &Env, caller: &Address) -> bool {
        let admin: Option<Address> = env.storage().instance().get(&DataKey::Admin);
        match admin {
            Some(stored_admin) => stored_admin == *caller,
            None => false,
        }
    }
}

#[cfg(test)]
mod test;
