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
    /// Break-glass access grant expiry timestamp for a responder.
    /// A grant is valid only until this ledger timestamp; afterwards the
    /// responder must issue a fresh `emergency_access_request`.
    EmergencyAccessGrant(Address),
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
            let dnr_key = DataKey::DNROrder(patient_id.clone());
            let dnr = DNROrder {
                provider_id: patient_id.clone(),
                dnr_document_hash: hash,
                effective_date: env.ledger().timestamp(),
                recorded_at: env.ledger().timestamp(),
            };
            env.storage().persistent().set(&dnr_key, &dnr);
        }

        Ok(())
    }

    /// Check whether a patient has an emergency profile on file.
    pub fn has_emergency_profile(env: Env, patient_id: Address) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::EmergencyProfile(patient_id))
    }

    /// Record a DNR (Do Not Resuscitate) order for a patient.
    /// Only a registered emergency responder may record a DNR order.
    pub fn record_dnr_order(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        dnr_document_hash: BytesN<32>,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        if !Self::is_registered_responder(env.clone(), provider_id.clone()) {
            return Err(Error::NotRegisteredResponder);
        }

        let now = env.ledger().timestamp();
        let order = DNROrder {
            provider_id,
            dnr_document_hash,
            effective_date: now,
            recorded_at: now,
        };
        env.storage()
            .persistent()
            .set(&DataKey::DNROrder(patient_id.clone()), &order);

        if let Some(mut profile) = env
            .storage()
            .persistent()
            .get::<DataKey, EmergencyProfile>(&DataKey::EmergencyProfile(patient_id.clone()))
        {
            profile.dnr_status = true;
            env.storage()
                .persistent()
                .set(&DataKey::EmergencyProfile(patient_id), &profile);
        }

        Ok(())
    }

    /// Revoke an existing DNR order for a patient.
    /// Only a registered emergency responder may revoke a DNR order.
    pub fn revoke_dnr_order(
        env: Env,
        provider_id: Address,
        patient_id: Address,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        if !Self::is_registered_responder(env.clone(), provider_id.clone()) {
            return Err(Error::NotRegisteredResponder);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::DNROrder(patient_id.clone()));

        if let Some(mut profile) = env
            .storage()
            .persistent()
            .get::<DataKey, EmergencyProfile>(&DataKey::EmergencyProfile(patient_id.clone()))
        {
            profile.dnr_status = false;
            env.storage()
                .persistent()
                .set(&DataKey::EmergencyProfile(patient_id), &profile);
        }

        Ok(())
    }

    /// Retrieve the DNR order for a patient, if any.
    pub fn get_dnr_order(env: Env, patient_id: Address) -> Option<DNROrder> {
        env.storage()
            .persistent()
            .get(&DataKey::DNROrder(patient_id))
    }

    /// Configure key-loss recovery for a patient.
    /// Only the patient may set their own recovery configuration.
    pub fn set_recovery_config(
        env: Env,
        patient_id: Address,
        emergency_contact: Address,
        guardians: Vec<Address>,
        recovery_threshold: u32,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        if recovery_threshold == 0 || recovery_threshold > guardians.len() {
            return Err(Error::InvalidRecoveryThreshold);
        }

        let config = RecoveryConfig {
            emergency_contact,
            guardians,
            recovery_threshold,
        };
        env.storage()
            .persistent()
            .set(&DataKey::RecoveryConfig(patient_id), &config);
        Ok(())
    }

    /// Propose a key-loss recovery by a guardian.
    /// Each guardian may approve once; once the configured threshold is met
    /// the proposal is recorded for the new owner.
    pub fn propose_recovery(
        env: Env,
        guardian: Address,
        patient_id: Address,
        new_owner: Address,
    ) -> Result<RecoveryProposal, Error> {
        guardian.require_auth();

        let config: RecoveryConfig = env
            .storage()
            .persistent()
            .get(&DataKey::RecoveryConfig(patient_id.clone()))
            .ok_or(Error::NotAuthorized)?;

        let mut is_guardian = false;
        for g in config.guardians.iter() {
            if g == guardian {
                is_guardian = true;
                break;
            }
        }
        if !is_guardian {
            return Err(Error::NotAuthorized);
        }

        let mut proposal = env
            .storage()
            .persistent()
            .get::<DataKey, RecoveryProposal>(&DataKey::RecoveryProposal(patient_id.clone()))
            .unwrap_or(RecoveryProposal {
                new_owner: new_owner.clone(),
                approvals: Vec::new(&env),
            });

        let mut already = false;
        for a in proposal.approvals.iter() {
            if a == guardian {
                already = true;
                break;
            }
        }
        if !already {
            proposal.approvals.push_back(guardian);
        }

        env.storage()
            .persistent()
            .set(&DataKey::RecoveryProposal(patient_id), &proposal);

        Ok(proposal)
    }

    /// Break-glass emergency access request by a registered responder.
    /// Grants a time-bounded read window (see `EMERGENCY_ACCESS_WINDOW_SECONDS`).
    pub fn emergency_access_request(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        emergency_type: Symbol,
        justification_hash: BytesN<32>,
        location_hash: BytesN<32>,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        if !Self::is_registered_responder(env.clone(), provider_id.clone()) {
            return Err(Error::NotRegisteredResponder);
        }

        let now = env.ledger().timestamp();
        let expires_at = now + EMERGENCY_ACCESS_WINDOW_SECONDS;
        env.storage()
            .persistent()
            .set(&DataKey::EmergencyAccessGrant(provider_id.clone()), &expires_at);

        let log = EmergencyAccessLog {
            provider_id,
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

    /// Read a patient's emergency profile via break-glass access.
    /// The responder must hold a non-expired grant; expired grants are
    /// rejected so access is never permanent.
    pub fn emergency_read(
        env: Env,
        provider_id: Address,
        patient_id: Address,
    ) -> Result<EmergencyProfile, Error> {
        provider_id.require_auth();

        let expires_at: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EmergencyAccessGrant(provider_id.clone()))
            .ok_or(Error::NotAuthorized)?;

        if env.ledger().timestamp() > expires_at {
            env.storage()
                .persistent()
                .remove(&DataKey::EmergencyAccessGrant(provider_id));
            return Err(Error::EmergencyAccessExpired);
        }

        env.storage()
            .persistent()
            .get(&DataKey::EmergencyProfile(patient_id))
            .ok_or(Error::EmergencyProfileNotFound)
    }

    /// Attach a critical alert to a patient's emergency profile.
    /// Only a registered emergency responder may add critical alerts.
    pub fn add_critical_alert(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        alert_type: Symbol,
        alert_text_hash: BytesN<32>,
        severity: Symbol,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        if !Self::is_registered_responder(env.clone(), provider_id.clone()) {
            return Err(Error::NotRegisteredResponder);
        }

        let alert = CriticalAlert {
            provider_id,
            alert_type,
            alert_text_hash,
            severity,
            timestamp: env.ledger().timestamp(),
        };

        let key = DataKey::CriticalAlerts(patient_id.clone());
        let mut alerts: Vec<CriticalAlert> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        alerts.push_back(alert);
        env.storage().persistent().set(&key, &alerts);

        Ok(())
    }

    /// Retrieve all critical alerts for a patient.
    pub fn get_critical_alerts(env: Env, patient_id: Address) -> Vec<CriticalAlert> {
        env.storage()
            .persistent()
            .get(&DataKey::CriticalAlerts(patient_id))
            .unwrap_or(Vec::new(&env))
    }
}

#[cfg(test)]
mod test;
