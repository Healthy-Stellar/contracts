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
    EmergencyAccessLog(Address),
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

        // Store advance directives if provided
        if let Some(hash) = advance_directives_hash {
            env.storage()
                .persistent()
                .set(&DataKey::EmergencyNotifications(patient_id.clone()), &hash);
        }

        Ok(())
    }

    /// Break-glass emergency access request. A registered responder files a
    /// time-bound grant scoped to a single patient. The grant is keyed by
    /// `(responder, patient)` so it cannot be reused to read other patients.
    pub fn emergency_access_request(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        emergency_type: Symbol,
        justification_hash: BytesN<32>,
        location_hash: BytesN<32>,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        if !env
            .storage()
            .persistent()
            .has(&DataKey::Responder(provider_id.clone()))
        {
            return Err(Error::NotRegisteredResponder);
        }

        let now = env.ledger().timestamp();
        let expires_at = now + EMERGENCY_ACCESS_WINDOW_SECONDS;

        env.storage().persistent().set(
            &DataKey::EmergencyAccessGrant(provider_id.clone(), patient_id.clone()),
            &expires_at,
        );

        let log = EmergencyAccessLog {
            provider_id: provider_id.clone(),
            emergency_type,
            justification_hash,
            location_hash,
            access_time: now,
        };
        env.storage()
            .persistent()
            .set(&DataKey::EmergencyAccessLog(patient_id), &log);

        Ok(())
    }

    /// Read a patient's emergency profile under a valid break-glass grant.
    /// The grant must be scoped to this exact `(provider_id, patient_id)` pair
    /// and must not have expired.
    pub fn emergency_read(
        env: Env,
        provider_id: Address,
        patient_id: Address,
    ) -> Result<EmergencyProfile, Error> {
        provider_id.require_auth();

        let grant_key = DataKey::EmergencyAccessGrant(provider_id.clone(), patient_id.clone());
        let expires_at: u64 = env
            .storage()
            .persistent()
            .get(&grant_key)
            .ok_or(Error::NotAuthorized)?;

        if env.ledger().timestamp() > expires_at {
            return Err(Error::EmergencyAccessExpired);
        }

        env.storage()
            .persistent()
            .get(&DataKey::EmergencyProfile(patient_id))
            .ok_or(Error::EmergencyProfileNotFound)
    }
}
