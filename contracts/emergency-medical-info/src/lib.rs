#![no_std]
#![allow(clippy::too_many_arguments)]

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
}

/// Maximum number of hashes allowed in `critical_allergy_hashes`.
/// Inputs exceeding this cap are rejected to prevent unbounded Vec growth.
pub const MAX_CRITICAL_ALLERGIES: u32 = 50;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    EmergencyProfileNotFound = 1,
    NotAuthorized = 2,
    InvalidRecoveryThreshold = 3,
    TooManyCriticalAllergies = 4,
}

#[contract]
pub struct EmergencyMedicalInfo;

#[contractimpl]
impl EmergencyMedicalInfo {
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

    /// Add critical alert to patient profile
    pub fn add_critical_alert(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        alert_type: Symbol,
        alert_text_hash: BytesN<32>,
        severity: Symbol,
    ) {
        provider_id.require_auth();

        let alert = CriticalAlert {
            provider_id,
            alert_type,
            alert_text_hash,
            severity: severity.clone(),
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
    }

    /// Emergency access with break-glass protocol
    /// Provides immediate access with full audit logging
    pub fn emergency_access_request(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        emergency_type: Symbol,
        justification_hash: BytesN<32>,
        location_hash: BytesN<32>,
    ) -> Result<EmergencyProfile, Error> {
        provider_id.require_auth();

        // Log the emergency access (break-glass audit)
        let access_log = EmergencyAccessLog {
            provider_id: provider_id.clone(),
            emergency_type: emergency_type.clone(),
            justification_hash,
            location_hash,
            access_time: env.ledger().timestamp(),
        };

        let log_key = DataKey::EmergencyAccessLog(patient_id.clone());
        let mut logs: Vec<EmergencyAccessLog> = env
            .storage()
            .persistent()
            .get(&log_key)
            .unwrap_or(Vec::new(&env));

        logs.push_back(access_log);
        env.storage().persistent().set(&log_key, &logs);

        // Retrieve emergency profile
        let profile_key = DataKey::EmergencyProfile(patient_id.clone());
        env.storage()
            .persistent()
            .get(&profile_key)
            .ok_or(Error::EmergencyProfileNotFound)
    }

    /// Notify emergency contacts
    pub fn notify_emergency_contacts(
        env: Env,
        patient_id: Address,
        emergency_type: Symbol,
        notification_time: u64,
    ) -> Result<Vec<EmergencyContact>, Error> {
        // Get emergency profile
        let profile_key = DataKey::EmergencyProfile(patient_id.clone());
        let profile: EmergencyProfile = env
            .storage()
            .persistent()
            .get(&profile_key)
            .ok_or(Error::EmergencyProfileNotFound)?;

        // Log notification
        let notif_key = DataKey::EmergencyNotifications(patient_id.clone());
        let mut notifications: Vec<(Symbol, u64)> = env
            .storage()
            .persistent()
            .get(&notif_key)
            .unwrap_or(Vec::new(&env));

        notifications.push_back((emergency_type, notification_time));
        env.storage().persistent().set(&notif_key, &notifications);

        Ok(profile.emergency_contacts)
    }

    /// Record DNR (Do Not Resuscitate) order
    pub fn record_dnr_order(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        dnr_document_hash: BytesN<32>,
        effective_date: u64,
    ) {
        provider_id.require_auth();

        let dnr = DNROrder {
            provider_id: provider_id.clone(),
            dnr_document_hash,
            effective_date,
            recorded_at: env.ledger().timestamp(),
        };

        let dnr_key = DataKey::DNROrder(patient_id.clone());
        env.storage().persistent().set(&dnr_key, &dnr);

        // Update profile DNR status
        let profile_key = DataKey::EmergencyProfile(patient_id.clone());
        if let Some(mut profile) = env
            .storage()
            .persistent()
            .get::<_, EmergencyProfile>(&profile_key)
        {
            profile.dnr_status = true;
            env.storage().persistent().set(&profile_key, &profile);
        }
    }

    /// Get emergency information (fast read access)
    pub fn get_emergency_info(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<EmergencyProfile, Error> {
        requester.require_auth();
        if requester != patient_id && !Self::has_emergency_access_log(&env, &patient_id, &requester)
        {
            return Err(Error::NotAuthorized);
        }

        let key = DataKey::EmergencyProfile(patient_id.clone());
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(Error::EmergencyProfileNotFound)
    }

    /// Get critical alerts for a patient
    pub fn get_critical_alerts(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<Vec<CriticalAlert>, Error> {
        Self::require_emergency_read_access(&env, &patient_id, &requester)?;
        let key = DataKey::CriticalAlerts(patient_id);
        Ok(env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env)))
    }

    /// Get emergency access logs (audit trail)
    pub fn get_emergency_access_logs(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<Vec<EmergencyAccessLog>, Error> {
        Self::require_emergency_read_access(&env, &patient_id, &requester)?;
        let key = DataKey::EmergencyAccessLog(patient_id);
        Ok(env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env)))
    }

