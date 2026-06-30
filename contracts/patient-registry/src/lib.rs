#![no_std]

//! # Patient Registry Contract
//!
//! Maintains central patient registry with demographic data, encrypted contact information,
//! incident tracking, and pagination support for patient directory management.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Patient authorization for registration. Provider access to
//! patient directory restricted to network participants. Patient consent for provider queries.
//! Encrypted contact information prevents unauthorized disclosure. Registry lookup authorization
//! enforced per query.
//!
//! **Audit Controls:** Patient registration events logged with identity. Patient profile update
//! events tracked. Deregistration events recorded. Incident reports linked to patient records.
//! Correlation IDs enable multi-contract incident tracking. Access audit trail via pagination
//! query logs.
//!
//! **Data Retention Policy:** Patient demographic data retained indefinitely. Contact information
//! encrypted and persisted. Deregistration marks patient inactive without full deletion. Incident
//! records retained for audit trail. Historical data preserved for continuity of care.
//!
//! **Encryption/Integrity:** EncryptedEnvelopeRef for contact information with content hashing.
//! PolicyMetadata enforces encryption requirements. Nonzero address validation. Hash validation
//! ensures contact data integrity. Pagination queries validated.

use shared::pagination::PageResult;

use shared::incident_tracking::{
    capture_incident, get_incidents_by_correlation_id as shared_get_by_corr, IncidentSeverity,
};
use shared::privacy::{
    validate_encrypted_ref, validate_nonzero_address, validate_nonzero_hash, validate_policy_metadata,
    EncryptedEnvelopeRef, PolicyMetadata,
};
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error,
    symbol_short, token, vec, xdr::ToXdr, Address, Bytes, BytesN, Env, IntoVal, Map, String,
    Symbol, Vec,
};
use ttl_config::critical::{LEDGER_BUMP_AMOUNT, LEDGER_THRESHOLD};
use ttl_config::{extend_critical_ttl_if_exists, extend_operational_ttl_if_exists};

pub mod merkle;
pub mod validation;
pub const NEW_RECORD_TOPIC: &str = "new_record";
pub const ARCHIVE_LEDGER_THRESHOLD: u32 = 100_000;
pub const ARCHIVE_LEDGER_BUMP_AMOUNT: u32 = 518_400;

/// --------------------
/// Patient Status
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatientStatus {
    Active,
    Deregistered,
}

/// Retention classification for patient data, aligned with HIPAA minimum standards.
///
/// - `Clinical`        → critical TTL (~31 days) for care-critical records
/// - `Administrative`  → operational TTL (~7 days) for admin/operational records
/// - `Financial`       → critical TTL (~31 days) for financial/billing records
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetentionClass {
    Clinical,
    Administrative,
    Financial,
}

/// Emitted on every patient status transition (Active ↔ Deregistered).
#[contractevent]
pub struct PatientStatusChanged {
    pub patient: Address,
    pub old_status: PatientStatus,
    pub new_status: PatientStatus,
    pub timestamp: u64,
}

/// --------------------
/// Retention Class
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetentionClass {
    Clinical,
    Financial,
    Administrative,
}

/// --------------------
/// Patient Structures
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatientData {
    pub name: String,
    pub dob: u64,
    pub encrypted_metadata_ref: EncryptedEnvelopeRef,
    pub status: PatientStatus,
    pub policy: PolicyMetadata,
    pub retention_class: RetentionClass,
    pub guardian: Option<Address>,
}

/// --------------------
/// Doctor Structures
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorData {
    pub name: String,
    pub specialization: String,
    pub certificate_hash: Bytes,
    pub verified: bool,
}

/// --------------------
/// Consent Types
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsentStatus {
    NeverSigned,
    Pending,
    Acknowledged,
}

/// --------------------
/// Storage Keys
/// --------------------
#[contracttype]
pub enum DataKey {
    Admin,
    Patient(Address),
    Doctor(Address),
    Institution(Address),
    MedicalRecords(Address),
    AuthorizedDoctors(Address),
    RegulatoryHold(Address),
    ConsentVersion,
    ConsentAck(Address),
    Guardian(Address),
    PatientList,
    DoctorList,
    LastSnapshotLedger,
    RecordFee,
    Treasury,
    FeeToken,
    TotalPatients,
    TotalRecordsCreated,
    TotalProviders,
    TotalAccessGrants,
    /// Nonce counter per patient for share-link token generation.
    ShareNonce(Address),
    /// Nonce counter per patient for data export ticket generation.
    ExportNonce(Address),
    /// Share link data keyed by token hash.
    ShareLink(BytesN<32>),
    /// Marks a patient as deregistered (value: timestamp of deregistration).
    Deregistered(Address),
    /// Contract-frozen flag (bool).
    Frozen,
    /// Global monotonic record counter (u64, instance storage).
    RecordCounter,
    /// Per-patient ordered list of record IDs (Vec<u64>).
    PatientRecordIds(Address),
    /// Individual record data keyed by global record ID.
    MedicalRecord(u64),
    /// Field-level access mask keyed by (patient, grantee, record_id).
    FieldAccess(Address, Address, u64),
    /// Platform-wide secondary index: record_type → Vec<TypeIndexEntry>.
    GlobalTypeIndex(Symbol),
    /// Soft-delete tombstone for a record (value: timestamp of deletion).
    DeletedRecord(u64),
    /// Archived record lookup keyed by global record ID.
    ArchivedRecord(u64),
    /// Merkle root for a patient's records.
    /// Merkle root over the patient's ordered record IDs (see `merkle` module).
    MerkleRoot(Address),
    /// Optional provider-registry contract address for cross-contract verification.
    ProviderRegistry,
    /// Per-caller nonce for replay attack protection: (caller) -> u64
    CallerNonce(Address),
}

