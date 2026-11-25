#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

use ttl_config::{extend_critical_ttl, extend_critical_ttl_if_exists};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
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
    Allergy(Address, u32),
    AllergyCount(Address),
}

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

    /// Retrieve a single allergy record.
    ///
    /// Requires `requester` auth and patient-scoped access (patient, recording provider,
    /// or a patient-granted access entry).
    pub fn get_allergy(
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
        Self::require_patient_access(&env, &requester, &record.patient_id, &record.provider_id)?;
        Ok(Some(record))
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
        if shared::access_grant::has_access(&env, &patient_id, &requester) {
            return Ok(patient_allergies);
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
        Self::require_patient_access(&env, &requester, &record.patient_id, &record.provider_id)?;
        let history: Vec<SeverityUpdate> = env
            .storage()
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

        // Access is granted if the requester is the patient, a recording provider for
        // any of the patient's allergies, or holds a patient-granted access entry.
        let mut authorized = requester == patient_id
            || shared::access_grant::has_access(&env, &patient_id, &requester);

        let mut warnings: Vec<InteractionWarning> = Vec::new(&env);
        for allergy_id in patient_allergies.iter() {
            let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, AllergyRecord>(&DataKey::Allergy(allergy_id))
            else {
                continue;
            };
            if record.is_deleted || record.status == AllergyStatus::Resolved {
                continue;
            }
            if record.provider_id == requester {
                authorized = true;
            }
            if record.allergen == drug_name {
                warnings.push_back(InteractionWarning {
                    allergy_id: record.allergy_id,
                    allergen: record.allergen.clone(),
                    severity: record.severity.clone(),
                    reaction_types: record.reaction_types.clone(),
                });
            }
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
