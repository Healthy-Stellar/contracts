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
    /// Patient-authored advance directive hash. Stored separately from
    /// `DNROrder` so a patient can never overwrite a clinician-recorded
    /// DNR order.
    AdvanceDirective(Address),
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
    /// Configured healthcare proxy authorized to act on a patient's behalf
    /// for DNR record/revoke decisions, keyed by patient.
    HealthcareProxy(Address),
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

    /// Configure the healthcare proxy authorized to act on `patient_id`'s
    /// behalf for DNR record/revoke decisions. Only the patient may set it.
    pub fn set_healthcare_proxy(
        env: Env,
        patient_id: Address,
        proxy: Address,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::HealthcareProxy(patient_id), &proxy);
        Ok(())
    }

    /// Record a DNR order for `patient_id`.
    ///
    /// Requires the caller to be a registered responder AND the patient (or
    /// their configured healthcare proxy) to authorize the change. Emits a
    /// `dnr_set` event including the acting provider.
    pub fn record_dnr_order(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        dnr_document_hash: BytesN<32>,
        effective_date: u64,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        if !Self::is_registered_responder(env.clone(), provider_id.clone()) {
            return Err(Error::NotRegisteredResponder);
        }
        Self::require_patient_or_proxy_auth(&env, &patient_id)?;

        let order = DNROrder {
            provider_id: provider_id.clone(),
            dnr_document_hash,
            effective_date,
            recorded_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::DNROrder(patient_id.clone()), &order);

        env.events().publish(
            (Symbol::new(&env, "dnr_set"), patient_id.clone()),
            (provider_id, dnr_document_hash, effective_date),
        );
        Ok(())
    }

    /// Revoke a DNR order for `patient_id`.
    ///
    /// Requires the caller to be a registered responder AND the patient (or
    /// their configured healthcare proxy) to authorize the change. Emits a
    /// `dnr_revoked` event including the acting provider.
    pub fn revoke_dnr_order(
        env: Env,
        provider_id: Address,
        patient_id: Address,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        if !Self::is_registered_responder(env.clone(), provider_id.clone()) {
            return Err(Error::NotRegisteredResponder);
        }
        Self::require_patient_or_proxy_auth(&env, &patient_id)?;

        env.storage()
            .persistent()
            .remove(&DataKey::DNROrder(patient_id.clone()));

        env.events().publish(
            (Symbol::new(&env, "dnr_revoked"), patient_id.clone()),
            provider_id,
        );
        Ok(())
    }

    /// Require authorization from the patient or their configured healthcare
    /// proxy. The patient's own auth is always accepted; if a proxy is
    /// configured, the proxy's auth is also accepted.
    fn require_patient_or_proxy_auth(env: &Env, patient_id: &Address) -> Result<(), Error> {
        patient_id.require_auth();
        if let Some(proxy) = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::HealthcareProxy(patient_id.clone()))
        {
            proxy.require_auth();
        }
        Ok(())
    }

    /// Append a break-glass audit log entry for `patient_id` and emit an
    /// event describing the access. The log is append-only and bounded: a
    /// per-patient sequence counter keys each entry so prior entries are
    /// never overwritten, and once `MAX_EMERGENCY_ACCESS_LOG_ENTRIES` is
    

/* … truncated 267 chars — edit only what you need near the top … */
