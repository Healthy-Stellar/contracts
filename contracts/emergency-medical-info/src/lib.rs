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
    /// Ledger timestamp at which the proposal was created. Used to enforce
    /// the recovery timelock before `execute_recovery` may run.
    pub created_at: u64,
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

/// Maximum number of critical alerts retained per patient. Once the cap is
/// reached, `add_critical_alert` evicts the oldest alert so the stored Vec
/// cannot grow unbounded and exceed the ledger-entry size limit (which would
/// otherwise permanently brick the alerting path for that patient).
pub const MAX_CRITICAL_ALERTS: u32 = 50;

/// Time window (in seconds) during which a break-glass emergency access
/// grant remains valid. Matches access-control's 1-hour emergency override.
/// After this window elapses, the responder must issue a fresh
/// `emergency_access_request` to regain read access.
pub const EMERGENCY_ACCESS_WINDOW_SECONDS: u64 = 3600;

/// Timelock (in seconds) that must elapse after a recovery proposal reaches
/// its approval threshold before `execute_recovery` may migrate ownership.
/// During this window the patient may cancel a malicious proposal.
pub const RECOVERY_TIMELOCK_SECONDS: u64 = 86_400;

/// --------------------
/// TTL Management
/// --------------------

/// TTL configuration for emergency medical data. Emergency profiles, DNR
/// orders and the responder registry must never be archived mid-emergency,
/// so all critical entries are bumped on every read and write.
mod ttl_config {
    use soroban_sdk::{Env, IntoVal, Val};

    /// Minimum TTL (in ledgers) for critical emergency entries.
    pub const CRITICAL_THRESHOLD: u32 = 100_000;
    /// Target TTL (in ledgers) applied when extending critical entries.
    pub const CRITICAL_EXTEND_TO: u32 = 500_000;

    /// Extend the TTL of a persistent entry so it is not archived.
    pub fn extend_critical_ttl<K: IntoVal<Env, Val>>(env: &Env, key: &K) {
        env.storage()
            .persistent()
            .extend_ttl(key, CRITICAL_THRESHOLD, CRITICAL_EXTEND_TO);
    }

    /// Extend the TTL of the contract instance entry (holds `Admin`).
    pub fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(CRITICAL_THRESHOLD, CRITICAL_EXTEND_TO);
    }
}

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
    RecoveryProposalNotFound = 9,
    RecoveryTargetMismatch = 10,
    RecoveryThresholdNotMet = 11,
    RecoveryTimelockNotElapsed = 12,
    DuplicateGuardian = 13,
}

#[contract]
pub struct EmergencyMedicalInfo;

