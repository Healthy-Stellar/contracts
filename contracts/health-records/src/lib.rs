#![no_std]

//! # Health Records Contract
//!
//! Central medical record repository with encrypted storage, consent-based access control,
//! versioning, and integrity verification for longitudinal patient health data.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Record creation (both single and batch) uses consent-based
//! write authorization. The provider authenticates the transaction (provider.require_auth()), and
//! active write consent previously granted by the patient is verified against contract storage.
//! The patient's live signature is not required at creation time, enabling asynchronous provider
//! workflows (such as lab results, imaging reports, and discharge summaries). Patient grants
//! scoped consent (read, write, share) per provider. Consent can expire or never expire (0 = no
//! expiry). Patient revokes provider consent atomically. Emergency access override with 1-hour TTL.
//! Batch record creation enforces per-patient consent validation.
//!
//! **Audit Controls:** Record creation events logged with patient, provider, category, and
//! timestamp. Consent grant/revoke events tracked. Record update events with version number.
//! Integrity verification events for tamper detection. Incident reporting with correlation IDs.
//! Deregistration events with cascade cleanup.
//!
//! **Data Retention Policy:** Records retained indefinitely with version history (prior versions
//! in RecordVersion storage). Record category index enables efficient queries. Consent scope
//! expiration enforced at check time. Deregistration removes all patient records and consent.
//! Policy metadata enforces per-record retention rules.
//!
//! **Encryption/Integrity:** EncryptedEnvelopeRef with content hashing validates encrypted data.
//! PolicyMetadata enforces encryption requirements per record type. SHA256 integrity hash computed
//! and verified on retrieval. Record versioning maintains amendment trail. Consent validation
//! prevents unauthorized access.

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

/// Maximum number of record IDs stored per CategoryIndex page.
pub const CATEGORY_PAGE_SIZE: u32 = 100;

#[contracttype]
pub enum DataKey {
    Record(u64),
    RecordCounter,
    Consent(Address, Address),          // (patient, provider) -> ConsentScope
    PatientProviders(Address),          // patient -> Vec<Address> of consented providers
    CategoryIndex(RecordCategory, u32), // (category, page) -> Vec<u64> of record ids
    CategoryCount(RecordCategory),      // category -> total record count
    RecordCategoryLoc(u64),             // record_id -> (RecordCategory, page)
    ProviderRegistry,
    RecordVersion(u64, u32),
    /// Per-caller nonce for replay attack protection: (caller) -> u64
    CallerNonce(Address),
    Admin,
    Analytics,
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
    ProviderNotRegistered = 8,
    VersionNotFound = 9,
    StaleNonce = 10,
}

#[soroban_sdk::contractclient(name = "ProviderRegistryClient")]
pub trait ProviderRegistryInterface {
    fn is_provider(env: Env, provider: Address) -> bool;
}

/// Append record ID to its paged CategoryIndex bucket.
fn index_record_by_category(env: &Env, category: RecordCategory, record_id: u64) {
    let count_key = DataKey::CategoryCount(category);
    let count: u64 = env.storage().persistent().get(&count_key).unwrap_or(0);
    let page = (count / (CATEGORY_PAGE_SIZE as u64)) as u32;
    let page_key = DataKey::CategoryIndex(category, page);
    let mut ids: Vec<u64> = env.storage().persistent().get(&page_key).unwrap_or(Vec::new(env));
    ids.push_back(record_id);
    env.storage().persistent().set(&page_key, &ids);
    env.storage().persistent().set(&count_key, &(count + 1));
    env.storage().persistent().set(&DataKey::RecordCategoryLoc(record_id), &(category, page));
}

