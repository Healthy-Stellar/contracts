#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    xdr::ToXdr, Address, Bytes, BytesN, Env, Map, String, Symbol, Vec,
};

pub mod merkle;
pub mod validation;
pub const NEW_RECORD_TOPIC: &str = "new_record";

// =====================================================
//                    TTL CONSTANTS
// =====================================================

/// Bump persistent entries by ~31 days (535,680 ledgers at ~5s/ledger).
pub const LEDGER_BUMP_AMOUNT: u32 = 535_680;

/// Extend TTL when fewer than ~30 days remain (518,400 ledgers).
pub const LEDGER_THRESHOLD: u32 = 518_400;

/// --------------------
/// Patient Status
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatientStatus {
    Active,
    Deregistered,
}

/// --------------------
/// Patient Structures
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatientData {
    pub name: String,
    pub dob: u64,
    pub metadata: String, // IPFS / encrypted medical refs
    pub status: PatientStatus,
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
    /// Platform-wide secondary index: record_type → Vec<TypeIndexEntry>.
    GlobalTypeIndex(Symbol),
    /// Soft-delete tombstone for a record (value: timestamp of deletion).
    DeletedRecord(u64),
    /// Merkle root for a patient's records.
    /// Merkle root over the patient's ordered record IDs (see `merkle` module).
    MerkleRoot(Address),
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
    pub record_hash: Bytes,
    pub description: String,
    pub timestamp: u64,
    pub record_type: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordVersion {
    pub ipfs_hash: Bytes,
    pub updated_by: Address,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordData {
    pub patient: Address,
    pub record_type: Symbol,
    pub description: String,
    pub current_ipfs: Bytes,
    pub history: Vec<RecordVersion>,
    pub latest_version: u64,
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
}

pub fn validate_cid(cid: &Bytes) -> Result<(), ContractError> {
    let len = cid.len() as usize;
    if len == 0 || len > 512 {
        return Err(ContractError::InvalidCID);
    }
    let mut buf = [0u8; 512];
    for i in 0..len {
        buf[i] = cid.get(i as u32).ok_or(ContractError::InvalidCID)?;
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

fn require_patient_or_guardian(env: &Env, patient: &Address, caller: &Address) -> Result<(), ContractError> {
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

    env.crypto().sha256(&preimage)
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

    env.crypto().sha256(&payload)
}

/// Enforces that `caller` is the patient, their guardian, or an authorized doctor.
fn require_record_access(env: &Env, patient: &Address, caller: &Address) -> Result<(), ContractError> {
    if caller == patient {
        caller.require_auth();
        return Ok(());
    }
    let guardian_key = DataKey::Guardian(patient.clone());
    let guardian_opt: Option<Address> = env.storage().persistent().get(&guardian_key);
    if guardian_opt.as_ref() == Some(caller) {
        caller.require_auth();
        return Ok(());
    }
    let access_key = DataKey::AuthorizedDoctors(patient.clone());
    let access_map: Map<Address, bool> = env
        .storage()
        .persistent()
        .get(&access_key)
        .unwrap_or(Map::new(env));
    if access_map.contains_key(caller.clone()) {
        caller.require_auth();
        return Ok(());
    }
    Err(ContractError::NotAuthorized)
}

#[contract]
pub struct MedicalRegistry;

#[contractimpl]
impl MedicalRegistry {
    // =====================================================
    //                    ADMIN / CONSENT
    // =====================================================

    pub fn initialize(env: Env, admin: Address, treasury: Address, fee_token: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
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
            .expect("Not initialized");
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
            .expect("Not initialized");
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
            .expect("Not initialized");
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
            .expect("Not initialized");
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
            .expect("No consent version published");
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

    pub fn register_patient(env: Env, wallet: Address, name: String, dob: u64, metadata: String) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        wallet.require_auth();

        let key = DataKey::Patient(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::AlreadyExists);
        }

        let patient = PatientData {
            name,
            dob,
            metadata,
            status: PatientStatus::Active,
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

    pub fn update_patient(env: Env, wallet: Address, caller: Address, metadata: String) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        require_patient_or_guardian(&env, &wallet, &caller)?;
        Self::require_not_on_hold(&env, &wallet)?;

        let key = DataKey::Patient(wallet.clone());
        let mut patient: PatientData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::NotFound)?;

        patient.metadata = metadata;
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

        data.status = PatientStatus::Deregistered;
        env.storage().persistent().set(&key, &data);

        env.storage().persistent().set(
            &DataKey::Deregistered(patient.clone()),
            &env.ledger().timestamp(),
        );

        env.storage()
            .persistent()
            .remove(&DataKey::AuthorizedDoctors(patient.clone()));

        env.events().publish(
            (symbol_short!("pat_dreg"), patient),
            env.ledger().timestamp(),
        );
        Ok(())
    }

    pub fn get_total_patients(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::TotalPatients)
            .unwrap_or(0)
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
            .expect("Not initialized");

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

        // Extend Patient record TTL
        let patient_key = DataKey::Patient(patient.clone());
        if env.storage().persistent().has(&patient_key) {
            env.storage().persistent().extend_ttl(
                &patient_key,
                LEDGER_THRESHOLD,
                LEDGER_BUMP_AMOUNT,
            );
        }

        // Extend MedicalRecords TTL
        let records_key = DataKey::MedicalRecords(patient.clone());
        if env.storage().persistent().has(&records_key) {
            env.storage().persistent().extend_ttl(
                &records_key,
                LEDGER_THRESHOLD,
                LEDGER_BUMP_AMOUNT,
            );
        }

        // Extend AuthorizedDoctors TTL
        let access_key = DataKey::AuthorizedDoctors(patient.clone());
        if env.storage().persistent().has(&access_key) {
            env.storage().persistent().extend_ttl(
                &access_key,
                LEDGER_THRESHOLD,
                LEDGER_BUMP_AMOUNT,
            );
        }

        // Extend ConsentAck TTL
        let consent_key = DataKey::ConsentAck(patient.clone());
        if env.storage().persistent().has(&consent_key) {
            env.storage().persistent().extend_ttl(
                &consent_key,
                LEDGER_THRESHOLD,
                LEDGER_BUMP_AMOUNT,
            );
        }
    }

    pub fn place_hold(env: Env, patient: Address, reason_hash: BytesN<32>, expires_at: u64) -> Result<(), ContractError> {
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

    pub fn verify_doctor(env: Env, wallet: Address, institution_wallet: Address) -> Result<(), ContractError> {
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

    // =====================================================
    //            MEDICAL RECORD ACCESS CONTROL
    // =====================================================

    pub fn grant_access(env: Env, patient: Address, caller: Address, doctor: Address) -> Result<(), ContractError> {
        Self::require_not_frozen(&env);
        require_patient_or_guardian(&env, &patient, &caller)?;
        Self::require_not_on_hold(&env, &patient)?;

        let key = DataKey::AuthorizedDoctors(patient.clone());
        let mut map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Map::new(&env));

        if !map.contains_key(doctor.clone()) {
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

    pub fn revoke_access(env: Env, patient: Address, caller: Address, doctor: Address) -> Result<(), ContractError> {
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
        record_hash: Bytes,
        description: String,
        record_type: Symbol,
    ) -> Result<u64, ContractError> {
        Self::require_not_frozen(&env);
        Self::require_patient_exists(&env, &patient)?;
        doctor.require_auth();
        validate_cid(&record_hash)?;

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
                .expect("Fee token not configured");
            let treasury: Address = env
                .storage()
                .instance()
                .get(&DataKey::Treasury)
                .expect("Treasury not configured");
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
        }     }

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
            ipfs_hash: record_hash.clone(),
            updated_by: doctor.clone(),
            updated_at: timestamp,
        };

        let record_data = RecordData {
            patient: patient.clone(),
            record_type: record_type.clone(),
            description: description.clone(),
            current_ipfs: record_hash.clone(),
            history: {
                let mut h = Vec::new(&env);
                h.push_back(initial_version);
                h
            },
            latest_version: 1u64,
        };

        let counter_key = DataKey::RecordCounter;
        let record_id: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0u64) + 1;
        env.storage().persistent().set(&counter_key, &record_id);

        let timestamp = env.ledger().timestamp();

        let record = MedicalRecord {
            record_id,
            doctor: doctor.clone(),
            record_hash,
            description,
            timestamp,
            record_type: record_type.clone(),
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

    pub fn get_medical_records(env: Env, patient: Address, caller: Address) -> Result<Vec<MedicalRecord>, ContractError> {
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
                    .expect("Not initialized");
                if caller != admin {
                    return Err(ContractError::NotAuthorized);
                }
            }
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

    pub fn get_latest_record(
        env: Env,
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
                    .expect("Not initialized");
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

        let mut latest = records.get(0).unwrap().clone();
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

    /// Returns all records for `patient` whose `record_type` matches the given symbol.
    /// Access control: caller must be the patient, their guardian, or an authorized doctor.
    /// Returns an empty vec (not an error) when no records match.
    pub fn update_record(
        env: Env,
        caller: Address,
        record_id: u64,
        new_ipfs_hash: Bytes,
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

        validate_cid(&new_ipfs_hash)?;

        let timestamp = env.ledger().timestamp();

        // Append old current to history
        let old_version = RecordVersion {
            ipfs_hash: record_data.current_ipfs.clone(),
            updated_by: caller.clone(),
            updated_at: timestamp,
        };
        record_data.history.push_back(old_version);
        record_data.current_ipfs = new_ipfs_hash;
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
    ) -> Result<Vec<RecordVersion>, ContractError> {
        let record_key = DataKey::MedicalRecord(record_id);
        let record_data: RecordData = env
            .storage()
            .persistent()
            .get(&record_key)
            .ok_or(ContractError::InvalidCID)?;

        // TTL bump
        env.storage()
            .persistent()
            .extend_ttl(&record_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);

        Ok(record_data.history)
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
            let record_id: u64 = id.into();
            if let Some(record_data) = env
                .storage()
                .persistent()
                .get::<DataKey, RecordData>(&DataKey::MedicalRecord(record_id))
            {
                if record_data.record_type == record_type {
                    // Map to MedicalRecord for compatibility
                    let mr = MedicalRecord {
                        record_id,
                        doctor: record_data
                            .history
                            .get(0)
                            .map(|v| v.updated_by.clone())
                            .unwrap_or_else(|| {
                                Address::from_str(
                                    &env,
                                    "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
                                )
                            }),
                        record_hash: record_data.current_ipfs.clone(),
                        description: record_data.description.clone(),
                        timestamp: record_data
                            .history
                            .get(0)
                            .map(|v| v.updated_at)
                            .unwrap_or(0),
                        record_type: record_type.clone(),
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
            .expect("Not initialized");
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
        if env.storage().persistent().has(&DataKey::DeletedRecord(record_id)) {
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
    //                  PRIVATE HELPERS
    // =====================================================

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
            .expect("Contract not initialized");
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

    /// Bump TTL for all critical persistent keys belonging to a patient.
    fn bump_patient_keys(env: &Env, patient: &Address) {
        let keys: [DataKey; 6] = [
            DataKey::Patient(patient.clone()),
            DataKey::MedicalRecords(patient.clone()),
            DataKey::AuthorizedDoctors(patient.clone()),
            DataKey::PatientRecordIds(patient.clone()),
            DataKey::ConsentAck(patient.clone()),
            DataKey::MerkleRoot(patient.clone()),
        ];
        for key in keys.iter() {
            if env.storage().persistent().has(key) {
                env.storage()
                    .persistent()
                    .extend_ttl(key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
            }
        }
    }
}

#[cfg(test)]
mod test;
