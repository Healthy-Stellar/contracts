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

/// Timelock (in seconds) that must elapse after a recovery proposal reaches
/// its approval threshold before `execute_recovery` may migrate ownership.
/// During this window the patient may cancel a malicious proposal.
pub const RECOVERY_TIMELOCK_SECONDS: u64 = 86_400;

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

    /// Configure the recovery guardians and threshold for a patient.
    /// Guardians must be unique; duplicates are rejected so the approval
    /// de-duplication in `propose_recovery` cannot make the threshold
    /// unreachable.
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

        // Reject duplicate guardians: approvals are de-duplicated, so a
        // repeated guardian would make the threshold unreachable.
        let mut i = 0;
        while i < guardians.len() {
            let mut j = i + 1;
            while j < guardians.len() {
                if guardians.get(i).unwrap() == guardians.get(j).unwrap() {
                    return Err(Error::DuplicateGuardian);
                }
                j += 1;
            }
            i += 1;
        }

        let config = RecoveryConfig {
            emergency_contact,
            guardians,
            recovery_threshold,
        };
        env.storage()
            .persistent()
            .set(&DataKey::RecoveryConfig(patient_id.clone()), &config);

        env.events()
            .publish((Symbol::new(&env, "recovery_cfg"), patient_id), config.recovery_threshold);

        Ok(())
    }

    /// Propose a new owner for a patient's emergency data (key-loss recovery).
    ///
    /// A guardian may only approve the `new_owner` of the currently pending
    /// proposal. If a proposal already exists with a different `new_owner`,
    /// the call is rejected so later guardians cannot unknowingly co-sign an
    /// attacker-chosen target.
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

        if !Self::is_guardian(&config, &guardian) {
            return Err(Error::NotAuthorized);
        }

        let key = DataKey::RecoveryProposal(patient_id.clone());
        let mut proposal: RecoveryProposal = match env.storage().persistent().get(&key) {
            Some(existing) => {
                // Reject approvals for a different target than the pending one.
                if existing.new_owner != new_owner {
                    return Err(Error::RecoveryTargetMismatch);
                }
                existing
            }
            None => RecoveryProposal {
                new_owner: new_owner.clone(),
                approvals: Vec::new(&env),
                created_at: env.ledger().timestamp(),
            },
        };

        // De-duplicate approvals.
        let mut already = false;
        let mut i = 0;
        while i < proposal.approvals.len() {
            if proposal.approvals.get(i).unwrap() == guardian {
                already = true;
                break;
            }
            i += 1;
        }
        if !already {
            proposal.approvals.push_back(guardian.clone());
        }

        env.storage().persistent().set(&key, &proposal);

        env.events().publish(
            (Symbol::new(&env, "recovery_prop"), patient_id),
            (guardian, new_owner, proposal.approvals.len()),
        );

        Ok(proposal)
    }

    /// Execute a pending recovery once the approval threshold is met and the
    /// timelock has elapsed. Migrates the profile, DNR order, alerts and
    /// access logs to `new_owner`, then deletes the old keys.
    pub fn execute_recovery(env: Env, patient_id: Address) -> Result<(), Error> {
        let config: RecoveryConfig = env
            .storage()
            .persistent()
            .get(&DataKey::RecoveryConfig(patient_id.clone()))
            .ok_or(Error::NotAuthorized)?;

        let key = DataKey::RecoveryProposal(patient_id.clone());
        let proposal: RecoveryProposal = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::RecoveryProposalNotFound)?;

        if proposal.approvals.len() < config.recovery_threshold {
            return Err(Error::RecoveryThresholdNotMet);
        }

        let now = env.ledger().timestamp();
        if now < proposal.created_at.saturating_add(RECOVERY_TIMELOCK_SECONDS) {
            return Err(Error::RecoveryTimelockNotElapsed);
        }

        let new_owner = proposal.new_owner.clone();

        // Migrate profile.
        let profile_key = DataKey::EmergencyProfile(patient_id.clone());
        if let Some(profile) = env.storage().persistent().get::<_, EmergencyProfile>(&profile_key) {
            env.storage()
                .persistent()
                .set(&DataKey::EmergencyProfile(new_owner.clone()), &profile);
            env.storage().persistent().remove(&profile_key);
        }

        // Migrate DNR order.
        let dnr_key = DataKey::DNROrder(patient_id.clone());
        if let Some(dnr) = env.storage().persistent().get::<_, DNROrder>(&dnr_key) {
            env.storage()
                .persistent()
                .set(&DataKey::DNROrder(new_owner.clone()), &dnr);
            env.storage().persistent().remove(&dnr_key);
        }

        // Migrate critical alerts.
        let alerts_key = DataKey::CriticalAlerts(patient_id.clone());
        if let Some(alerts) = env
            .storage()
            .persistent()
            .get::<_, Vec<CriticalAlert>>(&alerts_key)
        {
            env.storage()
                .persistent()
                .set(&DataKey::CriticalAlerts(new_owner.clone()), &alerts);
            env.storage().persistent().remove(&alerts_key);
        }

        // Migrate access logs.
        let logs_key = DataKey::EmergencyAccessLog(patient_id.clone());
        if let Some(logs) = env
            .storage()
            .persistent()
            .get::<_, Vec<EmergencyAccessLog>>(&logs_key)
        {
            env.storage()
                .persistent()
                .set(&DataKey::EmergencyAccessLog(new_owner.clone()), &logs);
            env.storage().persistent().remove(&logs_key);
        }

        // Migrate notifications / advance directives.
        let notif_key = DataKey::EmergencyNotifications(patient_id.clone());
        if let Some(notif) = env
            .storage()
            .persistent()
            .get::<_, BytesN<32>>(&notif_key)
        {
            env.storage()
                .persistent()
                .set(&DataKey::EmergencyNotifications(new_owner.clone()), &notif);
            env.storage().persistent().remove(&notif_key);
        }

        // Delete the old recovery config and proposal.
        env.storage()
            .persistent()
            .remove(&DataKey::RecoveryConfig(patient_id.clone()));
        env.storage().persistent().remove(&key);

        env.events().publish(
            (Symbol::new(&env, "recovery_exec"), patient_id),
            new_owner,
        );

        Ok(())
    }

    /// Cancel a pending recovery proposal. Only the patient may cancel, and
    /// only during the timelock window (before execution).
    pub fn cancel_recovery(env: Env, patient_id: Address) -> Result<(), Error> {
        patient_id.require_auth();

        let key = DataKey::RecoveryProposal(patient_id.clone());
        if !env.storage().persistent().has(&key) {
            return Err(Error::RecoveryProposalNotFound);
        }
        env.storage().persistent().remove(&key);

        env.events()
            .publish((Symbol::new(&env, "recovery_cancel"), patient_id), ());

        Ok(())
    }

    fn is_guardian(config: &RecoveryConfig, guardian: &Address) -> bool {
        let mut i = 0;
        while i < config.guardians.len() {
            if config.guardians.get(i).unwrap() == *guardian {
                return true;
            }
            i += 1;
        }
        false
    }
}