fn remove_record_from_category_index(env: &Env, category: RecordCategory, record_id: u64) {
    let loc_key = DataKey::RecordCategoryLoc(record_id);
    let loc_opt: Option<(RecordCategory, u32)> = env.storage().persistent().get(&loc_key);
    if let Some((stored_cat, page)) = loc_opt {
        if stored_cat == category {
            let page_key = DataKey::CategoryIndex(category, page);
            if let Some(ids) = env.storage().persistent().get::<DataKey, Vec<u64>>(&page_key) {
                let mut filtered = Vec::new(env);
                for id in ids.iter() {
                    if id != record_id {
                        filtered.push_back(id);
                    }
                }
                if filtered.is_empty() {
                    env.storage().persistent().remove(&page_key);
                } else {
                    env.storage().persistent().set(&page_key, &filtered);
                }
            }
            env.storage().persistent().remove(&loc_key);
        }
    }
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
    /// Bind the contract to an admin and the provider registry address.
    ///
    /// This runs atomically as part of contract creation and cannot be called
    /// again, so an attacker cannot front-run deployment and point
    /// `provider_registry` at a malicious contract.
    pub fn __constructor(env: Env, admin: Address, provider_registry: Address) {
        validate_nonzero_address(&admin).expect("admin address must not be zero");
        validate_nonzero_address(&provider_registry)
            .expect("provider registry address must not be zero");
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::ProviderRegistry, &provider_registry);
    }

    /// Patient grants a provider scoped consent to act on their records.
    ///
    /// `scope.expires_at == 0` means the consent never expires.
    pub fn grant_consent(
        env: Env,
        patient: Address,
        provider: Address,
        scope: ConsentScope,
        patient_nonce: u64,
    ) -> Result<(), Error> {
        validate_nonzero_address(&patient).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        patient.require_auth();
        Self::verify_and_increment_nonce(&env, &patient, patient_nonce)?;
        env.storage().persistent().set(
            &DataKey::Consent(patient.clone(), provider.clone()),
            &scope,
        );

        // #627: keep PatientProviders in sync so deregister_patient's consent
        // cleanup loop actually has providers to iterate over. Dedup on
        // repeat grants to the same provider.
        let idx_key = DataKey::PatientProviders(patient.clone());
        let mut providers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));
        if !providers.iter().any(|p| p == provider) {
            providers.push_back(provider);
            env.storage().persistent().set(&idx_key, &providers);
        }

        Ok(())
    }

    /// Patient revokes all consent for a provider.
    pub fn revoke_consent(env: Env, patient: Address, provider: Address, patient_nonce: u64) -> Result<(), Error> {
        validate_nonzero_address(&patient).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        patient.require_auth();
        Self::verify_and_increment_nonce(&env, &patient, patient_nonce)?;
        env.storage()
            .persistent()
            .remove(&DataKey::Consent(patient.clone(), provider.clone()));

        // #627: prune the PatientProviders index entry so it doesn't keep
        // accumulating providers the patient no longer has any consent
        // record for.
        let idx_key = DataKey::PatientProviders(patient.clone());
        let providers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));
        if providers.iter().any(|p| p == provider) {
            let mut remaining: Vec<Address> = Vec::new(&env);
            for p in providers.iter() {
                if p != provider {
                    remaining.push_back(p);
                }
            }
            env.storage().persistent().set(&idx_key, &remaining);
        }

        Ok(())
    }

    /// Remove all health-records state for a deregistered patient.
    ///
    /// Revokes consent for every provider that was ever granted access.
    /// Only callable by the patient themselves (they must auth before
    /// deregistering from patient-registry).
    pub fn deregister_patient(env: Env, patient: Address, patient_nonce: u64) -> Result<(), Error> {
        patient.require_auth();
        Self::verify_and_increment_nonce(&env, &patient, patient_nonce)?;
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
        Ok(())
    }

    /// Create a record. Uses consent-based write authorization matching create_records_batch:
    /// only the provider needs to auth, and active write consent granted by the patient is verified.
    pub fn create_record(
        env: Env,
        patient: Address,
        provider: Address,
        encrypted_ref: EncryptedEnvelopeRef,
        record_category: RecordCategory,
        record_description: Option<String>,
        policy: PolicyMetadata,
        provider_nonce: u64,
    ) -> Result<u64, Error> {
        validate_nonzero_address(&patient).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        provider.require_auth();
        Self::verify_and_increment_nonce(&env, &provider, provider_nonce)?;
        validate_encrypted_ref(&encrypted_ref).map_err(|_| Error::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| Error::InvalidPolicyMetadata)?;

        let registry_addr: Address = env.storage().instance().get(&DataKey::ProviderRegistry).unwrap();
        let provider_client = ProviderRegistryClient::new(&env, &registry_addr);
        if !provider_client.is_provider(&provider) {
            return Err(Error::ProviderNotRegistered);
        }

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

    /// Return record IDs filed under `category` for a given 0-indexed page (capped at CATEGORY_PAGE_SIZE).
    /// Access is restricted to the contract admin or configured analytics address.
    pub fn get_records_by_category(
        env: Env,
        requester: Address,
        category: RecordCategory,
        page: u32,
    ) -> Result<Vec<u64>, Error> {
        validate_nonzero_address(&requester).map_err(|_| Error::InvalidAddress)?;
        requester.require_auth();

        let is_admin = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
            .map(|a| a == requester)
            .unwrap_or(false);
        let is_analytics = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Analytics)
            .map(|a| a == requester)
            .unwrap_or(false);

        if !is_admin && !is_analytics {
            return Err(Error::Unauthorized);
        }

        let key = DataKey::CategoryIndex(category, page);
        Ok(env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env)))
    }

    /// Return total number of records indexed under a category.
    pub fn get_category_count(env: Env, category: RecordCategory) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::CategoryCount(category))
            .unwrap_or(0)
    }

    /// Set an authorized analytics address that can query category indexes. Admin-only.
    pub fn set_analytics_address(env: Env, admin: Address, analytics: Address) -> Result<(), Error> {
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&analytics).map_err(|_| Error::InvalidAddress)?;
        admin.require_auth();

        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if admin != current_admin {
            return Err(Error::Unauthorized);
        }

        env.storage().instance().set(&DataKey::Analytics, &analytics);
        Ok(())
    }

    /// Return the contract admin address.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)
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
        provider_nonce: u64,
    ) -> Result<Vec<u64>, Error> {
        validate_nonzero_address(&provider).map_err(|_| Error::InvalidAddress)?;
        provider.require_auth();
        Self::verify_and_increment_nonce(&env, &provider, provider_nonce)?;

        let registry_addr: Address = env.storage().instance().get(&DataKey::ProviderRegistry).unwrap();
        let provider_client = ProviderRegistryClient::new(&env, &registry_addr);
        if !provider_client.is_provider(&provider) {
            return Err(Error::ProviderNotRegistered);
        }

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
        caller_nonce: u64,
    ) -> Result<u32, Error> {
        validate_nonzero_address(&caller).map_err(|_| Error::InvalidAddress)?;
        validate_encrypted_ref(&new_encrypted_ref).map_err(|_| Error::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&new_policy).map_err(|_| Error::InvalidPolicyMetadata)?;
        caller.require_auth();
        Self::verify_and_increment_nonce(&env, &caller, caller_nonce)?;

        let mut record: MedicalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)?;

        if caller != record.patient {
            match get_active_consent(&env, &record.patient, &caller) {
                None => return Err(Error::Unauthorized),
                Some(s) if !s.can_write => return Err(Error::Unauthorized),
                _ => {}
            }
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

        // When category changes, remove from old index and add to new index.
        if new_record_category != record.record_category {
            remove_record_from_category_index(&env, record.record_category, record_id);
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

        if caller != current.patient {
            match get_active_consent(&env, &current.patient, &caller) {
                None => return Err(Error::Unauthorized),
                Some(s) if !s.can_read => return Err(Error::Unauthorized),
                _ => {}
            }
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
        reporter_nonce: u64,
    ) -> Result<u64, Error> {
        validate_nonzero_address(&reporter).map_err(|_| Error::InvalidAddress)?;
        reporter.require_auth();
        Self::verify_and_increment_nonce(&env, &reporter, reporter_nonce)?;
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

    /// Verify and increment caller's nonce for cross-contract call protection.
    /// Returns an error if the provided nonce is <= the last successful nonce.
    fn verify_and_increment_nonce(
        env: &Env,
        caller: &Address,
        provided_nonce: u64,
    ) -> Result<(), Error> {
        let nonce_key = DataKey::CallerNonce(caller.clone());
        let last_nonce: u64 = env
            .storage()
            .persistent()
            .get(&nonce_key)
            .unwrap_or(0);

        // Reject if provided nonce is not strictly greater than last successful nonce
        if provided_nonce <= last_nonce {
            return Err(Error::StaleNonce);
        }

        // Update nonce to prevent replay
        env.storage().persistent().set(&nonce_key, &provided_nonce);
        Ok(())
    }

    /// Get the current nonce for a caller.
    pub fn get_caller_nonce(env: Env, caller: Address) -> u64 {
        let nonce_key = DataKey::CallerNonce(caller);
        env.storage().persistent().get(&nonce_key).unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
#[cfg(test)]
mod cid_fuzz_tests;
