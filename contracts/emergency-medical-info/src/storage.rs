use soroban_sdk::{contracttype, Address, BytesN, Env, Vec};

use crate::types::{DataKey, EmergencyAccessLog, EmergencyType};

/// Maximum number of audit log entries retained per patient.
/// Older entries are evicted once this bound is reached.
pub const MAX_AUDIT_LOG_ENTRIES: u32 = 32;

/// Append an audit log entry for a patient, preserving prior entries.
///
/// The log is bounded: once `MAX_AUDIT_LOG_ENTRIES` entries exist, the oldest
/// entry is dropped so the ring buffer never grows without limit.
pub fn append_audit_log(
    env: &Env,
    patient: &Address,
    responder: &Address,
    emergency_type: &EmergencyType,
    justification_hash: &BytesN<32>,
    timestamp: u64,
) {
    let mut log: Vec<EmergencyAccessLog> = env
        .storage()
        .persistent()
        .get(&DataKey::EmergencyAccessLog(patient.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if log.len() >= MAX_AUDIT_LOG_ENTRIES {
        // Evict the oldest entry to keep the log bounded.
        log.remove(0);
    }

    log.push_back(EmergencyAccessLog {
        responder: responder.clone(),
        patient: patient.clone(),
        emergency_type: emergency_type.clone(),
        justification_hash: justification_hash.clone(),
        timestamp,
    });

    env.storage()
        .persistent()
        .set(&DataKey::EmergencyAccessLog(patient.clone()), &log);
}

/// Read the current audit log for a patient (empty if none).
pub fn get_audit_log(env: &Env, patient: &Address) -> Vec<EmergencyAccessLog> {
    env.storage()
        .persistent()
        .get(&DataKey::EmergencyAccessLog(patient.clone()))
        .unwrap_or_else(|| Vec::new(env))
}
