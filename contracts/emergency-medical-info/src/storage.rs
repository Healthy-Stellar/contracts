use soroban_sdk::{Address, Env, Vec};

use crate::types::{
    AccessLogEntry, CriticalAlert, DNROrder, DataKey, EmergencyAccessGrant, EmergencyProfile,
    RecoveryConfig, RecoveryProposal,
};

pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

// ---------------------------------------------------------------------------
// Emergency profile
// ---------------------------------------------------------------------------

pub fn get_emergency_profile(env: &Env, patient: &Address) -> Option<EmergencyProfile> {
    env.storage()
        .persistent()
        .get(&DataKey::EmergencyProfile(patient.clone()))
}

pub fn set_emergency_profile(env: &Env, patient: &Address, profile: &EmergencyProfile) {
    env.storage()
        .persistent()
        .set(&DataKey::EmergencyProfile(patient.clone()), profile);
}

// ---------------------------------------------------------------------------
// DNR orders
// ---------------------------------------------------------------------------

pub fn get_dnr_order(env: &Env, patient: &Address) -> Option<DNROrder> {
    env.storage()
        .persistent()
        .get(&DataKey::DNROrder(patient.clone()))
}

pub fn set_dnr_order(env: &Env, patient: &Address, order: &DNROrder) {
    env.storage()
        .persistent()
        .set(&DataKey::DNROrder(patient.clone()), order);
}

pub fn remove_dnr_order(env: &Env, patient: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::DNROrder(patient.clone()));
}

// ---------------------------------------------------------------------------
// Critical alerts
// ---------------------------------------------------------------------------

pub fn get_critical_alerts(env: &Env, patient: &Address) -> Vec<CriticalAlert> {
    env.storage()
        .persistent()
        .get(&DataKey::CriticalAlerts(patient.clone()))
        .unwrap_or(Vec::new(env))
}

pub fn set_critical_alerts(env: &Env, patient: &Address, alerts: &Vec<CriticalAlert>) {
    env.storage()
        .persistent()
        .set(&DataKey::CriticalAlerts(patient.clone()), alerts);
}

// ---------------------------------------------------------------------------
// Responder registry
// ---------------------------------------------------------------------------

pub fn is_registered_responder(env: &Env, responder: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Responder(responder.clone()))
        .unwrap_or(false)
}

pub fn set_registered_responder(env: &Env, responder: &Address, registered: bool) {
    env.storage()
        .persistent()
        .set(&DataKey::Responder(responder.clone()), &registered);
}

// ---------------------------------------------------------------------------
// Healthcare proxy
// ---------------------------------------------------------------------------

pub fn get_healthcare_proxy(env: &Env, patient: &Address) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::HealthcareProxy(patient.clone()))
}

pub fn set_healthcare_proxy(env: &Env, patient: &Address, proxy: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::HealthcareProxy(patient.clone()), proxy);
}

// ---------------------------------------------------------------------------
// Break-glass access grants (per responder + patient)
// ---------------------------------------------------------------------------

pub fn get_access_grant(
    env: &Env,
    responder: &Address,
    patient: &Address,
) -> Option<EmergencyAccessGrant> {
    env.storage()
        .persistent()
        .get(&DataKey::AccessGrant(responder.clone(), patient.clone()))
}

pub fn set_access_grant(
    env: &Env,
    responder: &Address,
    patient: &Address,
    grant: &EmergencyAccessGrant,
) {
    env.storage()
        .persistent()
        .set(&DataKey::AccessGrant(responder.clone(), patient.clone()), grant);
}

pub fn remove_access_grant(env: &Env, responder: &Address, patient: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::AccessGrant(responder.clone(), patient.clone()));
}

// ---------------------------------------------------------------------------
// Access log (per-patient sequence)
// ---------------------------------------------------------------------------

pub fn get_access_log(env: &Env, patient: &Address) -> Vec<AccessLogEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::AccessLog(patient.clone()))
        .unwrap_or(Vec::new(env))
}

pub fn set_access_log(env: &Env, patient: &Address, log: &Vec<AccessLogEntry>) {
    env.storage()
        .persistent()
        .set(&DataKey::AccessLog(patient.clone()), log);
}

// ---------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------

pub fn get_recovery_config(env: &Env) -> Option<RecoveryConfig> {
    env.storage().instance().get(&DataKey::RecoveryConfig)
}

pub fn set_recovery_config(env: &Env, config: &RecoveryConfig) {
    env.storage().instance().set(&DataKey::RecoveryConfig, config);
}

pub fn get_recovery_proposal(env: &Env, patient: &Address) -> Option<RecoveryProposal> {
    env.storage()
        .persistent()
        .get(&DataKey::RecoveryProposal(patient.clone()))
}

pub fn set_recovery_proposal(env: &Env, patient: &Address, proposal: &RecoveryProposal) {
    env.storage()
        .persistent()
        .set(&DataKey::RecoveryProposal(patient.clone()), proposal);
}

pub fn remove_recovery_proposal(env: &Env, patient: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::RecoveryProposal(patient.clone()));
}

pub fn get_guardians(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::Guardians)
        .unwrap_or(Vec::new(env))
}

pub fn set_guardians(env: &Env, guardians: &Vec<Address>) {
    env.storage().instance().set(&DataKey::Guardians, guardians);
}
