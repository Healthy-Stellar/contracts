use crate::types::{
    AccessGrant, CdRecord, DataKey, ImagingReport, ImagingStudy, QcReview, SeriesInfo, ViewRecord,
};
use soroban_sdk::{Address, BytesN, Env, String, Vec};
use ttl_config::critical::{LEDGER_BUMP_AMOUNT, LEDGER_THRESHOLD};
use shared_contracts::safe_increment;

/// Maximum number of view-log entries retained per study. Older entries are
/// evicted (FIFO) once the cap is reached so the persistent entry stays bounded.
pub const MAX_VIEW_LOGS: u32 = 100;

pub fn next_study_id(env: &Env) -> u64 {
    safe_increment(env, &DataKey::StudyCounter)
}

pub fn next_cd_id(env: &Env) -> u64 {
    safe_increment(env, &DataKey::CdCounter)
}

pub fn save_study(env: &Env, study: &ImagingStudy) {
    let key = DataKey::Study(study.study_id);
    env.storage().persistent().set(&key, study);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn load_study(env: &Env, study_id: u64) -> Option<ImagingStudy> {
    env.storage().persistent().get(&DataKey::Study(study_id))
}

pub fn save_series(env: &Env, study_id: u64, series: &Vec<SeriesInfo>) {
    let key = DataKey::SeriesList(study_id);
    env.storage().persistent().set(&key, series);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn load_series(env: &Env, study_id: u64) -> Vec<SeriesInfo> {
    env.storage()
        .persistent()
        .get(&DataKey::SeriesList(study_id))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn save_report(env: &Env, report: &ImagingReport) {
    let key = DataKey::Report(report.study_id);
    env.storage().persistent().set(&key, report);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

#[allow(dead_code)]
pub fn load_report(env: &Env, study_id: u64) -> Option<ImagingReport> {
    env.storage().persistent().get(&DataKey::Report(study_id))
}

pub fn save_access_list(env: &Env, study_id: u64, grants: &Vec<AccessGrant>) {
    let key = DataKey::AccessList(study_id);
    env.storage().persistent().set(&key, grants);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn load_access_list(env: &Env, study_id: u64) -> Vec<AccessGrant> {
    env.storage()
        .persistent()
        .get(&DataKey::AccessList(study_id))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn save_patient_studies(env: &Env, patient_id: &Address, studies: &Vec<u64>) {
    let key = DataKey::PatientStudies(patient_id.clone());
    env.storage().persistent().set(&key, studies);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn load_patient_studies(env: &Env, patient_id: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PatientStudies(patient_id.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn append_view_log(env: &Env, study_id: u64, record: &ViewRecord) {
    let key = DataKey::ViewLog(study_id);
    let mut logs: Vec<ViewRecord> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    logs.push_back(record.clone());
    while logs.len() > MAX_VIEW_LOGS {
        logs.remove(0);
    }
    env.storage().persistent().set(&key, &logs);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

/// Persist a full view-log list for a study, enforcing the bounded cap.
pub fn save_view_logs(env: &Env, study_id: u64, logs: &Vec<ViewRecord>) {
    let key = DataKey::ViewLog(study_id);
    let mut bounded = logs.clone();
    while bounded.len() > MAX_VIEW_LOGS {
        bounded.remove(0);
    }
    env.storage().persistent().set(&key, &bounded);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn load_view_logs(env: &Env, study_id: u64) -> Vec<ViewRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::ViewLog(study_id))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn save_viewer_last_view_timestamp(
    env: &Env,
    study_id: u64,
    viewer_id: &Address,
    view_timestamp: u64,
) {
    let key = DataKey::ViewerLastViewTs(study_id, viewer_id.clone());
    env.storage().persistent().set(&key, &view_timestamp);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn load_viewer_last_view_timestamp(env: &Env, study_id: u64, viewer_id: &Address) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::ViewerLastViewTs(study_id, viewer_id.clone()))
}

pub fn save_viewer_view_chain_head(
    env: &Env,
    study_id: u64,
    viewer_id: &Address,
    entry_hash: &BytesN<32>,
) {
    let key = DataKey::ViewerViewChainHead(study_id, viewer_id.clone());
    env.storage().persistent().set(&key, entry_hash);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn load_viewer_view_chain_head(env: &Env, study_id: u64, viewer_id: &Address) -> Option<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::ViewerViewChainHead(study_id, viewer_id.clone()))
}

pub fn save_qc_review(env: &Env, review: &QcReview) {
    let key = DataKey::QcReview(review.study_id);
    env.storage().persistent().set(&key, review);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn save_anonymized_uid(env: &Env, study_id: u64, rotation_epoch: u32, uid: &String) {
    let key = DataKey::AnonymizedStudy(study_id, rotation_epoch);
    env.storage().persistent().set(&key, uid);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn load_anonymized_uid(env: &Env, study_id: u64, rotation_epoch: u32) -> Option<String> {
    env.storage()
        .persistent()
        .get(&DataKey::AnonymizedStudy(study_id, rotation_epoch))
}

pub fn save_cd_record(env: &Env, record: &CdRecord) {
    let key = DataKey::CdRecord(record.cd_id);
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}
