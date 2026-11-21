#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Emergency Medical Information Contract
//!
//! Manages emergency contact information, medical allergies, and critical health conditions
//! accessible during medical emergencies with encrypted storage and priority-based access.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Emergency responders authenticated via role validation.
//! Patient authorization for emergency info registration. Contact information access restricted
//! to authorized emergency personnel. Allergy and condition data accessible only during emergencies
//! or with patient consent.
//!
//! **Audit Controls:** Emergency access events emitted with responder identity and timestamp.
//! Contact priority levels tracked for triage. Critical condition flags logged. All emergency
//! info retrievals tracked for breach notifications and compliance auditing.
//!
//! **Data Retention Policy:** Emergency contacts retained indefinitely for rapid access. Medical
//! allergies and critical conditions stored persistently. Priority metadata maintained for
//! emergency response optimization. Patient deregistration removes all emergency medical data.
//!
//! **Encryption/Integrity:** Contact information stored as encrypted hashes (BytesN<32>).
//! Contact labels hashed to prevent disclosure of relationship type. Relationship class symbols
//! enumerated (family, friend, emergency contact, etc.). Emergency access limited to time-bound
//! overrides with justification tracking.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Symbol, Vec,
};

/// --------------------
/// Emergency Structures
/// --------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyContact {
    pub contact_label_hash: BytesN<32>,
    pub relationship_class: Symbol,
    pub contact_hash: BytesN<32>, // Encrypted contact info
    pub priority: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyProfile {
    pub blood_type: Symbol,
    pub critical_allergy_hashes: Vec<BytesN<32>>,
    pub active_condition_hashes: Vec<BytesN<32>>,
    pub current_medication_hashes: Vec<BytesN<32>>,
    pub dnr_status: bool,
    pub emergency_contacts: Vec<EmergencyContact>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalAlert {
    pub provider_id: Address,
    pub alert_type: Symbol,
    pub alert_text_hash: BytesN<32>,
    pub severity: Symbol,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyAccessLog {
    pub provider_id: Address,
    pub emergency_type: Symbol,
    pub justification_hash: BytesN<32>,
    pub location_hash: BytesN<32>,
    pub access_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DNROrder {
    pub provider_id: Address,
    pub dnr_document_hash: BytesN<32>,
    pub effective_date: u64,
    pub recorded_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryConfig {
    pub emergency_contact: Address,
    pub guardians: Vec<Address>,
    pub recovery_threshold: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryProposal {
    pub new_owner: Address,
    pub approvals: Vec<Address>,
}

/// --------------------
/// Storage Keys
/// --------------------

#[contracttype]
pub enum DataKey {
    EmergencyProfile(Address),
    CriticalAlerts(Address),
    /// Append-only, bounded break-glass audit log entry, keyed by
    /// (patient, sequence). Sequence is a per-patient monotonically
    /// increasing counter so prior entries are never overwritten.
    EmergencyAccessLog(Address, u32),
    /// Per-patient counter tracking the next sequence number for the
    /// break-glass audit log.
    EmergencyAccessLogSeq(Address),
    DNROrder(Address),
    EmergencyNotifications(Address),
    RecoveryConfig(Address),
    RecoveryProposal(Address),
    Admin,
    /// Registered emergency responder -> active flag. Only a registered,
    /// active responder may invoke the break-glass `emergency_access_request`.
    Responder(Address),
    /// Break-glass access grant expiry timestamp, keyed by (responder, patient).
    /// A grant is valid only until this ledger timestamp and only for the
    /// specific patient it was requested for; afterwards the responder must
    /// issue a fresh `emergency_access_request`.
    EmergencyAccessGrant(Address, Address),
}

/// Maximum number of hashes allowed in `critical_allergy_hashes`.
/// Inputs exceeding this cap are rejected to prevent unbounded Vec growth.
pub const MAX_CRITICAL_ALLERGIES: u32 = 50;

/// Maximum number of break-glass audit log entries retained per patient.
/// Once the ring buffer is full, the oldest entry is evicted so storage
/// stays bounded while recent access history remains auditable.
pub const MAX_EMERGENCY_ACCESS_LOG_ENTRIES: u32 = 64;

/// Time window (in seconds) during which a break-glass emergency access
/// grant remains valid. Matches access-control's 1-hour emergency override.
/// After this window elapses, the responder must issue a fresh
/// `emergency_access_request` to regain read access.
pub const EMERGENCY_ACCESS_WINDOW_SECONDS: u64 = 3600;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    EmergencyProfileNotFound = 1,
    NotAuthorized = 2,
    InvalidRecoveryThreshold = 3,
    TooManyCriticalAllergies = 4,
    NotInitialized = 5,
    AlreadyInitialized = 6,
    NotRegisteredResponder = 7,
    EmergencyAccessExpired = 8,
}

#[contract]
pub struct EmergencyMedicalInfo;

#[contractimpl]
impl EmergencyMedicalInfo {
    /// Configure the admin address allowed to manage the emergency responder registry.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Register an address as an authorized emergency responder.
    /// Only the configured admin may grant responder status.
    pub fn register_responder(env: Env, admin: Address, responder: Address) -> Result<(), Error> {
        Self::assert_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::Responder(responder), &true);
        Ok(())
    }

    /// Revoke a previously registered emergency responder.
    /// Only the configured admin may revoke responder status.
    pub fn revoke_responder(env: Env, admin: Address, responder: Address) -> Result<(), Error> {
        Self::assert_admin(&env, &admin)?;
        env.storage().persistent().remove(&DataKey::Responder(responder));
        Ok(())
    }

    /// Check whether `responder` is currently a registered emergency responder.
    pub fn is_registered_responder(env: Env, responder: Address) -> bool {
        env.storage().persistent().has(&DataKey::Responder(responder))
    }

    fn assert_admin(env: &Env, admin: &Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if *admin != stored_admin {
            return Err(Error::NotAuthorized);
        }
        Ok(())
    }

    /// Append a break-glass audit log entry for `patient_id` and emit an
    /// event describing the access. The log is append-only and bounded: a
    /// per-patient sequence counter keys each entry so prior entries are
    /// never overwritten, and once `MAX_EMERGENCY_ACCESS_LOG_ENTRIES` is
    /// reached the oldest entry is evicted (ring buffer).
    ///
    /// The event payload contains only audit metadata (responder, patient,
    /// emergency type, justification hash, timestamp) — never profile content.
    fn record_emergency_access(
        env: &Env,
        patient_id: &Address,
        responder: &Address,
        emergency_type: &Symbol,
        justification_hash: &BytesN<32>,
        location_hash: &BytesN<32>,
    ) {
        let access_time = env.ledger().timestamp();

        let seq_key = DataKey::EmergencyAccessLogSeq(patient_id.clone());
        let seq: u32 = env.storage().persistent().get(&seq_key).unwrap_or(0);

        let log = EmergencyAccessLog {
            provider_id: responder.clone(),
            emergency_type: emergency_type.clone(),
            justification_hash: justification_hash.clone(),
            location_hash: location_hash.clone(),
            access_time,
        };
        env.storage()
            .persistent()
            .set(&DataKey::EmergencyAccessLog(patient_id.clone(), seq), &log);

        // Evict the oldest entry once the bounded ring buffer is full.
        if seq >= MAX_EMERGENCY_ACCESS_LOG_ENTRIES {
            let evict_seq = seq - MAX_EMERGENCY_ACCESS_LOG_ENTRIES;
            env.storage().persistent().remove(&DataKey::EmergencyAccessLog(
                patient_id.clone(),
                evict_seq,
            ));
        }

        env.storage().persistent().set(&seq_key, &(seq + 1));

        env.events().publish(
            (
                Symbol::new(env, "emergency_access"),
                patient_id.clone(),
                responder.clone(),
            ),
            (
                emergency_type.clone(),
                justification_hash.clone(),
                access_time,
            ),
        );
    }

    /// Set or update emergency profile for a patient
    /// Sub-second access optimized with persistent storage
    #[allow(clippy::too_many_arguments)]
    pub fn set_emergency_profile(
        env: Env,
        patient_id: Address,
        blood_type: Symbol,
        critical_allergy_hashes: Vec<BytesN<32>>,
        critical_condition_hashes: Vec<BytesN<32>>,
        current_medication_hashes: Vec<BytesN<32>>,
        emergency_contacts: Vec<EmergencyContact>,
        advance_directives_hash: Option<BytesN<32>>,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        if critical_allergy_hashes.len() > MAX_CRITICAL_ALLERGIES {
            return Err(Error::TooManyCriticalAllergies);
        }

        let profile = EmergencyProfile {
            blood_type,
            critical_allergy_hashes,
            active_condition_hashes: critical_condition_hashes,
            current_medication_hashes,
            dnr_status: false,
            emergency_contacts,
        };

        let key = DataKey::EmergencyProfile(patient_id.clone());
        env.storage().persistent().set(&key, &profile);
        Ok(())
    }
}
