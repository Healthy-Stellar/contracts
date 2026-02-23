use soroban_sdk::{Address, Env, String, Vec};
use crate::types::{
    AccessGrant, CdRecord, DataKey, ImagingReport, ImagingStudy, QcReview, SeriesInfo, ViewRecord,
};

const BUMP_AMOUNT: u32 = 518400; // ~60 days in ledgers (assuming 5s ledger)
const BUMP_THRESHOLD: u32 = 259200; // ~30 days

pub fn next_study_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::StudyCounter)
        .unwrap_or(0_u64)
        + 1;
    env.storage().instance().set(&DataKey::StudyCounter, &id);
    id
}

pub fn next_cd_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::CdCounter)
        .unwrap_or(0_u64)
        + 1;
    env.storage().instance().set(&DataKey::CdCounter, &id);
    id
}

pub fn save_study(env: &Env, study: &ImagingStudy) {
    let key = DataKey::Study(study.study_id);
    env.storage().persistent().set(&key, study);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn load_study(env: &Env, study_id: u64) -> Option<ImagingStudy> {
    env.storage().persistent().get(&DataKey::Study(study_id))
}

pub fn save_series(env: &Env, study_id: u64, series: &Vec<SeriesInfo>) {
    let key = DataKey::SeriesList(study_id);
    env.storage().persistent().set(&key, series);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
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
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn load_report(env: &Env, study_id: u64) -> Option<ImagingReport> {
    env.storage().persistent().get(&DataKey::Report(study_id))
}

pub fn save_access_list(env: &Env, study_id: u64, grants: &Vec<AccessGrant>) {
    let key = DataKey::AccessList(study_id);
    env.storage().persistent().set(&key, grants);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
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
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
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
    env.storage().persistent().set(&key, &logs);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn save_qc_review(env: &Env, review: &QcReview) {
    let key = DataKey::QcReview(review.study_id);
    env.storage().persistent().set(&key, review);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn save_anonymized_uid(env: &Env, study_id: u64, uid: &String) {
    let key = DataKey::AnonymizedStudy(study_id);
    env.storage().persistent().set(&key, uid);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}

pub fn save_cd_record(env: &Env, record: &CdRecord) {
    let key = DataKey::CdRecord(record.cd_id);
    env.storage().persistent().set(&key, record);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
}
