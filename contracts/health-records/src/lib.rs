#![no_std]

use shared::incident_tracking::{
    capture_incident, get_incidents_by_correlation_id as shared_get_by_corr, IncidentSeverity,
};
use shared::privacy::{
    validate_encrypted_ref, validate_nonzero_address, validate_policy_metadata, EncryptedEnvelopeRef, PolicyMetadata,
};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env,
    String, Vec,
};

/// Maximum number of records allowed in a single `create_records_batch` call.
pub const MAX_BATCH_SIZE: u32 = 10;

/// Per-provider consent scope granted by a patient.
///
/// `expires_at == 0` is treated as "never expires".
/// Any non-zero value is compared against the current ledger timestamp;
/// if `expires_at <= timestamp` the consent is considered expired.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentScope {
    pub can_read: bool,
    pub can_write: bool,
    pub can_share: bool,
    pub expires_at: u64,
}

/// Category of a medical record, used for type-safe queries and
/// category-specific retention rules. Replaces the previous free-form
/// `record_type: String` field (see `record_description` for the
/// human-readable free text that used to live there).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordCategory {
    Lab = 0,
    Imaging = 1,
    Consultation = 2,
    Prescription = 3,
    Discharge = 4,
    Vaccination = 5,
    Other = 6,
}

/// Input record for `create_records_batch`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordInput {
    pub patient: Address,
    pub encrypted_ref: EncryptedEnvelopeRef,
    pub record_category: RecordCategory,
    pub record_description: Option<String>,
    pub policy: PolicyMetadata,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedicalRecord {
    pub record_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub encrypted_ref: EncryptedEnvelopeRef,
    pub record_category: RecordCategory,
    pub record_description: Option<String>,
    pub timestamp: u64,
    pub integrity_hash: BytesN<32>,
    pub policy: PolicyMetadata,
    pub version: u32,
}

#[contracttype]
pub enum DataKey {
    Record(u64),
    RecordCounter,
    Consent(Address, Address),     // (patient, provider) -> ConsentScope
    PatientProviders(Address),     // patient -> Vec<Address> of consented providers
    CategoryIndex(RecordCategory), // category -> Vec<u64> of record ids in that category, for prefix-style queries
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    RecordNotFound = 1,
    Unauthorized = 2,
    ConsentNotGranted = 3,
    InvalidEncryptedEnvelope = 4,
    InvalidPolicyMetadata = 5,
    InvalidAddress = 6,
    BatchTooLarge = 7,
}

/// Append `record_category`'s record ID to its `CategoryIndex` so
/// `get_records_by_category` can look up records of a given category
/// without scanning every record.
fn index_record_by_category(env: &Env, category: RecordCategory, record_id: u64) {
    let key = DataKey::CategoryIndex(category);
    let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or(Vec::new(env));
    ids.push_back(record_id);
    env.storage().persistent().set(&key, &ids);
}

fn compute_hash(
    env: &Env,
    record_id: u64,
    patient: &Address,
    provider: &Address,
    encrypted_ref: &EncryptedEnvelopeRef,
    record_category: RecordCategory,
    record_description: &Option<String>,
    timestamp: u64,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_array(&record_id.to_be_bytes());
    let patient_bytes = patient.clone().to_xdr(env);
    data.append(&patient_bytes);
    let provider_bytes = provider.clone().to_xdr(env);
    data.append(&provider_bytes);
    data.append(&Bytes::from(encrypted_ref.content_hash.clone()));
    data.extend_from_array(&(record_category as u32).to_be_bytes());
    if let Some(description) = record_description {
        data.append(&description.clone().to_xdr(env));
    }
    data.extend_from_array(&timestamp.to_be_bytes());
    env.crypto().sha256(&data).into()
}

