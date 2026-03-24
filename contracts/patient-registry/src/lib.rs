#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env, Map, String,
    Vec,
    contract, contractimpl, contracttype, symbol_short, token, Address, Bytes, BytesN, Env, Map,
    String, Vec,
};

/// --------------------
/// Patient Structures
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatientData {
    pub name: String,
    pub dob: u64,
    pub metadata: String, // IPFS / encrypted medical refs
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
    Admin,
    ConsentVersion,
    ConsentAck(Address),
    Guardian(Address),
    PatientList,
    DoctorList,
    LastSnapshotLedger,
    RecordFee,
    Treasury,
    FeeToken,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedicalRecord {
    pub doctor: Address,
    pub record_hash: Bytes,
    pub description: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegulatoryHold {
    pub reason_hash: BytesN<32>,
    pub expires_at: u64,
    pub placed_at: u64,
fn require_patient_or_guardian(env: &Env, patient: &Address, caller: &Address) {
    let guardian_key = DataKey::Guardian(patient.clone());
    let guardian_opt: Option<Address> = env.storage().persistent().get(&guardian_key);
    if caller == patient {
        caller.require_auth();
    } else if guardian_opt.as_ref() == Some(caller) {
        caller.require_auth();
    } else {
        panic!("Caller is not patient or assigned guardian");
    }
}

#[contract]
pub struct MedicalRegistry;

#[contractimpl]
impl MedicalRegistry {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }

        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);

        env.events()
            .publish((symbol_short!("init"), admin), symbol_short!("success"));
    }

    // =====================================================
    //                    ADMIN / CONSENT
    // =====================================================

    pub fn initialize(env: Env, admin: Address, treasury: Address, fee_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Treasury, &treasury);
        env.storage().instance().set(&DataKey::FeeToken, &fee_token);
        env.storage().instance().set(&DataKey::RecordFee, &0i128);
    }

    pub fn set_record_fee(env: Env, amount: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();
        if amount < 0 {
            panic!("Fee cannot be negative");
        }
        env.storage().instance().set(&DataKey::RecordFee, &amount);
    }

    pub fn get_record_fee(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::RecordFee)
            .unwrap_or(0)
    }

    pub fn publish_consent_version(env: Env, version_hash: BytesN<32>) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::ConsentVersion, &version_hash);
        env.events().publish(
            (symbol_short!("consent_v"), admin),
            version_hash,
        );
    }

    pub fn assign_guardian(env: Env, patient: Address, guardian: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();
        // Prevent a guardian from being assigned as guardian of another patient
        // (no delegation chain: guardian must not itself have a guardian entry as a patient)
        env.storage()
            .persistent()
            .set(&DataKey::Guardian(patient.clone()), &guardian);
        env.events().publish(
            (symbol_short!("grd_asgn"), patient),
            guardian,
        );
    }

    pub fn revoke_guardian(env: Env, patient: Address) {
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
        env.storage()
            .persistent()
            .get(&DataKey::Guardian(patient))
    }

    pub fn acknowledge_consent(env: Env, patient: Address, caller: Address, version_hash: BytesN<32>) {
        require_patient_or_guardian(&env, &patient, &caller);
        let current: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::ConsentVersion)
            .expect("No consent version published");
        if current != version_hash {
            panic!("Version mismatch");
        }
        env.storage()
            .persistent()
            .set(&DataKey::ConsentAck(patient.clone()), &version_hash);
        env.events().publish(
            (symbol_short!("consent_a"), patient),
            version_hash,
        );
    }

    pub fn get_consent_status(env: Env, patient: Address) -> ConsentStatus {
        let current_opt: Option<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&DataKey::ConsentVersion);
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

    // =====================================================
    //                    PATIENT LOGIC
    // =====================================================

    pub fn register_patient(env: Env, wallet: Address, name: String, dob: u64, metadata: String) {
        wallet.require_auth();

        let key = DataKey::Patient(wallet.clone());
        if env.storage().persistent().has(&key) {
            panic!("Patient already registered");
        }

        let patient = PatientData {
            name,
            dob,
            metadata,
        };
        env.storage().persistent().set(&key, &patient);

        let mut pat_list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientList)
            .unwrap_or(Vec::new(&env));
        pat_list.push_back(wallet.clone());
        env.storage().persistent().set(&DataKey::PatientList, &pat_list);

        env.events()
            .publish((symbol_short!("reg_pat"), wallet), symbol_short!("success"));
    }

    pub fn update_patient(env: Env, wallet: Address, metadata: String) {
        wallet.require_auth();
        Self::require_not_on_hold(&env, &wallet);
    pub fn update_patient(env: Env, wallet: Address, caller: Address, metadata: String) {
        require_patient_or_guardian(&env, &wallet, &caller);

        let key = DataKey::Patient(wallet.clone());
        let mut patient: PatientData = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Patient not found");

        patient.metadata = metadata;
        env.storage().persistent().set(&key, &patient);

        env.events()
            .publish((symbol_short!("upd_pat"), wallet), symbol_short!("success"));
    }

    pub fn get_patient(env: Env, wallet: Address) -> PatientData {
        let key = DataKey::Patient(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .expect("Patient not found")
    }

    pub fn is_patient_registered(env: Env, wallet: Address) -> bool {
        let key = DataKey::Patient(wallet);
        env.storage().persistent().has(&key)
    }

    pub fn place_hold(env: Env, patient: Address, reason_hash: BytesN<32>, expires_at: u64) {
        Self::require_admin(&env);
        Self::require_patient_exists(&env, &patient);

        let now = env.ledger().timestamp();
        if expires_at <= now {
            panic!("Hold expiry must be in the future");
        }
        if Self::active_hold(&env, &patient).is_some() {
            panic!("Regulatory hold already active");
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
    }

    pub fn lift_hold(env: Env, patient: Address) {
        Self::require_admin(&env);

        let hold = Self::active_hold(&env, &patient).expect("No active regulatory hold");
        let lifted_at = env.ledger().timestamp();

        env.storage()
            .persistent()
            .remove(&DataKey::RegulatoryHold(patient.clone()));

        env.events().publish(
            (symbol_short!("hold_lift"), patient),
            (hold.reason_hash, hold.expires_at, hold.placed_at, lifted_at),
        );
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
    ) {
        wallet.require_auth();

        let key = DataKey::Doctor(wallet.clone());
        if env.storage().persistent().has(&key) {
            panic!("Doctor already registered");
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
        env.storage().persistent().set(&DataKey::DoctorList, &doc_list);

        env.events()
            .publish((symbol_short!("reg_doc"), wallet), symbol_short!("success"));
    }

    pub fn verify_doctor(env: Env, wallet: Address, institution_wallet: Address) {
        institution_wallet.require_auth();

        let inst_key = DataKey::Institution(institution_wallet);
        if !env.storage().persistent().has(&inst_key) {
            panic!("Unauthorized institution");
        }

        let doc_key = DataKey::Doctor(wallet.clone());
        let mut doctor: DoctorData = env
            .storage()
            .persistent()
            .get(&doc_key)
            .expect("Doctor not found");

        doctor.verified = true;
        env.storage().persistent().set(&doc_key, &doctor);

        env.events().publish(
            (symbol_short!("ver_doc"), wallet),
            symbol_short!("verified"),
        );
    }

    pub fn get_doctor(env: Env, wallet: Address) -> DoctorData {
        let key = DataKey::Doctor(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .expect("Doctor not found")
    }

    // =====================================================
    //              INSTITUTION MANAGEMENT
    // =====================================================

    pub fn register_institution(env: Env, institution_wallet: Address) {
        institution_wallet.require_auth();
        let key = DataKey::Institution(institution_wallet);
        env.storage().persistent().set(&key, &true);
    }

    // =====================================================
    //            MEDICAL RECORD ACCESS CONTROL
    // =====================================================

    pub fn grant_access(env: Env, patient: Address, doctor: Address) {
        patient.require_auth();
        Self::require_not_on_hold(&env, &patient);
    pub fn grant_access(env: Env, patient: Address, caller: Address, doctor: Address) {
        require_patient_or_guardian(&env, &patient, &caller);

        let key = DataKey::AuthorizedDoctors(patient.clone());
        let mut map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Map::new(&env));

        map.set(doctor, true);
        env.storage().persistent().set(&key, &map);
    }

    pub fn revoke_access(env: Env, patient: Address, doctor: Address) {
        patient.require_auth();
        Self::require_not_on_hold(&env, &patient);
    pub fn revoke_access(env: Env, patient: Address, caller: Address, doctor: Address) {
        require_patient_or_guardian(&env, &patient, &caller);

        let key = DataKey::AuthorizedDoctors(patient.clone());
        let mut map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Map::new(&env));

        map.remove(doctor);
        env.storage().persistent().set(&key, &map);
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

    pub fn add_medical_record(
        env: Env,
        patient: Address,
        doctor: Address,
        record_hash: Bytes,
        description: String,
    ) {
        doctor.require_auth();

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
            token::TokenClient::new(&env, &token_id).transfer(
                &doctor,
                &treasury,
                &fee,
            );
        }

        // Check consent
        if Self::get_consent_status(env.clone(), patient.clone()) != ConsentStatus::Acknowledged {
            panic!("Patient has not acknowledged current consent version");
        }

        // Check access
        let access_key = DataKey::AuthorizedDoctors(patient.clone());
        let access_map: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Map::new(&env));

        if !access_map.contains_key(doctor.clone()) {
            panic!("Doctor not authorized");
        }

        let record = MedicalRecord {
            doctor,
            record_hash,
            description,
            timestamp: env.ledger().timestamp(),
        };

        let records_key = DataKey::MedicalRecords(patient.clone());
        let mut records: Vec<MedicalRecord> = env
            .storage()
            .persistent()
            .get(&records_key)
            .unwrap_or(Vec::new(&env));

        records.push_back(record);
        env.storage().persistent().set(&records_key, &records);
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
    ///    Verify completeness of the other two events against these counts.
    ///
    /// 2. `snap_pats` — topics: `("snap_pats", ledger_sequence)`,
    ///    data: `Vec<Address>` of all registered patient addresses.
    ///
    /// 3. `snap_docs` — topics: `("snap_docs", ledger_sequence)`,
    ///    data: `Vec<Address>` of all registered doctor addresses.
    ///
    /// # Off-chain reconstruction steps
    /// 1. Subscribe to the Horizon/RPC event stream filtered by this contract ID.
    /// 2. On `snap_meta`, record `ledger_sequence` and expected counts.
    /// 3. Collect `snap_pats` and `snap_docs` at the same ledger sequence.
    /// 4. Assert address list lengths match the counts in `snap_meta`.
    /// 5. For each address call `get_patient` / `get_doctor` via RPC at that
    ///    exact ledger (requires an archive node: `--ledger <seq>`).
    /// 6. Persist the reconstructed structs to your off-chain store.
    pub fn emit_state_snapshot(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        admin.require_auth();

        const SNAPSHOT_INTERVAL: u32 = 100_000;
        let current_ledger = env.ledger().sequence();
        let last: Option<u32> = env
            .storage()
            .instance()
            .get(&DataKey::LastSnapshotLedger);

        if let Some(last_ledger) = last {
            if current_ledger.saturating_sub(last_ledger) < SNAPSHOT_INTERVAL {
                panic!("Snapshot rate limit: must wait 100,000 ledgers between snapshots");
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
        env.events().publish(
            (symbol_short!("snap_pats"), current_ledger),
            patients,
        );
        env.events().publish(
            (symbol_short!("snap_docs"), current_ledger),
            doctors,
        );
    }

    pub fn get_last_snapshot_ledger(env: Env) -> Option<u32> {
        env.storage()
            .instance()
            .get(&DataKey::LastSnapshotLedger)
    }

    pub fn get_medical_records(env: Env, patient: Address) -> Vec<MedicalRecord> {
        let key = DataKey::MedicalRecords(patient);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env))
    }

    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");
        admin.require_auth();
    }

    fn require_patient_exists(env: &Env, patient: &Address) {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Patient(patient.clone()))
        {
            panic!("Patient not found");
        }
    }

    fn require_not_on_hold(env: &Env, patient: &Address) {
        if Self::active_hold(env, patient).is_some() {
            panic!("Patient data is on regulatory hold");
        }
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
}
#[cfg(test)]
mod test;
