use soroban_sdk::{Address, Env, Vec};

use crate::{
    AdverseEventReport, ClinicalTrial, DataKey, EligibilityCriteria, Error,
    ParticipantEnrollment, ProtocolDeviation, SafetyReport, StudyVisit,
};

/// Get the next trial record ID and increment counter
pub fn get_next_trial_id(env: &Env) -> u64 {
    let current_id = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::TrialCounter)
        .unwrap_or(0);

    env.storage()
        .instance()
        .set(&DataKey::TrialCounter, &(current_id + 1));

    current_id
}

/// Get the next enrollment ID and increment counter
pub fn get_next_enrollment_id(env: &Env) -> u64 {
    let current_id = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::EnrollmentCounter)
        .unwrap_or(0);

    env.storage()
        .instance()
        .set(&DataKey::EnrollmentCounter, &(current_id + 1));

    current_id
}

/// Get the next adverse event ID and increment counter
pub fn get_next_event_id(env: &Env) -> u64 {
    let current_id = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::EventCounter)
        .unwrap_or(0);

    env.storage()
        .instance()
        .set(&DataKey::EventCounter, &(current_id + 1));

    current_id
}

/// Save a clinical trial record
pub fn save_trial(env: &Env, trial: &ClinicalTrial) {
    let key = DataKey::Trial(trial.trial_record_id);
    env.storage().persistent().set(&key, trial);
}

/// Get a clinical trial record by ID
pub fn get_trial(env: &Env, trial_record_id: u64) -> Result<ClinicalTrial, Error> {
    let key = DataKey::Trial(trial_record_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(Error::TrialNotFound)
}

/// Save eligibility criteria for a trial
pub fn save_criteria(env: &Env, criteria: &EligibilityCriteria) {
    let key = DataKey::Criteria(criteria.trial_record_id);
    env.storage().persistent().set(&key, criteria);
}

/// Get eligibility criteria for a trial
pub fn get_criteria(env: &Env, trial_record_id: u64) -> Result<EligibilityCriteria, Error> {
    let key = DataKey::Criteria(trial_record_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(Error::CriteriaNotFound)
}

/// Save a participant enrollment record
pub fn save_enrollment(env: &Env, enrollment: &ParticipantEnrollment) {
    let key = DataKey::Enrollment(enrollment.enrollment_id);
    env.storage().persistent().set(&key, enrollment);
}

/// Get a participant enrollment record by ID
pub fn get_enrollment(env: &Env, enrollment_id: u64) -> Result<ParticipantEnrollment, Error> {
    let key = DataKey::Enrollment(enrollment_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(Error::EnrollmentNotFound)
}

/// Add enrollment ID to trial's enrollment list
pub fn add_trial_enrollment(env: &Env, trial_record_id: u64, enrollment_id: u64) {
    let key = DataKey::TrialEnrollments(trial_record_id);
    let mut enrollments: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));

    enrollments.push_back(enrollment_id);
    env.storage().persistent().set(&key, &enrollments);
}

/// Get all enrollment IDs for a trial
pub fn get_trial_enrollments(env: &Env, trial_record_id: u64) -> Vec<u64> {
    let key = DataKey::TrialEnrollments(trial_record_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env))
}

/// Add enrollment ID to patient's enrollment list
pub fn add_patient_enrollment(env: &Env, patient_id: &Address, enrollment_id: u64) {
    let key = DataKey::PatientEnrollments(patient_id.clone());
    let mut enrollments: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));

    enrollments.push_back(enrollment_id);
    env.storage().persistent().set(&key, &enrollments);
}

/// Save a study visit record
pub fn save_study_visit(env: &Env, visit: &StudyVisit) {
    let key = DataKey::StudyVisit(visit.enrollment_id, visit.visit_number);
    env.storage().persistent().set(&key, visit);
}

/// Get a study visit record
pub fn get_study_visit(
    env: &Env,
    enrollment_id: u64,
    visit_number: u32,
) -> Result<StudyVisit, Error> {
    let key = DataKey::StudyVisit(enrollment_id, visit_number);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(Error::VisitNotFound)
}

/// Save an adverse event report
pub fn save_adverse_event(env: &Env, event: &AdverseEventReport) {
    let key = DataKey::AdverseEvent(event.event_id);
    env.storage().persistent().set(&key, event);
}

/// Get an adverse event report
pub fn get_adverse_event(env: &Env, event_id: u64) -> Result<AdverseEventReport, Error> {
    let key = DataKey::AdverseEvent(event_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(Error::EventNotFound)
}

/// Save a protocol deviation record
pub fn save_protocol_deviation(env: &Env, enrollment_id: u64, deviation: &ProtocolDeviation) {
    let deviation_count = get_deviation_count(env, enrollment_id);
    let key = DataKey::ProtocolDeviation(enrollment_id, deviation_count);
    env.storage().persistent().set(&key, deviation);
    increment_deviation_count(env, enrollment_id);
}

/// Get deviation count for an enrollment
fn get_deviation_count(env: &Env, enrollment_id: u64) -> u64 {
    let key = DataKey::ProtocolDeviation(enrollment_id, u64::MAX);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(0u64)
}

/// Increment deviation count for an enrollment
fn increment_deviation_count(env: &Env, enrollment_id: u64) {
    let count = get_deviation_count(env, enrollment_id);
    let key = DataKey::ProtocolDeviation(enrollment_id, u64::MAX);
    env.storage().persistent().set(&key, &(count + 1));
}

/// Save a safety report
pub fn save_safety_report(env: &Env, trial_record_id: u64, report: &SafetyReport) {
    let report_count = get_safety_report_count(env, trial_record_id);
    let key = DataKey::SafetyReport(trial_record_id, report_count);
    env.storage().persistent().set(&key, report);
    increment_safety_report_count(env, trial_record_id);
}

/// Get safety report count for a trial
fn get_safety_report_count(env: &Env, trial_record_id: u64) -> u64 {
    let key = DataKey::SafetyReport(trial_record_id, u64::MAX);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(0u64)
}

/// Increment safety report count for a trial
fn increment_safety_report_count(env: &Env, trial_record_id: u64) {
    let count = get_safety_report_count(env, trial_record_id);
    let key = DataKey::SafetyReport(trial_record_id, u64::MAX);
    env.storage().persistent().set(&key, &(count + 1));
}

/// Check if patient is already enrolled in trial
pub fn check_duplicate_enrollment(
    env: &Env,
    trial_record_id: u64,
    patient_id: &Address,
) -> bool {
    let enrollments = get_trial_enrollments(env, trial_record_id);

    for enrollment_id in enrollments.iter() {
        if let Ok(enrollment) = get_enrollment(env, enrollment_id) {
            if enrollment.patient_id == *patient_id
                && enrollment.status == crate::EnrollmentStatus::Active
            {
                return true;
            }
        }
    }

    false
}