    /// Get DNR order details
    pub fn get_dnr_order(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<Option<DNROrder>, Error> {
        Self::require_emergency_read_access(&env, &patient_id, &requester)?;
        let key = DataKey::DNROrder(patient_id);
        Ok(env.storage().persistent().get(&key))
    }

    /// Check if patient has emergency profile
    pub fn has_emergency_profile(env: Env, patient_id: Address) -> bool {
        let key = DataKey::EmergencyProfile(patient_id);
        env.storage().persistent().has(&key)
    }

    /// Configure read-only emergency access and optional guardian recovery.
    pub fn set_recovery_config(
        env: Env,
        patient_id: Address,
        emergency_contact: Address,
        guardians: Vec<Address>,
        recovery_threshold: u32,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        if recovery_threshold as u32 > guardians.len() {
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

    /// Read profile as the configured emergency contact or a listed guardian.
    pub fn emergency_read(
        env: Env,
        patient_id: Address,
        caller: Address,
    ) -> Result<EmergencyProfile, Error> {
        caller.require_auth();
        let config: RecoveryConfig = env
            .storage()
            .persistent()
            .get(&DataKey::RecoveryConfig(patient_id.clone()))
            .ok_or(Error::NotAuthorized)?;

        if caller != config.emergency_contact && !config.guardians.contains(&caller) {
            return Err(Error::NotAuthorized);
        }

        env.storage()
            .persistent()
            .get(&DataKey::EmergencyProfile(patient_id))
            .ok_or(Error::EmergencyProfileNotFound)
    }

    /// Guardian vote for re-keying the emergency profile to a new patient address.
    pub fn propose_recovery(
        env: Env,
        patient_id: Address,
        guardian: Address,
        new_owner: Address,
    ) -> Result<(), Error> {
        guardian.require_auth();
        let config: RecoveryConfig = env
            .storage()
            .persistent()
            .get(&DataKey::RecoveryConfig(patient_id.clone()))
            .ok_or(Error::NotAuthorized)?;

        if !config.guardians.contains(&guardian) || config.recovery_threshold == 0 {
            return Err(Error::NotAuthorized);
        }

        let proposal_key = DataKey::RecoveryProposal(patient_id.clone());
        let mut proposal: RecoveryProposal = env
            .storage()
            .temporary()
            .get(&proposal_key)
            .unwrap_or(RecoveryProposal {
                new_owner: new_owner.clone(),
                approvals: Vec::new(&env),
            });

        if proposal.new_owner != new_owner {
            return Err(Error::NotAuthorized);
        }
        if !proposal.approvals.contains(&guardian) {
            proposal.approvals.push_back(guardian);
        }

        if proposal.approvals.len() >= config.recovery_threshold {
            let old_profile_key = DataKey::EmergencyProfile(patient_id.clone());
            let profile: EmergencyProfile = env
                .storage()
                .persistent()
                .get(&old_profile_key)
                .ok_or(Error::EmergencyProfileNotFound)?;
            env.storage()
                .persistent()
                .set(&DataKey::EmergencyProfile(new_owner.clone()), &profile);
            env.storage().persistent().remove(&old_profile_key);
            env.storage().temporary().remove(&proposal_key);
            env.events()
                .publish((Symbol::new(&env, "recovered"), patient_id), new_owner);
        } else {
            env.storage().temporary().set(&proposal_key, &proposal);
        }

        Ok(())
    }

    fn require_emergency_read_access(
        env: &Env,
        patient_id: &Address,
        requester: &Address,
    ) -> Result<(), Error> {
        requester.require_auth();
        if requester == patient_id || Self::has_emergency_access_log(env, patient_id, requester) {
            return Ok(());
        }
        Err(Error::NotAuthorized)
    }

    fn has_emergency_access_log(env: &Env, patient_id: &Address, requester: &Address) -> bool {
        let key = DataKey::EmergencyAccessLog(patient_id.clone());
        let logs: Vec<EmergencyAccessLog> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(env));
        for log in logs.iter() {
            if log.provider_id == *requester {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod test;