#[contractimpl]
impl EmergencyMedicalInfo {
    /// Configure the admin address allowed to manage the emergency responder registry.
    /// Runs once at deployment, so the deployer (not the first caller) becomes admin.
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        ttl_config::extend_instance_ttl(&env);
    }

    /// Register an address as an authorized emergency responder.
    /// Only the configured admin may grant responder status.
    pub fn register_responder(env: Env, admin: Address, responder: Address) -> Result<(), Error> {
        Self::assert_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::Responder(responder.clone()), &true);
        ttl_config::extend_critical_ttl(&env, &DataKey::Responder(responder));
        ttl_config::extend_instance_ttl(&env);
        Ok(())
    }

    /// Revoke a previously registered emergency responder.
    /// Only the configured admin may revoke responder status.
    pub fn revoke_responder(env: Env, admin: Address, responder: Address) -> Result<(), Error> {
        Self::assert_admin(&env, &admin)?;
        env.storage().persistent().remove(&DataKey::Responder(responder));
        ttl_config::extend_instance_ttl(&env);
        Ok(())
    }

    /// Append a critical alert for a patient.
    ///
    /// The stored alert list is capped at `MAX_CRITICAL_ALERTS`. When the cap
    /// is reached the oldest alert is evicted so the entry can never grow past
    /// the ledger-entry size limit (which would otherwise permanently brick
    /// the alerting path for that patient).
    pub fn add_critical_alert(
        env: Env,
        provider: Address,
        patient: Address,
        alert_type: Symbol,
        alert_text_hash: BytesN<32>,
        severity: Symbol,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        provider.require_auth();
        Self::assert_responder(&env, &provider)?;

        let key = DataKey::CriticalAlerts(patient.clone());
        let mut alerts: Vec<CriticalAlert> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        // Evict the oldest alert(s) so the Vec stays bounded. Alerts are
        // appended in chronological order, so index 0 is the oldest.
        while alerts.len() >= MAX_CRITICAL_ALERTS {
            alerts.remove(0);
        }
        Self::require_patient_or_proxy_auth(&env, &patient_id)?;

        alerts.push_back(CriticalAlert {
            provider_id: provider.clone(),
            alert_type,
            alert_text_hash,
            severity,
            timestamp: env.ledger().timestamp(),
        });

        env.storage().persistent().set(&key, &alerts);
        ttl_config::extend_critical_ttl(&env, &key);
        Ok(())
    }

    /// Read the critical alerts for a patient.
    ///
    /// Access is restricted to the patient themselves, an active break-glass
    /// grant holder, or a registered responder. Every successful read is
    /// logged for compliance auditing.
    pub fn get_critical_alerts(
        env: Env,
        caller: Address,
        patient: Address,
    ) -> Result<Vec<CriticalAlert>, Error> {
        caller.require_auth();
        Self::assert_read_access(&env, &caller, &patient)?;

        let key = DataKey::CriticalAlerts(patient.clone());
        let alerts: Vec<CriticalAlert> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));
        ttl_config::extend_critical_ttl(&env, &key);

        Self::log_access(&env, &caller, &patient, Symbol::new(&env, "read_alerts"));
        Ok(alerts)
    }

    /// Read the DNR order for a patient.
    ///
    /// Access is restricted to the patient themselves, an active break-glass
    /// grant holder, or a registered responder. Every successful read is
    /// logged for compliance auditing.
    pub fn get_dnr_order(
        env: Env,
        caller: Address,
        patient: Address,
    ) -> Result<Option<DNROrder>, Error> {
        caller.require_auth();
        Self::assert_read_access(&env, &caller, &patient)?;

        let key = DataKey::DNROrder(patient.clone());
        let order: Option<DNROrder> = env.storage().persistent().get(&key);
        ttl_config::extend_critical_ttl(&env, &key);

        Self::log_access(&env, &caller, &patient, Symbol::new(&env, "read_dnr"));
        Ok(order)
    }

    /// Check whether a patient has an emergency profile on file.
    ///
    /// Access is restricted to the patient themselves, an active break-glass
    /// grant holder, or a registered responder. Every successful read is
    /// logged for compliance auditing.
    pub fn has_emergency_profile(
        env: Env,
        caller: Address,
        patient: Address,
    ) -> Result<bool, Error> {
        caller.require_auth();
        Self::assert_read_access(&env, &caller, &patient)?;

        let key = DataKey::EmergencyProfile(patient.clone());
        let exists = env.storage().persistent().has(&key);
        ttl_config::extend_critical_ttl(&env, &key);

        Self::log_access(&env, &caller, &patient, Symbol::new(&env, "read_profile"));
        Ok(exists)
    }

    /// --------------------
    /// Internal helpers
    /// --------------------

    fn assert_admin(env: &Env, admin: &Address) -> Result<(), Error> {
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if &stored != admin {
            return Err(Error::NotAuthorized);
        }
        Ok(())
    }

    fn assert_responder(env: &Env, responder: &Address) -> Result<(), Error> {
        let active: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Responder(responder.clone()))
            .unwrap_or(false);
        if !active {
            return Err(Error::NotRegisteredResponder);
        }
        Ok(())
    }

    /// Authorize a read of a patient's emergency data. Allowed when the caller
    /// is the patient, holds an unexpired break-glass grant, or is a registered
    /// responder.
    fn assert_read_access(env: &Env, caller: &Address, patient: &Address) -> Result<(), Error> {
        if caller == patient {
            return Ok(());
        }

        // Active break-glass grant for this caller.
        if let Some(expiry) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::EmergencyAccessGrant(caller.clone()))
        {
            if env.ledger().timestamp() < expiry {
                return Ok(());
            }
        }

        // Registered responder.
        let is_responder: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Responder(caller.clone()))
            .unwrap_or(false);
        if is_responder {
            return Ok(());
        }

        Err(Error::NotAuthorized)
    }

    /// Append an access-log entry recording who read a patient's data and why.
    fn log_access(env: &Env, caller: &Address, patient: &Address, emergency_type: Symbol) {
        let key = DataKey::EmergencyAccessLog(patient.clone());
        let mut logs: Vec<EmergencyAccessLog> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        logs.push_back(EmergencyAccessLog {
            provider_id: caller.clone(),
            emergency_type,
            justification_hash: BytesN::from_array(env, &[0u8; 32]),
            location_hash: BytesN::from_array(env, &[0u8; 32]),
            access_time: env.ledger().timestamp(),
        });
        env.storage().persistent().set(&key, &logs);
        ttl_config::extend_critical_ttl(env, &key);
    }
}