/// --------------------
/// Share Link
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShareLinkData {
    pub patient: Address,
    pub record_id: u64,
    pub uses_remaining: u32,
    pub expires_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportTicket {
    pub patient: Address,
    pub issued_at: u64,
    pub expires_at: u64,
    pub nonce: BytesN<32>,
    pub signature: BytesN<32>,
}

/// One entry in the platform-wide secondary index.
/// Maps a `record_type` to the patient who owns it and the global record ID.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeIndexEntry {
    pub patient: Address,
    pub record_id: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedicalRecord {
    pub record_id: u64,
    pub doctor: Address,
    pub encrypted_ref: EncryptedEnvelopeRef,
    pub timestamp: u64,
    pub record_type: Symbol,
    pub policy: PolicyMetadata,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldPermission {
    RecordType,
    EncryptedRef,
    CreatedAt,
    CreatedBy,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartialRecord {
    pub record_type: Option<Symbol>,
    pub encrypted_ref_hash: Option<BytesN<32>>,
    pub created_at: Option<u64>,
    pub created_by: Option<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordVersion {
    pub encrypted_ref: EncryptedEnvelopeRef,
    pub updated_by: Address,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordData {
    pub patient: Address,
    pub record_type: Symbol,
    pub current_ref: EncryptedEnvelopeRef,
    pub history: Vec<RecordVersion>,
    pub latest_version: u64,
    pub policy: PolicyMetadata,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchivedRecordRef {
    pub patient: Address,
    pub record_id: u64,
    pub record_type: Symbol,
    pub cid_hash: BytesN<32>,
    pub archived_at_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegulatoryHold {
    pub reason_hash: BytesN<32>,
    pub expires_at: u64,
    pub placed_at: u64,
}

#[allow(clippy::upper_case_acronyms)]
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    InvalidCID = 1,
    InvalidToken = 2,
    NotAuthorized = 3,
    InvalidDID = 4,
    InvalidScore = 5,
    ContractFrozen = 6,
    NoRecordsFound = 7,
    NotFound = 8,
    AlreadyInitialized = 9,
    AlreadyExists = 10,
    AlreadyDeregistered = 11,
    AlreadyDeleted = 12,
    InvalidFee = 13,
    ConsentVersionMismatch = 14,
    ConsentNotAcknowledged = 15,
    HoldAlreadyActive = 16,
    HoldExpired = 17,
    TooManyIds = 18,
    SnapshotRateLimit = 19,
    UnauthorizedInstitution = 20,
    InvalidEncryptedEnvelope = 21,
    InvalidPolicyMetadata = 22,
    InvalidPagination = 23,
    BatchTooLarge = 24,
    RecordNotFound = 25,
    NoHistoryFound = 26,
    InvalidAddress = 27,
    ProviderNotRegistered = 28,
    /// A Vec parameter or accumulator exceeds its maximum allowed length.
    InputTooLarge = 29,
    StaleNonce = 30,
}

pub fn validate_cid(cid: &Bytes) -> Result<(), ContractError> {
    let len = cid.len() as usize;
    if len == 0 || len > 512 {
        return Err(ContractError::InvalidCID);
    }
    let mut buf = [0u8; 512];
    for (i, slot) in buf[..len].iter_mut().enumerate() {
        *slot = cid.get(i as u32).ok_or(ContractError::InvalidCID)?;
    }
    validation::validate_cid_bytes(&buf[..len]).map_err(|_| ContractError::InvalidCID)
}

/// Validates a decentralized identifier string (`did:method:…`) for metadata or
/// cross-chain references. Fuzzed via `validation::validate_did_bytes`.
pub fn validate_did(did: &String) -> Result<(), ContractError> {
    let len = did.len() as usize;
    if len > 256 {
        return Err(ContractError::InvalidDID);
    }
    let mut buf = [0u8; 256];
    did.copy_into_slice(&mut buf[..len]);
    validation::validate_did_bytes(&buf[..len]).map_err(|_| ContractError::InvalidDID)
}

/// Validates a bounded numeric score (default 0–100). Fuzzed via
/// `validation::validate_score_i32`.
pub fn validate_score(score: i32) -> Result<(), ContractError> {
    validation::validate_score_i32(score).map_err(|_| ContractError::InvalidScore)
}

fn require_patient_or_guardian(
    env: &Env,
    patient: &Address,
    caller: &Address,
) -> Result<(), ContractError> {
    let guardian_key = DataKey::Guardian(patient.clone());
    let guardian_opt: Option<Address> = env.storage().persistent().get(&guardian_key);
    if caller == patient || guardian_opt.as_ref() == Some(caller) {
        caller.require_auth();
        Ok(())
    } else {
        Err(ContractError::NotAuthorized)
    }
}

fn next_export_nonce(env: &Env, patient: &Address, issued_at: u64) -> BytesN<32> {
    let nonce_key = DataKey::ExportNonce(patient.clone());
    let nonce_counter: u64 = env.storage().persistent().get(&nonce_key).unwrap_or(0u64) + 1;
    env.storage().persistent().set(&nonce_key, &nonce_counter);

    let mut preimage = Bytes::new(env);
    preimage.append(&patient.clone().to_xdr(env));
    preimage.extend_from_array(&issued_at.to_be_bytes());
    preimage.extend_from_array(&nonce_counter.to_be_bytes());

    env.crypto().sha256(&preimage).into()
}

fn sign_export_ticket(
    env: &Env,
    patient: &Address,
    issued_at: u64,
    expires_at: u64,
    nonce: &BytesN<32>,
) -> BytesN<32> {
    let mut payload = Bytes::new(env);
    payload.append(&patient.clone().to_xdr(env));
    payload.extend_from_array(&issued_at.to_be_bytes());
    payload.extend_from_array(&expires_at.to_be_bytes());
    payload.append(&Bytes::from(nonce.clone()));

    env.crypto().sha256(&payload).into()
}

/// Enforces that `caller` is the patient, their guardian, or an authorized doctor.
fn require_record_access(
    env: &Env,
    patient: &Address,
    caller: &Address,
) -> Result<(), ContractError> {
    if caller == patient {
        return Ok(());
    }
    let guardian_key = DataKey::Guardian(patient.clone());
    let guardian_opt: Option<Address> = env.storage().persistent().get(&guardian_key);
    if guardian_opt.as_ref() == Some(caller) {
        return Ok(());
    }
    let access_key = DataKey::AuthorizedDoctors(patient.clone());
    let access_map: Map<Address, bool> = env
        .storage()
        .persistent()
        .get(&access_key)
        .unwrap_or(Map::new(env));
    if access_map.contains_key(caller.clone()) {
        return Ok(());
    }
    Err(ContractError::NotAuthorized)
}

const FIELD_RECORD_TYPE: u32 = 1 << 0;
const FIELD_ENCRYPTED_REF: u32 = 1 << 1;
const FIELD_CREATED_AT: u32 = 1 << 2;
const FIELD_CREATED_BY: u32 = 1 << 3;
const FIELD_ALL: u32 =
    FIELD_RECORD_TYPE | FIELD_ENCRYPTED_REF | FIELD_CREATED_AT | FIELD_CREATED_BY;

fn field_permission_mask(fields: Vec<FieldPermission>) -> u32 {
    let mut mask = 0u32;
    for field in fields.iter() {
        mask |= match field {
            FieldPermission::RecordType => FIELD_RECORD_TYPE,
            FieldPermission::EncryptedRef => FIELD_ENCRYPTED_REF,
            FieldPermission::CreatedAt => FIELD_CREATED_AT,
            FieldPermission::CreatedBy => FIELD_CREATED_BY,
        };
    }
    mask
}

fn empty_partial_record() -> PartialRecord {
    PartialRecord {
        record_type: None,
        encrypted_ref_hash: None,
        created_at: None,
        created_by: None,
    }
}

fn build_partial_record(record_data: &RecordData, mask: u32) -> PartialRecord {
    let created = record_data.history.get(0);
    PartialRecord {
        record_type: if (mask & FIELD_RECORD_TYPE) != 0 {
            Some(record_data.record_type.clone())
        } else {
            None
        },
        encrypted_ref_hash: if (mask & FIELD_ENCRYPTED_REF) != 0 {
            Some(record_data.current_ref.content_hash.clone())
        } else {
            None
        },
        created_at: if (mask & FIELD_CREATED_AT) != 0 {
            created.as_ref().map(|version| version.updated_at)
        } else {
            None
        },
        created_by: if (mask & FIELD_CREATED_BY) != 0 {
            created.map(|version| version.updated_by)
        } else {
            None
        },
    }
}

/// Delegate TTL extension to the appropriate `ttl-config` helper based on the
/// patient's `RetentionClass`, calling `extend_*_ttl_if_exists` so that missing
/// keys are silently skipped.
fn extend_for_retention_class<K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &K,
    class: &RetentionClass,
) {
    match class {
        RetentionClass::Clinical | RetentionClass::Financial => {
            extend_critical_ttl_if_exists(env, key);
        }
        RetentionClass::Administrative => {
            extend_operational_ttl_if_exists(env, key);
        }
    }
}

/// Maximum number of records that may be returned in a single paginated call.
pub const MAX_PAGE_SIZE: u32 = 50;

/// Maximum number of entries allowed in a single batch registration call.
pub const MAX_BATCH_SIZE: u32 = 50;

/// Maximum number of authorized doctors a single patient may have.
pub const MAX_AUTHORIZED_DOCTORS: u32 = 50;

/// Input entry for `batch_register_patients`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchPatientEntry {
    pub wallet: Address,
    pub name: String,
    pub dob: u64,
    pub encrypted_metadata_ref: EncryptedEnvelopeRef,
    pub policy: PolicyMetadata,
}

/// Per-entry outcome returned by batch registration functions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BatchEntryStatus {
    /// Entry registered successfully.
    Success,
    /// Entry skipped because the address is already registered.
    AlreadyExists,
    /// Entry failed validation (error code carried along for observability).
    Failed(u32),
}

/// Return type for `get_medical_records_paged`.
///
/// `total` is the total number of records stored for the patient (excluding
/// nothing — callers use it to determine whether more pages exist).
/// `records` contains at most `limit` entries starting at `offset`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagedRecords {
    pub records: Vec<MedicalRecord>,
    pub total: u32,
}

#[contract]
pub struct MedicalRegistry;

#[contractimpl]
impl MedicalRegistry {
    // =====================================================
    //                    ADMIN / CONSENT
    // =====================================================

    pub fn initialize(
        env: Env,
        admin: Address,
        treasury: Address,
        fee_token: Address,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        validate_nonzero_address(&admin).map_err(|_| ContractError::InvalidAddress)?;
        validate_nonzero_address(&treasury).map_err(|_| ContractError::InvalidAddress)?;
        validate_nonzero_address(&fee_token).map_err(|_| ContractError::InvalidAddress)?;
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Treasury, &treasury);
        env.storage().instance().set(&DataKey::FeeToken, &fee_token);
        env.storage().instance().set(&DataKey::RecordFee, &0i128);
        env.storage().instance().set(&DataKey::TotalPatients, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalRecordsCreated, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalProviders, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalAccessGrants, &0u64);
        env.storage().instance().set(&DataKey::RecordCounter, &0u64);
        Ok(())
    }

    // =====================================================
    //                  CONTRACT FREEZE
    // =====================================================

    pub fn freeze_contract(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Frozen, &true);
        env.events()
            .publish((symbol_short!("freeze"),), symbol_short!("frozen"));
    }

    pub fn unfreeze_contract(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Frozen, &false);
        env.events()
            .publish((symbol_short!("unfreeze"),), symbol_short!("active"));
    }

    pub fn is_frozen(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Frozen)
            .unwrap_or(false)
    }

    // =====================================================
    //                    ADMIN / CONSENT
    // =====================================================

    pub fn set_record_fee(env: Env, amount: i128) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotFound)?;
        admin.require_auth();
        if amount < 0 {
            return Err(ContractError::InvalidFee);
        }
        env.storage().instance().set(&DataKey::RecordFee, &amount);
        Ok(())
    }

    pub fn get_record_fee(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::RecordFee)
            .unwrap_or(0)
    }

    pub fn publish_consent_version(env: Env, version_hash: BytesN<32>) {
        Self::require_not_frozen(&env);
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotFound));
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::ConsentVersion, &version_hash);
        env.events()
            .publish((symbol_short!("consent_v"), admin), version_hash);
    }

    pub fn assign_guardian(env: Env, patient: Address, guardian: Address) {
        Self::require_not_frozen(&env);
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotFound));
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Guardian(patient.clone()), &guardian);
        env.events()
            .publish((symbol_short!("grd_asgn"), patient), guardian);
    }

    pub fn revoke_guardian(env: Env, patient: Address) {
        Self::require_not_frozen(&env);
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotFound));
        admin.require_auth();
        env.storage()
            .persistent()
            .remove(&DataKey::Guardian(patient.clone()));
        env.events().publish(
            (symbol_short!("grd_rev"), patient),
            symbol_short!("revoked"),
        );
    }

    pub fn get_guardian(env: Env, patient: Address) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Guardian(patient))
    }

    pub fn acknowledge_consent(
        env: Env,
        patient: Address,
        caller: Address,
        version_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        require_patient_or_guardian(&env, &patient, &caller)?;
        let current: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::ConsentVersion)
            .ok_or(ContractError::NotFound)?;
        if current != version_hash {
            return Err(ContractError::ConsentVersionMismatch);
        }
        env.storage()
            .persistent()
            .set(&DataKey::ConsentAck(patient.clone()), &version_hash);
        env.events()
            .publish((symbol_short!("consent_a"), patient), version_hash);
        Ok(())
    }

    pub fn get_consent_status(env: Env, patient: Address) -> ConsentStatus {
        let current_opt: Option<BytesN<32>> =
            env.storage().persistent().get(&DataKey::ConsentVersion);
        let ack_opt: Option<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&DataKey::ConsentAck(patient));
        match (current_opt, ack_opt) {
            (None, _) => ConsentStatus::NeverSigned,
            (Some(_), None) => ConsentStatus::NeverSigned,
            (Some(current), Some(ack)) => {
                if ack == current {
                    ConsentStatus::Acknowledged
                } else {
                    ConsentStatus::Pending
                }
            }
        }
    }

    pub fn get_total_records_created(env: Env) -> u64 {
        Self::require_admin(&env);
        env.storage()
            .instance()
            .get(&DataKey::TotalRecordsCreated)
            .unwrap_or(0u64)
    }

    pub fn get_total_providers(env: Env) -> u64 {
        Self::require_admin(&env);
        env.storage()
            .instance()
            .get(&DataKey::TotalProviders)
            .unwrap_or(0u64)
    }

    pub fn get_total_access_grants(env: Env) -> u64 {
        Self::require_admin(&env);
        env.storage()
            .instance()
            .get(&DataKey::TotalAccessGrants)
            .unwrap_or(0u64)
    }

    // =====================================================
    //                    PATIENT LOGIC
    // =====================================================

    pub fn register_patient(
        env: Env,
        wallet: Address,
        name: String,
        dob: u64,
        encrypted_metadata_ref: EncryptedEnvelopeRef,
        policy: PolicyMetadata,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        wallet.require_auth();
        validate_encrypted_ref(&encrypted_metadata_ref)
            .map_err(|_| ContractError::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| ContractError::InvalidPolicyMetadata)?;

        let key = DataKey::Patient(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyExists);
        }

        let patient = PatientData {
            name,
            dob,
            encrypted_metadata_ref,
            status: PatientStatus::Active,
            policy,
            retention_class: RetentionClass::Clinical,
            guardian: None,
        };
        env.storage().persistent().set(&key, &patient);
        let total_patients: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalPatients)
            .unwrap_or(0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalPatients, &(total_patients + 1));

        let mut pat_list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientList)
            .unwrap_or(Vec::new(&env));
        pat_list.push_back(wallet.clone());
        env.storage()
            .persistent()
            .set(&DataKey::PatientList, &pat_list);

        env.events()
            .publish((symbol_short!("reg_pat"), wallet), symbol_short!("success"));
        Ok(())
    }

    /// Register a patient on behalf of a minor or an adult lacking capacity.
    ///
    /// Both `wallet` (the patient) and `guardian` must authorise the call.
    /// The guardian is stored both inside `PatientData` and under the
    /// `DataKey::Guardian` key so that all existing guardian-aware helpers
    /// (`require_patient_or_guardian`, `acknowledge_consent`, etc.) keep
    /// working without modification.
    pub fn register_patient_with_guardian(
        env: Env,
        wallet: Address,
        guardian: Address,
        name: String,
        dob: u64,
        encrypted_metadata_ref: EncryptedEnvelopeRef,
        policy: PolicyMetadata,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        wallet.require_auth();
        guardian.require_auth();

        validate_encrypted_ref(&encrypted_metadata_ref)
            .map_err(|_| ContractError::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| ContractError::InvalidPolicyMetadata)?;

        let key = DataKey::Patient(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyExists);
        }

        let patient = PatientData {
            name,
            dob,
            encrypted_metadata_ref,
            status: PatientStatus::Active,
            policy,
            retention_class: RetentionClass::Clinical,
            guardian: Some(guardian.clone()),
        };
        env.storage().persistent().set(&key, &patient);

        env.storage()
            .persistent()
            .set(&DataKey::Guardian(wallet.clone()), &guardian);

        let total_patients: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalPatients)
            .unwrap_or(0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalPatients, &(total_patients + 1));

        let mut pat_list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientList)
            .unwrap_or(Vec::new(&env));
        pat_list.push_back(wallet.clone());
        env.storage()
            .persistent()
            .set(&DataKey::PatientList, &pat_list);

        env.events().publish(
            (symbol_short!("reg_grd"), wallet),
            guardian,
        );
        Ok(())
    }

    pub fn update_patient(
        env: Env,
        wallet: Address,
        caller: Address,
        encrypted_metadata_ref: EncryptedEnvelopeRef,
        policy: PolicyMetadata,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        require_patient_or_guardian(&env, &wallet, &caller)?;
        Self::require_not_on_hold(&env, &wallet)?;
        validate_encrypted_ref(&encrypted_metadata_ref)
            .map_err(|_| ContractError::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| ContractError::InvalidPolicyMetadata)?;

        let key = DataKey::Patient(wallet.clone());
        let mut patient: PatientData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::NotFound)?;

        patient.encrypted_metadata_ref = encrypted_metadata_ref;
        patient.policy = policy;
        env.storage().persistent().set(&key, &patient);

        env.events()
            .publish((symbol_short!("upd_pat"), wallet), symbol_short!("success"));
        Ok(())
    }

    pub fn get_patient(env: Env, wallet: Address) -> Result<PatientData, ContractError> {
        let key = DataKey::Patient(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::NotFound)
    }

    pub fn is_patient_registered(env: Env, wallet: Address) -> bool {
        let key = DataKey::Patient(wallet);
        env.storage().persistent().has(&key)
    }

    /// Deregister the calling patient.
    ///
    /// - Sets `PatientData.status` to `Deregistered`.
    /// - Clears all access grants so former grantees can no longer read records.
    /// - Records are retained (not deleted) and remain readable by the admin.
    /// - Emits a `pat_dreg` audit event.
    pub fn deregister_patient(env: Env, patient: Address) -> Result<(), ContractError> {
        patient.require_auth();

        let key = DataKey::Patient(patient.clone());
        let mut data: PatientData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::NotFound)?;

        if data.status == PatientStatus::Deregistered {
            return Err(ContractError::AlreadyDeregistered);
        }

        let old_status = data.status.clone();
        data.status = PatientStatus::Deregistered;
        env.storage().persistent().set(&key, &data);

        env.storage().persistent().set(
            &DataKey::Deregistered(patient.clone()),
            &env.ledger().timestamp(),
        );

        env.storage()
            .persistent()
            .remove(&DataKey::AuthorizedDoctors(patient.clone()));

        let ts = env.ledger().timestamp();
        env.events().publish(
            (symbol_short!("pat_dreg"), patient.clone()),
            ts,
        );
        PatientStatusChanged {
            patient: patient.clone(),
            old_status,
            new_status: PatientStatus::Deregistered,
            timestamp: ts,
        }
        .publish(&env);
        Ok(())
    }

    /// Reactivate a previously deregistered patient (admin-only).
    pub fn reactivate_patient(env: Env, patient: Address) -> Result<(), ContractError> {
        Self::require_admin(&env);

        let key = DataKey::Patient(patient.clone());
        let mut data: PatientData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::NotFound)?;

        if data.status != PatientStatus::Deregistered {
            return Err(ContractError::NotFound);
        }

        let old_status = data.status.clone();
        data.status = PatientStatus::Active;
        env.storage().persistent().set(&key, &data);

        env.storage()
            .persistent()
            .remove(&DataKey::Deregistered(patient.clone()));

        let ts = env.ledger().timestamp();
        PatientStatusChanged {
            patient: patient.clone(),
            old_status,
            new_status: PatientStatus::Active,
            timestamp: ts,
        }
        .publish(&env);
        Ok(())
    }

    pub fn get_total_patients(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::TotalPatients)
            .unwrap_or(0)
    }

    /// Register multiple patients in one transaction.
    ///
    /// Returns a `Vec<BatchEntryStatus>` aligned 1-to-1 with `entries`.
    /// Already-registered wallets produce `AlreadyExists` (no error, no overwrite).
    /// Validation failures produce `Failed(error_code)`.
    /// Enforces `MAX_BATCH_SIZE`; returns `BatchTooLarge` if exceeded.
    pub fn batch_register_patients(
        env: Env,
        entries: Vec<BatchPatientEntry>,
    ) -> Result<Vec<BatchEntryStatus>, ContractError> {
        Self::require_not_frozen(&env);
        if entries.len() > MAX_BATCH_SIZE {
            return Err(ContractError::BatchTooLarge);
        }

        let mut results: Vec<BatchEntryStatus> = Vec::new(&env);

        for entry in entries.iter() {
            // Validate encrypted ref and policy.
            if validate_encrypted_ref(&entry.encrypted_metadata_ref).is_err() {
                results.push_back(BatchEntryStatus::Failed(
                    ContractError::InvalidEncryptedEnvelope as u32,
                ));
                continue;
            }
            if validate_policy_metadata(&entry.policy).is_err() {
                results.push_back(BatchEntryStatus::Failed(
                    ContractError::InvalidPolicyMetadata as u32,
                ));
                continue;
            }

            let key = DataKey::Patient(entry.wallet.clone());
            if env.storage().persistent().has(&key) {
                results.push_back(BatchEntryStatus::AlreadyExists);
                continue;
            }

            let patient = PatientData {
                name: entry.name.clone(),
                dob: entry.dob,
                encrypted_metadata_ref: entry.encrypted_metadata_ref.clone(),
                status: PatientStatus::Active,
                policy: entry.policy.clone(),
                retention_class: RetentionClass::Clinical,
                guardian: None,
            };
            env.storage().persistent().set(&key, &patient);

            let total_patients: u64 = env
                .storage()
                .instance()
                .get(&DataKey::TotalPatients)
                .unwrap_or(0u64);
            env.storage()
                .instance()
                .set(&DataKey::TotalPatients, &(total_patients + 1));

            let mut pat_list: Vec<Address> = env
                .storage()
                .persistent()
                .get(&DataKey::PatientList)
                .unwrap_or(Vec::new(&env));
            pat_list.push_back(entry.wallet.clone());
            env.storage()
                .persistent()
                .set(&DataKey::PatientList, &pat_list);

            env.events().publish(
                (symbol_short!("reg_pat"), entry.wallet.clone()),
                symbol_short!("success"),
            );

            results.push_back(BatchEntryStatus::Success);
        }

        Ok(results)
    }

    /// Extend the TTL of all persistent storage entries for a patient.
    /// Callable by the patient themselves or the contract admin.
    pub fn extend_patient_ttl(env: Env, patient: Address) {
        Self::require_not_frozen(&env);
        // Authorize: patient or admin
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::NotFound));

        let is_admin = admin == patient;
        if is_admin {
            patient.require_auth();
        } else {
            // Check if caller is the patient itself or a guardian
            let guardian_key = DataKey::Guardian(patient.clone());
            let guardian_opt: Option<Address> = env.storage().persistent().get(&guardian_key);
            // We allow the patient or the admin — require patient auth here
            // (admin path handled above, so this must be the patient)
            let _ = guardian_opt; // not used here; only patient or admin may call
            patient.require_auth();
        }

        // Read patient's retention class to select the appropriate TTL helper.
        let patient_key = DataKey::Patient(patient.clone());
        let class = env
            .storage()
            .persistent()
            .get::<DataKey, PatientData>(&patient_key)
            .map(|d| d.retention_class)
            .unwrap_or(RetentionClass::Clinical);

        // Extend Patient record TTL
        extend_for_retention_class(&env, &patient_key, &class);

        // Extend MedicalRecords TTL
        let records_key = DataKey::MedicalRecords(patient.clone());
        extend_for_retention_class(&env, &records_key, &class);

        // Extend AuthorizedDoctors TTL
        let access_key = DataKey::AuthorizedDoctors(patient.clone());
        extend_for_retention_class(&env, &access_key, &class);

        // Extend ConsentAck TTL
        let consent_key = DataKey::ConsentAck(patient.clone());
        extend_for_retention_class(&env, &consent_key, &class);
    }

    pub fn place_hold(
        env: Env,
        patient: Address,
        reason_hash: BytesN<32>,
        expires_at: u64,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        Self::require_admin(&env);
        Self::require_patient_exists(&env, &patient)?;

        let now = env.ledger().timestamp();
        if expires_at <= now {
            return Err(ContractError::HoldExpired);
        }
        if Self::active_hold(&env, &patient).is_some() {
            return Err(ContractError::HoldAlreadyActive);
        }

        let hold = RegulatoryHold {
            reason_hash: reason_hash.clone(),
            expires_at,
            placed_at: now,
        };

        env.storage()
            .persistent()
            .set(&DataKey::RegulatoryHold(patient.clone()), &hold);

        env.events().publish(
            (symbol_short!("hold_set"), patient),
            (reason_hash, expires_at, now),
        );
        Ok(())
    }

    pub fn lift_hold(env: Env, patient: Address) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        Self::require_admin(&env);

        let hold = Self::active_hold(&env, &patient).ok_or(ContractError::NotFound)?;
        let lifted_at = env.ledger().timestamp();

        env.storage()
            .persistent()
            .remove(&DataKey::RegulatoryHold(patient.clone()));

        env.events().publish(
            (symbol_short!("hold_lift"), patient),
            (hold.reason_hash, hold.expires_at, hold.placed_at, lifted_at),
        );
        Ok(())
    }

    pub fn is_hold_active(env: Env, patient: Address) -> bool {
        Self::active_hold(&env, &patient).is_some()
    }

    pub fn get_hold(env: Env, patient: Address) -> Option<RegulatoryHold> {
        Self::active_hold(&env, &patient)
    }

    // =====================================================
    //                    DOCTOR LOGIC
    // =====================================================

    pub fn register_doctor(
        env: Env,
        wallet: Address,
        name: String,
        specialization: String,
        certificate_hash: Bytes,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        wallet.require_auth();

        let key = DataKey::Doctor(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyExists);
        }

        let doctor = DoctorData {
            name,
            specialization,
            certificate_hash,
            verified: false,
        };

        env.storage().persistent().set(&key, &doctor);

        let mut doc_list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::DoctorList)
            .unwrap_or(Vec::new(&env));
        doc_list.push_back(wallet.clone());
        env.storage()
            .persistent()
            .set(&DataKey::DoctorList, &doc_list);

        let total_providers: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalProviders)
            .unwrap_or(0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalProviders, &(total_providers + 1));

        env.events()
            .publish((symbol_short!("reg_doc"), wallet), symbol_short!("success"));
        Ok(())
    }

    pub fn verify_doctor(
        env: Env,
        wallet: Address,
        institution_wallet: Address,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        institution_wallet.require_auth();

        let inst_key = DataKey::Institution(institution_wallet);
        if !env.storage().persistent().has(&inst_key) {
            return Err(ContractError::UnauthorizedInstitution);
        }

        let doc_key = DataKey::Doctor(wallet.clone());
        let mut doctor: DoctorData = env
            .storage()
            .persistent()
            .get(&doc_key)
            .ok_or(ContractError::NotFound)?;

        doctor.verified = true;
        env.storage().persistent().set(&doc_key, &doctor);

        env.events().publish(
            (symbol_short!("ver_doc"), wallet),
            symbol_short!("verified"),
        );
        Ok(())
    }

    pub fn get_doctor(env: Env, wallet: Address) -> Result<DoctorData, ContractError> {
        let key = DataKey::Doctor(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::NotFound)
    }

    // =====================================================
    //              INSTITUTION MANAGEMENT
    // =====================================================

    pub fn register_institution(env: Env, institution_wallet: Address) {
        Self::require_not_frozen(&env);
        institution_wallet.require_auth();
        let key = DataKey::Institution(institution_wallet);
        env.storage().persistent().set(&key, &true);

        let total_providers: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalProviders)
            .unwrap_or(0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalProviders, &(total_providers + 1));
    }

    /// Store the provider-registry contract address (admin-only).
    /// Once set, `grant_access` will cross-call `is_provider` before authorising a doctor.
    pub fn set_provider_registry(
        env: Env,
        provider_registry: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &provider_registry);
        Ok(())
    }

    // =====================================================
    //            MEDICAL RECORD ACCESS CONTROL
    // =====================================================

    pub fn grant_access(
        env: Env,
        patient: Address,
        caller: Address,
        doctor: Address,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        require_patient_or_guardian(&env, &patient, &caller)?;
        Self::require_not_on_hold(&env, &patient)?;

        if !Self::is_provider_registered(&env, &doctor) {
            return Err(ContractError::ProviderNotRegistered);
        }

        let key = DataKey::AuthorizedDoctors(patient.clone());
        let mut map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Map::new(&env));

        if !map.contains_key(doctor.clone()) {
            if map.len() >= MAX_AUTHORIZED_DOCTORS {
                return Err(ContractError::InputTooLarge);
            }
            let total_access_grants: u64 = env
                .storage()
                .instance()
                .get(&DataKey::TotalAccessGrants)
                .unwrap_or(0u64);
            env.storage()
                .instance()
                .set(&DataKey::TotalAccessGrants, &(total_access_grants + 1));
        }

        map.set(doctor.clone(), true);
        env.storage().persistent().set(&key, &map);
        Ok(())
    }

    pub fn grant_field_access(
        env: Env,
        patient: Address,
        grantee: Address,
        record_id: u64,
        fields: Vec<FieldPermission>,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        patient.require_auth();
        Self::require_not_on_hold(&env, &patient)?;

        let record_data: RecordData = env
            .storage()
            .persistent()
            .get(&DataKey::MedicalRecord(record_id))
            .ok_or(ContractError::NotFound)?;
        if record_data.patient != patient {
            return Err(ContractError::NotAuthorized);
        }

        let access_key = DataKey::AuthorizedDoctors(patient.clone());
        let access_map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Map::new(&env));
        if !access_map.contains_key(grantee.clone()) {
            return Err(ContractError::NotAuthorized);
        }

        let mask = field_permission_mask(fields);
        env.storage()
            .persistent()
            .set(&DataKey::FieldAccess(patient, grantee, record_id), &mask);
        Ok(())
    }

    pub fn revoke_access(
        env: Env,
        patient: Address,
        caller: Address,
        doctor: Address,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        require_patient_or_guardian(&env, &patient, &caller)?;
        Self::require_not_on_hold(&env, &patient)?;

        let key = DataKey::AuthorizedDoctors(patient.clone());
        let mut map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Map::new(&env));

        if map.contains_key(doctor.clone()) {
            let total_access_grants: u64 = env
                .storage()
                .instance()
                .get(&DataKey::TotalAccessGrants)
                .unwrap_or(0u64);
            let new_total = total_access_grants.saturating_sub(1);
            env.storage()
                .instance()
                .set(&DataKey::TotalAccessGrants, &new_total);
        }

        map.remove(doctor);
        env.storage().persistent().set(&key, &map);
        Ok(())
    }

    pub fn get_authorized_doctors(env: Env, patient: Address) -> Vec<Address> {
        let key = DataKey::AuthorizedDoctors(patient);
        let map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Map::new(&env));

        map.keys()
    }

    // =====================================================
    //                  MEDICAL RECORDS
    // =====================================================

    pub fn add_medical_record(
        env: Env,
        patient: Address,
        doctor: Address,
        encrypted_ref: EncryptedEnvelopeRef,
        record_type: Symbol,
        policy: PolicyMetadata,
    ) -> Result<u64, ContractError> {
        Self::require_not_frozen(&env);
        Self::require_patient_exists(&env, &patient)?;
        doctor.require_auth();
        validate_encrypted_ref(&encrypted_ref)
            .map_err(|_| ContractError::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| ContractError::InvalidPolicyMetadata)?;

        // Collect record fee if set
        let fee: i128 = env
            .storage()
            .instance()
            .get(&DataKey::RecordFee)
            .unwrap_or(0);
        if fee > 0 {
            let token_id: Address = env
                .storage()
                .instance()
                .get(&DataKey::FeeToken)
                .ok_or(ContractError::NotFound)?;
            let treasury: Address = env
                .storage()
                .instance()
                .get(&DataKey::Treasury)
                .ok_or(ContractError::NotFound)?;
            token::TokenClient::new(&env, &token_id).transfer(&doctor, &treasury, &fee);
        }

        // Check consent
        if Self::get_consent_status(env.clone(), patient.clone()) != ConsentStatus::Acknowledged {
            return Err(ContractError::ConsentNotAcknowledged);
        }

        // Check access
        let access_key = DataKey::AuthorizedDoctors(patient.clone());
        let access_map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Map::new(&env));

        if !access_map.contains_key(doctor.clone()) {
            return Err(ContractError::NotAuthorized);
        }

        let timestamp = env.ledger().timestamp();

        // Advance global monotonic record counter (instance storage).
        let mut record_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RecordCounter)
            .unwrap_or(0u64);
        record_id += 1;
        env.storage()
            .instance()
            .set(&DataKey::RecordCounter, &record_id);

        let initial_version = RecordVersion {
            encrypted_ref: encrypted_ref.clone(),
            updated_by: doctor.clone(),
            updated_at: timestamp,
        };

        let record_data = RecordData {
            patient: patient.clone(),
            record_type: record_type.clone(),
            current_ref: encrypted_ref.clone(),
            history: {
                let mut h = Vec::new(&env);
                h.push_back(initial_version);
                h
            },
            latest_version: 1u64,
            policy: policy.clone(),
        };

        let counter_key = DataKey::RecordCounter;
        let record_id: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0u64) + 1;
        env.storage().persistent().set(&counter_key, &record_id);

        let timestamp = env.ledger().timestamp();

        let record = MedicalRecord {
            record_id,
            doctor: doctor.clone(),
            encrypted_ref,
            timestamp,
            record_type: record_type.clone(),
            policy,
        };

        // Store record data (using cloned values)
        env.storage()
            .persistent()
            .set(&DataKey::MedicalRecord(record_id), &record_data);

        // Append to patient's medical record list for quick access
        let mut records: Vec<MedicalRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::MedicalRecords(patient.clone()))
            .unwrap_or(Vec::new(&env));
        records.push_back(record.clone());
        env.storage()
            .persistent()
            .set(&DataKey::MedicalRecords(patient.clone()), &records);

        // Increment total records created
        let total_records: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TotalRecordsCreated)
            .unwrap_or(0u64);
        env.storage()
            .instance()
            .set(&DataKey::TotalRecordsCreated, &(total_records + 1));

        // Append to patient's record IDs
        let ids_key = DataKey::PatientRecordIds(patient.clone());
        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&ids_key)
            .unwrap_or(Vec::new(&env));
        ids.push_back(record_id);
        env.storage().persistent().set(&ids_key, &ids);
        Self::update_merkle_root(&env, &patient, &ids);

        // ── Secondary index update ────────────────────────────────────────────
        // Atomically append (patient, record_id) to the global type index.
        let idx_key = DataKey::GlobalTypeIndex(record_type.clone());
        let mut type_index: Vec<TypeIndexEntry> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));
        type_index.push_back(TypeIndexEntry {
            patient: patient.clone(),
            record_id,
        });
        env.storage().persistent().set(&idx_key, &type_index);
        env.storage()
            .persistent()
            .extend_ttl(&idx_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
        // ─────────────────────────────────────────────────────────────────────

        // TTL bumps for per-patient and per-record keys.
        Self::bump_patient_keys(&env, &patient);
        env.storage().persistent().extend_ttl(
            &DataKey::MedicalRecord(record_id),
            LEDGER_THRESHOLD,
            LEDGER_BUMP_AMOUNT,
        );
        env.storage()
            .persistent()
            .extend_ttl(&ids_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);

        env.events().publish(
            (
                Symbol::new(&env, NEW_RECORD_TOPIC),
                patient.clone(),
                doctor.clone(),
            ),
            (record_id, record_type, timestamp),
        );

        Ok(record_id)
    }

    pub fn get_medical_records(
        env: Env,
        patient: Address,
        caller: Address,
    ) -> Result<Vec<MedicalRecord>, ContractError> {
        let patient_key = DataKey::Patient(patient.clone());
        if let Some(data) = env
            .storage()
            .persistent()
            .get::<DataKey, PatientData>(&patient_key)
        {
            if data.status == PatientStatus::Deregistered {
                let admin: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::Admin)
                    .ok_or(ContractError::NotFound)?;
                if caller != admin {
                    return Err(ContractError::NotAuthorized);
                }
                caller.require_auth();
            } else {
                require_record_access(&env, &patient, &caller)?;
            }
        } else {
            require_record_access(&env, &patient, &caller)?;
        }

        let key = DataKey::MedicalRecords(patient.clone());

        if env.storage().persistent().has(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
        }

        let patient_key = DataKey::Patient(patient.clone());
        if env.storage().persistent().has(&patient_key) {
            env.storage().persistent().extend_ttl(
                &patient_key,
                LEDGER_THRESHOLD,
                LEDGER_BUMP_AMOUNT,
            );
        }

        Ok(env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env)))
    }

    /// Paginated variant of `get_medical_records`.
    ///
    /// Returns up to `limit` records starting at `offset` (0-based) together
    /// with the patient's total record count so callers can page through all
    /// records without loading the entire list in one call.
    ///
    /// # Constraints
    /// - `limit` must be in `1..=MAX_PAGE_SIZE` (50). Returns
    ///   `InvalidPagination` otherwise.
    /// - `offset` beyond the end of the list is not an error — it returns an
    ///   empty `records` vec with the correct `total`.
    ///
    /// # Access control
    /// Same as `get_medical_records`: patient, guardian, or authorized doctor.
    pub fn get_medical_records_paged(
        env: Env,
        patient: Address,
        caller: Address,
        offset: u32,
        limit: u32,
    ) -> Result<PagedRecords, ContractError> {
        // ── Validate pagination params ────────────────────────────────────────
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(ContractError::InvalidPagination);
        }

        // ── Access control (mirrors get_medical_records) ──────────────────────
        let patient_key = DataKey::Patient(patient.clone());
        if let Some(data) = env
            .storage()
            .persistent()
            .get::<DataKey, PatientData>(&patient_key)
        {
            if data.status == PatientStatus::Deregistered {
                let admin: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::Admin)
                    .ok_or(ContractError::NotFound)?;
                if caller != admin {
                    return Err(ContractError::NotAuthorized);
                }
                caller.require_auth();
            } else {
                require_record_access(&env, &patient, &caller)?;
            }
        } else {
            require_record_access(&env, &patient, &caller)?;
        }

        // ── Load full list (single persistent read) ───────────────────────────
        let key = DataKey::MedicalRecords(patient.clone());

        if env.storage().persistent().has(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
        }
        if env.storage().persistent().has(&patient_key) {
            env.storage().persistent().extend_ttl(
                &patient_key,
                LEDGER_THRESHOLD,
                LEDGER_BUMP_AMOUNT,
            );
        }

        let all_records: Vec<MedicalRecord> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));

        let total = all_records.len();

        // ── Slice [offset, offset+limit) ──────────────────────────────────────
        let mut page: Vec<MedicalRecord> = Vec::new(&env);

        if offset < total {
            let end = (offset + limit).min(total);
            for i in offset..end {
                if let Some(record) = all_records.get(i) {
                    page.push_back(record);
                }
            }
        }

        Ok(PagedRecords {
            records: page,
            total,
        })
    }

    pub fn get_latest_record(        env: Env,
        patient: Address,
        caller: Address,
    ) -> Result<MedicalRecord, ContractError> {
        let patient_key = DataKey::Patient(patient.clone());
        if let Some(data) = env
            .storage()
            .persistent()
            .get::<DataKey, PatientData>(&patient_key)
        {
            if data.status == PatientStatus::Deregistered {
                let admin: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::Admin)
                    .ok_or(ContractError::NotFound)?;
                if caller != admin {
                    return Err(ContractError::NotAuthorized);
                }
            } else {
                require_record_access(&env, &patient, &caller)?;
            }
        } else {
            require_record_access(&env, &patient, &caller)?;
        }

        let key = DataKey::MedicalRecords(patient.clone());
        let records: Vec<MedicalRecord> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));

        if records.is_empty() {
            return Err(ContractError::NoRecordsFound);
        }

        // Bump TTL as in get_medical_records
        if env.storage().persistent().has(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
        }
        if env.storage().persistent().has(&patient_key) {
            env.storage().persistent().extend_ttl(
                &patient_key,
                LEDGER_THRESHOLD,
                LEDGER_BUMP_AMOUNT,
            );
        }

        let mut latest = records.get(0).ok_or(ContractError::NoRecordsFound)?.clone();
        for r in records.iter() {
            if r.timestamp > latest.timestamp {
                latest = r.clone();
            }
        }

        Ok(latest)
    }

    /// Merkle root over the patient's ordered record IDs (see `merkle` module).
    /// If no root was persisted yet, recomputes from `PatientRecordIds` (or empty sentinel).
    pub fn get_merkle_root(env: Env, patient: Address) -> BytesN<32> {
        let key = DataKey::MerkleRoot(patient.clone());
        if let Some(root) = env.storage().persistent().get::<DataKey, BytesN<32>>(&key) {
            root
        } else {
            let ids_key = DataKey::PatientRecordIds(patient);
            let ids: Vec<u64> = env
                .storage()
                .persistent()
                .get(&ids_key)
                .unwrap_or(Vec::new(&env));
            merkle::compute_merkle_root(&env, &ids)
        }
    }

    /// Returns true iff `proof` is a valid Merkle membership proof for `record_id` under this patient's root.
    pub fn verify_record_membership(
        env: Env,
        patient: Address,
        record_id: u64,
        proof: Vec<BytesN<32>>,
    ) -> bool {
        let root = Self::get_merkle_root(env.clone(), patient);
        merkle::verify_membership(&env, record_id, &proof, &root)
    }

    /// Generate a Merkle membership proof for the record at `record_index`
    /// (0-based position within `PatientRecordIds`, insertion order -- not
    /// a record ID) in `patient`'s record tree.
    ///
    /// The returned proof, together with `hash_leaf(env, record_id)`, can be
    /// passed to `verify_record_membership`; or pass the leaf hash directly
    /// to `verify_record_proof`. Returns `ContractError::RecordNotFound` if
    /// `record_index` is out of bounds.
    pub fn get_record_proof(
        env: Env,
        patient: Address,
        record_index: u32,
    ) -> Result<Vec<BytesN<32>>, ContractError> {
        let ids_key = DataKey::PatientRecordIds(patient);
        let record_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&ids_key)
            .unwrap_or(Vec::new(&env));

        if record_index >= record_ids.len() {
            return Err(ContractError::RecordNotFound);
        }

        Ok(merkle::generate_proof(&env, &record_ids, record_index))
    }

    /// Returns true iff `proof` is a valid Merkle membership proof for
    /// `leaf_hash` under this patient's root.
    ///
    /// Like `verify_record_membership` but takes the leaf hash directly
    /// rather than a raw record ID -- pair with `get_record_proof` and
    /// `merkle::hash_leaf` when the caller already has (or only needs) the
    /// hash.
    pub fn verify_record_proof(
        env: Env,
        patient: Address,
        leaf_hash: BytesN<32>,
        proof: Vec<BytesN<32>>,
    ) -> bool {
        let root = Self::get_merkle_root(env.clone(), patient);
        merkle::verify_leaf_membership(&env, leaf_hash, &proof, &root)
    }

    /// Returns all records for `patient` whose `record_type` matches the given symbol.
    /// Access control: caller must be the patient, their guardian, or an authorized doctor.
    /// Returns an empty vec (not an error) when no records match.
    pub fn update_record(
        env: Env,
        caller: Address,
        record_id: u64,
        new_encrypted_ref: EncryptedEnvelopeRef,
        policy: PolicyMetadata,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);

        let record_key = DataKey::MedicalRecord(record_id);
        let mut record_data: RecordData = env
            .storage()
            .persistent()
            .get(&record_key)
            .ok_or(ContractError::NotFound)?;

        let patient = record_data.patient.clone();
        Self::require_patient_exists(&env, &patient)?;
        Self::require_not_on_hold(&env, &patient)?;

        caller.require_auth();
        require_record_access(&env, &patient, &caller)?;

        validate_encrypted_ref(&new_encrypted_ref)
            .map_err(|_| ContractError::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| ContractError::InvalidPolicyMetadata)?;

        let timestamp = env.ledger().timestamp();

        // Append new version to history, then update current_ref
        let new_version = RecordVersion {
            encrypted_ref: new_encrypted_ref.clone(),
            updated_by: caller.clone(),
            updated_at: timestamp,
        };
        record_data.history.push_back(new_version);
        record_data.current_ref = new_encrypted_ref;
        record_data.policy = policy;
        record_data.latest_version += 1;

        env.storage().persistent().set(&record_key, &record_data);

        // TTL bump
        Self::bump_patient_keys(&env, &patient);
        env.storage()
            .persistent()
            .extend_ttl(&record_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);

        env.events().publish(
            (symbol_short!("rec_upd"), (patient.clone(), caller.clone())),
            record_id,
        );

        Ok(())
    }

    pub fn get_record_history(
        env: Env,
        record_id: u64,
        caller: Address,
        page: u32,
    ) -> Result<PageResult, ContractError> {
        caller.require_auth();
        let record_key = DataKey::MedicalRecord(record_id);
        let record_data: RecordData = env
            .storage()
            .persistent()
            .get(&record_key)
            .ok_or(ContractError::NotFound)?;
        require_record_access(&env, &record_data.patient, &caller)?;

        // TTL bump
        env.storage()
            .persistent()
            .extend_ttl(&record_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);

        if record_data.history.is_empty() {
            return Err(ContractError::NoHistoryFound);
        }

        let total = record_data.history.len();
        let limit = 20u32;
        let offset = page * limit;

        let mut page_items: Vec<soroban_sdk::Val> = Vec::new(&env);
        let mut has_more = false;

        if offset < total {
            let end = (offset + limit).min(total);
            for i in offset..end {
                if let Some(version) = record_data.history.get(i) {
                    page_items.push_back(version.into_val(&env));
                }
            }
            has_more = end < total;
        }

        Ok(PageResult {
            ids: page_items,
            has_more,
        })
    }

    pub fn get_record_fields(
        env: Env,
        patient: Address,
        caller: Address,
        record_id: u64,
    ) -> PartialRecord {
        caller.require_auth();

        let record_data: RecordData = match env
            .storage()
            .persistent()
            .get(&DataKey::MedicalRecord(record_id))
        {
            Some(record) => record,
            None => return empty_partial_record(),
        };
        if record_data.patient != patient {
            return empty_partial_record();
        }

        let guardian_key = DataKey::Guardian(patient.clone());
        let guardian_opt: Option<Address> = env.storage().persistent().get(&guardian_key);
        let mask = if caller == patient || guardian_opt.as_ref() == Some(&caller) {
            FIELD_ALL
        } else {
            let access_key = DataKey::AuthorizedDoctors(patient.clone());
            let access_map: Map<Address, bool> = env
                .storage()
                .persistent()
                .get(&access_key)
                .unwrap_or(Map::new(&env));
            if !access_map.contains_key(caller.clone()) {
                return empty_partial_record();
            }

            env.storage()
                .persistent()
                .get(&DataKey::FieldAccess(patient, caller, record_id))
                .unwrap_or(0u32)
        };

        build_partial_record(&record_data, mask)
    }

    pub fn get_records_by_type(
        env: Env,
        patient: Address,
        caller: Address,
        record_type: Symbol,
    ) -> Result<Vec<MedicalRecord>, ContractError> {
        require_record_access(&env, &patient, &caller)?;

        let ids_key = DataKey::PatientRecordIds(patient);
        let record_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&ids_key)
            .unwrap_or(Vec::new(&env));

        let mut filtered = Vec::new(&env);
        for id in record_ids.iter() {
            let record_id: u64 = id;
            if let Some(record_data) = env
                .storage()
                .persistent()
                .get::<DataKey, RecordData>(&DataKey::MedicalRecord(record_id))
            {
                if record_data.record_type == record_type {
                    // Map to MedicalRecord for compatibility
                    let first_version =
                        record_data.history.get(0).ok_or(ContractError::NotFound)?;
                    let mr = MedicalRecord {
                        record_id,
                        doctor: first_version.updated_by.clone(),
                        encrypted_ref: record_data.current_ref.clone(),
                        timestamp: first_version.updated_at,
                        record_type: record_type.clone(),
                        policy: record_data.policy.clone(),
                    };
                    filtered.push_back(mr);
                }
            }
        }
        Ok(filtered)
    }

    /// Returns records by positional IDs for a patient.
    ///
    /// `ids` can contain up to 10 entries. Missing IDs are either skipped
    /// (`strict_not_found = false`) or cause a panic (`strict_not_found = true`).
    pub fn get_records_by_ids(
        env: Env,
        patient: Address,
        caller: Address,
        ids: Vec<u32>,
        strict_not_found: bool,
    ) -> Result<Vec<MedicalRecord>, ContractError> {
        if ids.len() > 10 {
            return Err(ContractError::TooManyIds);
        }
        require_record_access(&env, &patient, &caller)?;

        let key = DataKey::MedicalRecords(patient.clone());
        let records: Vec<MedicalRecord> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));

        let mut selected = Vec::new(&env);
        for id in ids.iter() {
            match records.get(id) {
                Some(record) => selected.push_back(record),
                None => {
                    if strict_not_found {
                        return Err(ContractError::NotFound);
                    }
                }
            }
        }

        Ok(selected)
    }

    /// Extend TTL for active patient record keys without changing record data.
    pub fn extend_record_ttl(env: Env, patient: Address) -> Result<(), ContractError> {
        Self::require_patient_exists(&env, &patient)?;
        Self::bump_patient_keys(&env, &patient);

        let ids_key = DataKey::PatientRecordIds(patient.clone());
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&ids_key)
            .unwrap_or(Vec::new(&env));
        for record_id in ids.iter() {
            let key = DataKey::MedicalRecord(record_id);
            if env.storage().persistent().has(&key) {
                env.storage()
                    .persistent()
                    .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
            }
        }

        Ok(())
    }

    /// Archive a hot medical record by replacing on-chain detail with a CID hash.
    pub fn archive_record(
        env: Env,
        patient: Address,
        record_id: u64,
        cid_hash: BytesN<32>,
    ) -> Result<ArchivedRecordRef, ContractError> {
        patient.require_auth();
        validate_nonzero_hash(&cid_hash).map_err(|_| ContractError::InvalidCID)?;

        let record_key = DataKey::MedicalRecord(record_id);
        let record_data: RecordData = env
            .storage()
            .persistent()
            .get(&record_key)
            .ok_or(ContractError::NotFound)?;
        if record_data.patient != patient {
            return Err(ContractError::NotAuthorized);
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::ArchivedRecord(record_id))
        {
            return Err(ContractError::AlreadyDeleted);
        }

        let archived = ArchivedRecordRef {
            patient: patient.clone(),
            record_id,
            record_type: record_data.record_type.clone(),
            cid_hash,
            archived_at_ledger: env.ledger().sequence(),
        };
        let archive_key = DataKey::ArchivedRecord(record_id);
        env.storage().persistent().set(&archive_key, &archived);
        env.storage().persistent().extend_ttl(
            &archive_key,
            ARCHIVE_LEDGER_THRESHOLD,
            ARCHIVE_LEDGER_BUMP_AMOUNT,
        );

        env.storage().persistent().remove(&record_key);
        env.storage()
            .persistent()
            .set(&DataKey::DeletedRecord(record_id), &env.ledger().timestamp());

        let records_key = DataKey::MedicalRecords(patient.clone());
        let records: Vec<MedicalRecord> = env
            .storage()
            .persistent()
            .get(&records_key)
            .unwrap_or(Vec::new(&env));
        let mut active_records = Vec::new(&env);
        for record in records.iter() {
            if record.record_id != record_id {
                active_records.push_back(record);
            }
        }
        env.storage()
            .persistent()
            .set(&records_key, &active_records);

        let ids_key = DataKey::PatientRecordIds(patient.clone());
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&ids_key)
            .unwrap_or(Vec::new(&env));
        let mut active_ids = Vec::new(&env);
        for id in ids.iter() {
            if id != record_id {
                active_ids.push_back(id);
            }
        }
        env.storage().persistent().set(&ids_key, &active_ids);
        Self::update_merkle_root(&env, &patient, &active_ids);
        Self::bump_patient_keys(&env, &patient);

        env.events().publish(
            (symbol_short!("archived"), patient),
            (record_id, archived.cid_hash.clone()),
        );
        Ok(archived)
    }

    /// Retrieve the archival pointer used by off-chain storage/indexers.
    pub fn get_archived_ref(
        env: Env,
        record_id: u64,
    ) -> Result<ArchivedRecordRef, ContractError> {
        env.storage()
            .persistent()
            .get(&DataKey::ArchivedRecord(record_id))
            .ok_or(ContractError::NotFound)
    }

    // =====================================================
    //                  STATE SNAPSHOT
    // =====================================================

    /// Emit a full-state snapshot as events for off-chain reconstruction.
    ///
    /// # Rate limit
    /// Once every 100,000 ledgers (~5-6 days on Stellar mainnet).
    ///
    /// # Emitted events
    /// 1. `snap_meta` — topics: `("snap_meta", ledger_sequence)`,
    ///    data: `(patient_count, doctor_count, consent_version)`
    ///
    /// 2. `snap_pats` — topics: `("snap_pats", ledger_sequence)`,
    ///    data: `Vec<Address>` of all registered patient addresses.
    ///
    /// 3. `snap_docs` — topics: `("snap_docs", ledger_sequence)`,
    ///    data: `Vec<Address>` of all registered doctor addresses.
    pub fn emit_state_snapshot(env: Env) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotFound)?;
        admin.require_auth();

        const SNAPSHOT_INTERVAL: u32 = 100_000;
        let current_ledger = env.ledger().sequence();
        let last: Option<u32> = env.storage().instance().get(&DataKey::LastSnapshotLedger);

        if let Some(last_ledger) = last {
            if current_ledger.saturating_sub(last_ledger) < SNAPSHOT_INTERVAL {
                return Err(ContractError::SnapshotRateLimit);
            }
        }

        env.storage()
            .instance()
            .set(&DataKey::LastSnapshotLedger, &current_ledger);

        let patients: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientList)
            .unwrap_or(Vec::new(&env));
        let doctors: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::DoctorList)
            .unwrap_or(Vec::new(&env));
        let consent_version: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::ConsentVersion)
            .unwrap_or(BytesN::from_array(&env, &[0u8; 32]));

        let patient_count = patients.len();
        let doctor_count = doctors.len();

        env.events().publish(
            (symbol_short!("snap_meta"), current_ledger),
            (patient_count, doctor_count, consent_version),
        );
        env.events()
            .publish((symbol_short!("snap_pats"), current_ledger), patients);
        env.events()
            .publish((symbol_short!("snap_docs"), current_ledger), doctors);
        Ok(())
    }

    pub fn get_last_snapshot_ledger(env: Env) -> Option<u32> {
        env.storage().instance().get(&DataKey::LastSnapshotLedger)
    }

    // =====================================================
    //              PATIENT-CONTROLLED SHARE LINKS
    // =====================================================

    /// Create a time-limited, use-counted sharing token for a single record.
    ///
    /// Token = sha256(patient_bytes || record_id_be || nonce_be || expires_at_be)
    ///
    /// # Arguments
    /// * `patient`    - The patient who owns the record (must auth).
    /// * `record_id`  - 0-based index into the patient's medical records vec.
    /// * `uses_remaining` - How many times the token may be used (must be > 0).
    /// * `expires_at` - Unix timestamp after which the token is invalid.
    pub fn create_share_link(
        env: Env,
        patient: Address,
        record_id: u64,
        uses_remaining: u32,
        expires_at: u64,
    ) -> Result<BytesN<32>, ContractError> {
        patient.require_auth();

        if uses_remaining == 0 {
            return Err(ContractError::InvalidToken);
        }
        if expires_at <= env.ledger().timestamp() {
            return Err(ContractError::InvalidToken);
        }

        // Verify the record_id is in-bounds.
        let records_key = DataKey::MedicalRecords(patient.clone());
        let records: Vec<MedicalRecord> = env
            .storage()
            .persistent()
            .get(&records_key)
            .unwrap_or(Vec::new(&env));
        if record_id >= records.len() as u64 {
            return Err(ContractError::InvalidToken);
        }

        // Increment per-patient nonce.
        let nonce_key = DataKey::ShareNonce(patient.clone());
        let nonce: u64 = env.storage().persistent().get(&nonce_key).unwrap_or(0u64);
        let next_nonce = nonce + 1;
        env.storage().persistent().set(&nonce_key, &next_nonce);

        // Build preimage: patient address bytes (32) + record_id (8) + nonce (8) + expires_at (8)
        let patient_bytes = patient.clone().to_xdr(&env);
        let mut preimage = Bytes::new(&env);
        preimage.append(&patient_bytes);
        preimage.extend_from_array(&record_id.to_be_bytes());
        preimage.extend_from_array(&next_nonce.to_be_bytes());
        preimage.extend_from_array(&expires_at.to_be_bytes());

        let token: BytesN<32> = env.crypto().sha256(&preimage).into();

        // Reject duplicate tokens — prevents a nonce collision from silently
        // overwriting an active link and granting unintended access.
        if env
            .storage()
            .persistent()
            .has(&DataKey::ShareLink(token.clone()))
        {
            return Err(ContractError::AlreadyExists);
        }

        let link = ShareLinkData {
            patient: patient.clone(),
            record_id,
            uses_remaining,
            expires_at,
        };
        env.storage()
            .persistent()
            .set(&DataKey::ShareLink(token.clone()), &link);

        env.events().publish(
            (symbol_short!("sl_create"), patient),
            (token.clone(), record_id, uses_remaining, expires_at),
        );

        Ok(token)
    }

    /// Redeem a share link token to read a single medical record.
    ///
    /// Any address may call this function. The token is validated for expiry
    /// and remaining uses; uses_remaining is decremented on success and the
    /// token is removed when it reaches zero.
    pub fn use_share_link(env: Env, token: BytesN<32>) -> Result<MedicalRecord, ContractError> {
        let link_key = DataKey::ShareLink(token.clone());
        let mut link: ShareLinkData = env
            .storage()
            .persistent()
            .get(&link_key)
            .ok_or(ContractError::InvalidToken)?;

        // Check expiry.
        if env.ledger().timestamp() >= link.expires_at {
            env.storage().persistent().remove(&link_key);
            return Err(ContractError::InvalidToken);
        }

        // Check uses.
        if link.uses_remaining == 0 {
            env.storage().persistent().remove(&link_key);
            return Err(ContractError::InvalidToken);
        }

        // Fetch the record.
        let records_key = DataKey::MedicalRecords(link.patient.clone());
        let records: Vec<MedicalRecord> = env
            .storage()
            .persistent()
            .get(&records_key)
            .unwrap_or(Vec::new(&env));
        let record = records
            .get(link.record_id as u32)
            .ok_or(ContractError::InvalidToken)?;

        // Decrement uses.
        link.uses_remaining -= 1;
        if link.uses_remaining == 0 {
            env.storage().persistent().remove(&link_key);
        } else {
            env.storage().persistent().set(&link_key, &link);
        }

        env.events().publish(
            (symbol_short!("sl_use"), token),
            (link.patient, link.record_id, link.uses_remaining),
        );

        Ok(record)
    }

    /// Create a one-hour export authorization ticket for a patient's data.
    pub fn request_data_export(env: Env, patient: Address) -> ExportTicket {
        patient.require_auth();

        let issued_at = env.ledger().timestamp();
        let expires_at = issued_at.saturating_add(3600);
        let nonce = next_export_nonce(&env, &patient, issued_at);
        let signature = sign_export_ticket(&env, &patient, issued_at, expires_at, &nonce);

        ExportTicket {
            patient,
            issued_at,
            expires_at,
            nonce,
            signature,
        }
    }

    /// Validate a patient data export ticket for expiry and integrity.
    pub fn validate_export_ticket(env: Env, ticket: ExportTicket) -> bool {
        if env.ledger().timestamp() > ticket.expires_at {
            return false;
        }

        let expected_signature = sign_export_ticket(
            &env,
            &ticket.patient,
            ticket.issued_at,
            ticket.expires_at,
            &ticket.nonce,
        );

        expected_signature == ticket.signature
    }

    // =====================================================
    //           GLOBAL SECONDARY INDEX (ADMIN)
    // =====================================================

    /// Soft-delete a record: marks it as deleted and atomically removes it from
    /// the global type index.
    ///
    /// Callable by the owning patient, their guardian, or an authorized doctor.
    /// After deletion the record data is retained for audit purposes but will no
    /// longer appear in index queries.
    pub fn soft_delete_record(
        env: Env,
        record_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);

        let record_key = DataKey::MedicalRecord(record_id);
        let record_data: RecordData = env
            .storage()
            .persistent()
            .get(&record_key)
            .ok_or(ContractError::NotFound)?;

        let patient = record_data.patient.clone();
        Self::require_patient_exists(&env, &patient)?;
        require_record_access(&env, &patient, &caller)?;

        // Guard: already deleted?
        if env
            .storage()
            .persistent()
            .has(&DataKey::DeletedRecord(record_id))
        {
            return Err(ContractError::AlreadyDeleted);
        }

        // Stamp the tombstone.
        env.storage().persistent().set(
            &DataKey::DeletedRecord(record_id),
            &env.ledger().timestamp(),
        );

        // ── Secondary index update ────────────────────────────────────────────
        // Remove this entry from the global type index atomically.
        let idx_key = DataKey::GlobalTypeIndex(record_data.record_type.clone());
        let type_index: Vec<TypeIndexEntry> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));

        let mut updated = Vec::new(&env);
        for entry in type_index.iter() {
            if entry.record_id != record_id {
                updated.push_back(entry);
            }
        }
        env.storage().persistent().set(&idx_key, &updated);
        env.storage()
            .persistent()
            .extend_ttl(&idx_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
        // ─────────────────────────────────────────────────────────────────────

        // Recompute Merkle root so outstanding proofs against the old root are invalidated.
        let ids_key = DataKey::PatientRecordIds(patient.clone());
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&ids_key)
            .unwrap_or(Vec::new(&env));
        Self::update_merkle_root(&env, &patient, &ids);

        env.events().publish(
            (symbol_short!("rec_del"), patient),
            (record_id, env.ledger().timestamp()),
        );

        Ok(())
    }

    /// Returns every (patient, record_id) pair indexed under `record_type`.
    ///
    /// **Admin-only.** Non-admins receive `NotAuthorized`.
    pub fn get_global_records_by_type(
        env: Env,
        record_type: Symbol,
    ) -> Result<Vec<TypeIndexEntry>, ContractError> {
        Self::require_admin(&env);

        let idx_key = DataKey::GlobalTypeIndex(record_type);
        let index: Vec<TypeIndexEntry> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));

        if env.storage().persistent().has(&idx_key) {
            env.storage()
                .persistent()
                .extend_ttl(&idx_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
        }

        Ok(index)
    }

    /// Returns the count of active (non-deleted) records of the given type
    /// across all patients.
    ///
    /// **Admin-only.** Non-admins receive `NotAuthorized`.
    pub fn get_global_type_count(env: Env, record_type: Symbol) -> Result<u64, ContractError> {
        Self::require_admin(&env);

        let idx_key = DataKey::GlobalTypeIndex(record_type);
        let index: Vec<TypeIndexEntry> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));

        Ok(index.len() as u64)
    }

    // =====================================================
    //              INCIDENT TRACKING
    // =====================================================

    /// Capture an incident for this contract, optionally linking it to a
    /// cross-contract correlation ID.  Returns the new incident ID.
    pub fn report_incident(
        env: Env,
        reporter: Address,
        error_code: u32,
        description: String,
        correlation_id: Option<BytesN<32>>,
    ) -> u64 {
        reporter.require_auth();
        capture_incident(
            &env,
            IncidentSeverity::High,
            String::from_str(&env, "patient-registry"),
            error_code,
            description,
            reporter,
            correlation_id,
        )
    }

    /// Return all incident IDs that share the given correlation ID.
    pub fn get_incidents_by_correlation_id(
        env: Env,
        correlation_id: BytesN<32>,
    ) -> Vec<u64> {
        shared_get_by_corr(&env, correlation_id)
    }

    // =====================================================
    //              RETENTION CLASS MANAGEMENT
    // =====================================================

    /// Update a patient's data retention class.
    ///
    /// Admin-only. Changing the class takes effect on the next TTL bump so that
    /// subsequent operations use the appropriate `ttl-config` thresholds.
    pub fn set_patient_retention_class(
        env: Env,
        patient: Address,
        class: RetentionClass,
    ) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        Self::require_admin(&env);
        Self::require_patient_exists(&env, &patient)?;

        let key = DataKey::Patient(patient.clone());
        let mut data: PatientData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::NotFound)?;
        data.retention_class = class.clone();
        env.storage().persistent().set(&key, &data);

        env.events().publish(
            (symbol_short!("ret_cls"), patient),
            symbol_short!("updated"),
        );
        Ok(())
    }

    /// Retrieve the current retention class for a patient.
    pub fn get_patient_retention_class(
        env: Env,
        patient: Address,
    ) -> Result<RetentionClass, ContractError> {
        let key = DataKey::Patient(patient);
        let data: PatientData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::NotFound)?;
        Ok(data.retention_class)
    }

    // =====================================================
    //                  PRIVATE HELPERS
    // =====================================================

    /// Returns true if the provider-registry is not configured, or if the address
    /// is a registered and active provider in the configured registry.
    fn is_provider_registered(env: &Env, provider: &Address) -> bool {
        let registry: Address = match env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ProviderRegistry)
        {
            Some(r) => r,
            None => return true,
        };
        let args: Vec<soroban_sdk::Val> = vec![env, provider.clone().into_val(env)];
        env.invoke_contract(&registry, &Symbol::new(env, "is_provider"), args)
    }

    fn require_not_frozen(env: &Env) {
        let frozen: bool = env
            .storage()
            .instance()
            .get(&DataKey::Frozen)
            .unwrap_or(false);
        if frozen {
            panic_with_error!(env, ContractError::ContractFrozen);
        }
    }

    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, ContractError::NotFound));
        admin.require_auth();
    }

    fn require_patient_exists(env: &Env, patient: &Address) -> Result<(), ContractError> {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Patient(patient.clone()))
        {
            return Err(ContractError::NotFound);
        }
        Ok(())
    }

    fn require_not_on_hold(env: &Env, patient: &Address) -> Result<(), ContractError> {
        if Self::active_hold(env, patient).is_some() {
            return Err(ContractError::HoldAlreadyActive);
        }
        Ok(())
    }

    fn active_hold(env: &Env, patient: &Address) -> Option<RegulatoryHold> {
        let key = DataKey::RegulatoryHold(patient.clone());
        let hold: Option<RegulatoryHold> = env.storage().persistent().get(&key);

        match hold {
            Some(existing) if existing.expires_at > env.ledger().timestamp() => Some(existing),
            Some(_) => {
                env.storage().persistent().remove(&key);
                None
            }
            None => None,
        }
    }

    /// Recompute and persist the Merkle root for `patient` from their current
    /// record-ID list.  Called by `add_medical_record` after every insertion.
    fn update_merkle_root(env: &Env, patient: &Address, ids: &Vec<u64>) {
        let root = merkle::compute_merkle_root(env, ids);
        let root_key = DataKey::MerkleRoot(patient.clone());
        env.storage().persistent().set(&root_key, &root);
        env.storage()
            .persistent()
            .extend_ttl(&root_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
    }

    /// Bump TTL for all persistent keys belonging to a patient using the
    /// appropriate `ttl-config` helper for their retention class.
    fn bump_patient_keys(env: &Env, patient: &Address) {
        let class = env
            .storage()
            .persistent()
            .get::<DataKey, PatientData>(&DataKey::Patient(patient.clone()))
            .map(|d| d.retention_class)
            .unwrap_or(RetentionClass::Clinical);

        let keys: [DataKey; 6] = [
            DataKey::Patient(patient.clone()),
            DataKey::MedicalRecords(patient.clone()),
            DataKey::AuthorizedDoctors(patient.clone()),
            DataKey::PatientRecordIds(patient.clone()),
            DataKey::ConsentAck(patient.clone()),
            DataKey::MerkleRoot(patient.clone()),
        ];
        for key in keys.iter() {
            extend_for_retention_class(env, key, &class);
        }
    }

    /// Verify and increment caller's nonce for cross-contract call protection.
    /// Returns an error if the provided nonce is <= the last successful nonce.
    fn verify_and_increment_nonce(
        env: &Env,
        caller: &Address,
        provided_nonce: u64,
    ) -> Result<(), ContractError> {
        let nonce_key = DataKey::CallerNonce(caller.clone());
        let last_nonce: u64 = env
            .storage()
            .persistent()
            .get(&nonce_key)
            .unwrap_or(0);

        // Reject if provided nonce is not strictly greater than last successful nonce
        if provided_nonce <= last_nonce {
            return Err(ContractError::StaleNonce);
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
