use crate::types::{
    AccessGrant, CdRecord, DataKey, ImagingReport, ImagingStudy, QcReview, SeriesInfo, ViewLogHead,
    ViewRecord,
};
use soroban_sdk::{Address, Env, String, Vec};
use ttl_config::critical::{LEDGER_BUMP_AMOUNT, LEDGER_THRESHOLD};
use shared_contracts::safe_increment;

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

/// Append `study_id` to the patient's paged study index.
pub fn push_patient_study(env: &Env, patient_id: &Address, study_id: u64) {
    let count = patient_study_count(env, patient_id);

    let key = DataKey::PatientStudy(patient_id.clone(), count);
    env.storage().persistent().set(&key, &study_id);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);

    let count_key = DataKey::PatientStudyCount(patient_id.clone());
    env.storage().persistent().set(&count_key, &(count + 1));
    env.storage()
        .persistent()
        .extend_ttl(&count_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

pub fn patient_study_count(env: &Env, patient_id: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::PatientStudyCount(patient_id.clone()))
        .unwrap_or(0)
}

/// Load up to `limit` study ids for a patient starting at index `start`.
pub fn load_patient_studies_page(
    env: &Env,
    patient_id: &Address,
    start: u64,
    limit: u32,
) -> Vec<u64> {
    let count = patient_study_count(env, patient_id);
    let end = count.min(start.saturating_add(limit as u64));
    let mut ids = Vec::new(env);
    let mut i = start;
    while i < end {
        if let Some(id) = env
            .storage()
            .persistent()
            .get::<_, u64>(&DataKey::PatientStudy(patient_id.clone(), i))
        {
            ids.push_back(id);
        }
        i += 1;
    }
    ids
}

pub fn load_view_log_head(env: &Env, study_id: u64) -> ViewLogHead {
    env.storage()
        .persistent()
        .get(&DataKey::ViewLogHead(study_id))
        .unwrap_or(ViewLogHead {
            count: 0,
            last_hash: None,
        })
}

/// Append one view record at `ViewLog(study_id, head.count)` and advance the
/// head pointer. Only the new entry and the head are written, so the cost of
/// an append does not grow with the size of the log.
pub fn append_view_log(env: &Env, study_id: u64, record: &ViewRecord) {
    let head = load_view_log_head(env, study_id);

    let key = DataKey::ViewLog(study_id, head.count);
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);

    let head_key = DataKey::ViewLogHead(study_id);
    let new_head = ViewLogHead {
        count: head.count + 1,
        last_hash: Some(record.entry_hash.clone()),
    };
    env.storage().persistent().set(&head_key, &new_head);
    env.storage()
        .persistent()
        .extend_ttl(&head_key, LEDGER_THRESHOLD, LEDGER_BUMP_AMOUNT);
}

/// Load up to `limit` view records for a study starting at seq `start`.
pub fn load_view_logs_page(env: &Env, study_id: u64, start: u64, limit: u32) -> Vec<ViewRecord> {
    let head = load_view_log_head(env, study_id);
    let end = head.count.min(start.saturating_add(limit as u64));
    let mut logs = Vec::new(env);
    let mut i = start;
    while i < end {
        if let Some(record) = env
            .storage()
            .persistent()
            .get::<_, ViewRecord>(&DataKey::ViewLog(study_id, i))
        {
            logs.push_back(record);
        }
        i += 1;
    }
    logs
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