/// Returns the active `ConsentScope` for `(patient, provider)`, or `None` if
/// no consent exists or the consent has expired.
fn get_active_consent(env: &Env, patient: &Address, provider: &Address) -> Option<ConsentScope> {
    let scope: Option<ConsentScope> = env
        .storage()
        .persistent()
        .get(&DataKey::Consent(patient.clone(), provider.clone()));
    scope.and_then(|s| {
        if s.expires_at == 0 || s.expires_at > env.ledger().timestamp() {
            Some(s)
        } else {
            None
        }
    })
}

#[contract]
pub struct HealthRecords;

#[contractimpl]
impl HealthRecords {
    /// Patient grants a provider scoped consent to act on their records.
    ///
    /// `scope.expires_at == 0` means the consent never expires.
    pub fn grant_consent(
        env: Env,
        patient: Address,
        provider: Address,
        scope: ConsentScope,
    ) -> Result<(), Error> {
        validate_nonzero_address(&patient).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        patient.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Consent(patient, provider), &scope);
        Ok(())
    }

    /// Patient revokes all consent for a provider.
    pub fn revoke_consent(env: Env, patient: Address, provider: Address) -> Result<(), Error> {
        validate_nonzero_address(&patient).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        patient.require_auth();
        env.storage()
            .persistent()
            .remove(&DataKey::Consent(patient, provider));
        Ok(())
    }

    /// Remove all health-records state for a deregistered patient.
    ///
    /// Revokes consent for every provider that was ever granted access.
    /// Only callable by the patient themselves (they must auth before
    /// deregistering from patient-registry).
    pub fn deregister_patient(env: Env, patient: Address) {
        patient.require_auth();
        let idx_key = DataKey::PatientProviders(patient.clone());
        let providers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));
        for provider in providers.iter() {
            env.storage()
                .persistent()
                .remove(&DataKey::Consent(patient.clone(), provider));
        }
        env.storage().persistent().remove(&idx_key);
    }

    /// Create a record. Requires both patient and provider auth, plus active write consent.
    pub fn create_record(
        env: Env,
        patient: Address,
        provider: Address,
        encrypted_ref: EncryptedEnvelopeRef,
        record_category: RecordCategory,
        record_description: Option<String>,
        policy: PolicyMetadata,
    ) -> Result<u64, Error> {
        validate_nonzero_address(&patient).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        patient.require_auth();
        provider.require_auth();
        validate_encrypted_ref(&encrypted_ref).map_err(|_| Error::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| Error::InvalidPolicyMetadata)?;

        match get_active_consent(&env, &patient, &provider) {
            None => return Err(Error::ConsentNotGranted),
            Some(s) if !s.can_write => return Err(Error::Unauthorized),
            _ => {}
        }

        let counter_key = DataKey::RecordCounter;
        let record_id: u64 = shared_contracts::safe_increment_persistent(&env, &counter_key);

        let timestamp = env.ledger().timestamp();

        let integrity_hash = compute_hash(
            &env,
            record_id,
            &patient,
            &provider,
            &encrypted_ref,
            record_category,
            &record_description,
            timestamp,
        );

        let record = MedicalRecord {
            record_id,
            patient,
            provider,
            encrypted_ref,
            record_category,
            record_description,
            timestamp,
            integrity_hash,
            policy,
            version: 1,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id), &record);
        index_record_by_category(&env, record_category, record_id);

        Ok(record_id)
    }

    /// Return every record ID filed under `category`, in creation order.
    ///
    /// Backed by `DataKey::CategoryIndex`, populated by `create_record` and
    /// `create_records_batch` -- a category-prefixed index rather than a
    /// linear scan of every record ever created.
    pub fn get_records_by_category(env: Env, category: RecordCategory) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::CategoryIndex(category))
            .unwrap_or(Vec::new(&env))
    }

    /// Create multiple records in one transaction.
    ///
    /// Only the provider needs to auth; per-patient write consent is checked
    /// individually for each `RecordInput`. Returns record IDs in input order.
    /// Enforces `MAX_BATCH_SIZE` (10) — returns `BatchTooLarge` if exceeded.
    pub fn create_records_batch(
        env: Env,
        provider: Address,
        records: Vec<RecordInput>,
    ) -> Result<Vec<u64>, Error> {
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        provider.require_auth();

        if records.len() > MAX_BATCH_SIZE {
            return Err(Error::BatchTooLarge);
        }

        let mut ids: Vec<u64> = Vec::new(&env);
        let timestamp = env.ledger().timestamp();

        for input in records.iter() {
            validate_nonzero_address(&input.patient).map_err(|_| Error::InvalidAddress)?;
            validate_encrypted_ref(&input.encrypted_ref)
                .map_err(|_| Error::InvalidEncryptedEnvelope)?;
            validate_policy_metadata(&input.policy).map_err(|_| Error::InvalidPolicyMetadata)?;

            match get_active_consent(&env, &input.patient, &provider) {
                None => return Err(Error::ConsentNotGranted),
                Some(s) if !s.can_write => return Err(Error::Unauthorized),
                _ => {}
            }

            let counter_key = DataKey::RecordCounter;
            let record_id: u64 = shared_contracts::safe_increment_persistent(&env, &counter_key);

            let integrity_hash = compute_hash(
                &env,
                record_id,
                &input.patient,
                &provider,
                &input.encrypted_ref,
                input.record_category,
                &input.record_description,
                timestamp,
            );

            let record = MedicalRecord {
                record_id,
                patient: input.patient.clone(),
                provider: provider.clone(),
                encrypted_ref: input.encrypted_ref.clone(),
                record_category: input.record_category,
                record_description: input.record_description.clone(),
                timestamp,
                integrity_hash,
                policy: input.policy.clone(),
                version: 1,
            };

            env.storage()
                .persistent()
                .set(&DataKey::Record(record_id), &record);
            index_record_by_category(&env, input.record_category, record_id);

            ids.push_back(record_id);
        }

        Ok(ids)
    }

    /// Retrieve a record. Caller must be the patient or have active read consent.
    pub fn get_record(env: Env, caller: Address, record_id: u64) -> Result<MedicalRecord, Error> {
        validate_nonzero_address(&caller).map_err(|_| Error::InvalidAddress)?;
        caller.require_auth();

        let record: MedicalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)?;

        if caller != record.patient {
            match get_active_consent(&env, &record.patient, &caller) {
                None => return Err(Error::Unauthorized),
                Some(s) if !s.can_read => return Err(Error::Unauthorized),
                _ => {}
            }
        }

        Ok(record)
    }

    /// Verify integrity. Caller must be the patient or have active read consent.
    pub fn verify_record_integrity(
        env: Env,
        caller: Address,
        record_id: u64,
        expected_hash: Bytes,
    ) -> Result<bool, Error> {
        validate_nonzero_address(&caller).map_err(|_| Error::InvalidAddress)?;
        caller.require_auth();

        let record: MedicalRecord =
            match env.storage().persistent().get(&DataKey::Record(record_id)) {
                Some(r) => r,
                None => return Ok(false),
            };

        if caller != record.patient {
            match get_active_consent(&env, &record.patient, &caller) {
                None => return Err(Error::Unauthorized),
                Some(s) if !s.can_read => return Err(Error::Unauthorized),
                _ => {}
            }
        }

        if expected_hash.len() != 32 {
            return Ok(false);
        }

        let recomputed = compute_hash(
            &env,
            record.record_id,
            &record.patient,
            &record.provider,
            &record.encrypted_ref,
            record.record_category,
            &record.record_description,
            record.timestamp,
        );

        let recomputed_bytes: Bytes = recomputed.into();
        Ok(recomputed_bytes == expected_hash)
    }

    /// Update an existing record, preserving the previous version in the amendment trail.
    ///
    /// - Saves the current record under `DataKey::RecordVersion(record_id, current_version)`.
    /// - Increments `version` by one and stores the updated record under `DataKey::Record(record_id)`.
    /// - Caller must be the patient or a consented provider.
    pub fn update_record(
        env: Env,
        caller: Address,
        record_id: u64,
        new_encrypted_ref: EncryptedEnvelopeRef,
        new_record_category: RecordCategory,
        new_record_description: Option<String>,
        new_policy: PolicyMetadata,
    ) -> Result<u32, Error> {
        validate_nonzero_address(&caller).map_err(|_| Error::InvalidAddress)?;
        validate_encrypted_ref(&new_encrypted_ref).map_err(|_| Error::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&new_policy).map_err(|_| Error::InvalidPolicyMetadata)?;
        caller.require_auth();

        let mut record: MedicalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)?;

        if caller != record.patient && !has_consent(&env, &record.patient, &caller) {
            return Err(Error::Unauthorized);
        }

        // Archive the current version before overwriting.
        let version_key = DataKey::RecordVersion(record_id, record.version);
        env.storage().persistent().set(&version_key, &record);

        let new_version = record.version + 1;
        let timestamp = env.ledger().timestamp();

        let new_integrity_hash = compute_hash(
            &env,
            record_id,
            &record.patient,
            &record.provider,
            &new_encrypted_ref,
            new_record_category,
            &new_record_description,
            timestamp,
        );

        // Note: this only adds the record to the new category's index; it
        // doesn't remove it from the old one (Soroban's Vec has no O(1)
        // remove-by-value), so a record that changes category will appear
        // under both until/unless that's addressed separately.
        if new_record_category != record.record_category {
            index_record_by_category(&env, new_record_category, record_id);
        }

        record.encrypted_ref = new_encrypted_ref;
        record.record_category = new_record_category;
        record.record_description = new_record_description;
        record.policy = new_policy;
        record.timestamp = timestamp;
        record.integrity_hash = new_integrity_hash;
        record.version = new_version;

        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id), &record);

        Ok(new_version)
    }

    /// Retrieve a specific historical version of a record.
    ///
    /// - Current version: read directly from `DataKey::Record(record_id)`.
    /// - Prior versions: read from `DataKey::RecordVersion(record_id, version)`.
    /// - Caller must be the patient or a consented provider.
    pub fn get_record_version(
        env: Env,
        caller: Address,
        record_id: u64,
        version: u32,
    ) -> Result<MedicalRecord, Error> {
        validate_nonzero_address(&caller).map_err(|_| Error::InvalidAddress)?;
        caller.require_auth();

        let current: MedicalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)?;

        if caller != current.patient && !has_consent(&env, &current.patient, &caller) {
            return Err(Error::Unauthorized);
        }

        if version == current.version {
            return Ok(current);
        }

        env.storage()
            .persistent()
            .get(&DataKey::RecordVersion(record_id, version))
            .ok_or(Error::VersionNotFound)
    }

    /// Capture an incident for this contract, optionally linking it to a
    /// cross-contract correlation ID.  Returns the new incident ID.
    pub fn report_incident(
        env: Env,
        reporter: Address,
        error_code: u32,
        description: String,
        correlation_id: Option<BytesN<32>>,
    ) -> Result<u64, Error> {
        validate_nonzero_address(&reporter).map_err(|_| Error::InvalidAddress)?;
        reporter.require_auth();
        Ok(capture_incident(
            &env,
            IncidentSeverity::High,
            String::from_str(&env, "health-records"),
            error_code,
            description,
            reporter,
            correlation_id,
        ))
    }

    /// Return all incident IDs that share the given correlation ID.
    pub fn get_incidents_by_correlation_id(
        env: Env,
        correlation_id: BytesN<32>,
    ) -> Vec<u64> {
        shared_get_by_corr(&env, correlation_id)
    }
}

#[cfg(test)]
mod test;
#[cfg(test)]
mod cid_fuzz_tests;
