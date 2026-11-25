use soroban_sdk::Env;

use crate::storage::{
    get_discharge_plan, get_latest_readiness_assessment, get_readiness_assessment,
};
use crate::types::{Error, ReadinessLevel};

/// Maximum allowed value for a 0-100 bounded score.
pub const MAX_SCORE: u32 = 100;

/// Maximum number of follow-up appointments accepted in a single batch.
pub const MAX_APPOINTMENTS_PER_BATCH: u32 = 20;

/// Validate that expected discharge date is after admission date
pub fn validate_dates(
    _env: &Env,
    admission_date: u64,
    expected_discharge_date: u64,
) -> Result<(), Error> {
    if expected_discharge_date <= admission_date {
        return Err(Error::InvalidDates);
    }
    Ok(())
}

/// Validate that a discharge plan exists
pub fn validate_plan_exists(env: &Env, plan_id: u64) -> Result<(), Error> {
    get_discharge_plan(env, plan_id)?;
    Ok(())
}

/// Validate that a single score is within the 0-100 range.
pub fn validate_score_range(score: u32) -> Result<(), Error> {
    if score > MAX_SCORE {
        return Err(Error::InvalidScore);
    }
    Ok(())
}

/// Validate that all readiness domain scores are within range and sum them
/// using checked arithmetic to avoid overflow.
pub fn validate_and_sum_scores(
    medical_stability_score: u32,
    functional_status_score: u32,
    support_system_score: u32,
) -> Result<u32, Error> {
    validate_score_range(medical_stability_score)?;
    validate_score_range(functional_status_score)?;
    validate_score_range(support_system_score)?;

    let total = medical_stability_score
        .checked_add(functional_status_score)
        .ok_or(Error::ScoreOverflow)?;
    let total = total
        .checked_add(support_system_score)
        .ok_or(Error::ScoreOverflow)?;
    Ok(total)
}

/// Validate that the latest readiness assessment for a plan is `Ready`.
/// If it is not `Ready`, an explicit override must be recorded (a non-zero
/// override reason hash and an approver).
pub fn validate_readiness_gate(
    env: &Env,
    plan_id: u64,
    override_reason_hash: Option<u64>,
    approver: Option<soroban_sdk::Address>,
) -> Result<(), Error> {
    let assessment = get_latest_readiness_assessment(env, plan_id)?;

    if assessment.readiness_level == ReadinessLevel::Ready {
        return Ok(());
    }

    // Not ready: require a recorded override.
    let has_reason = matches!(override_reason_hash, Some(hash) if hash != 0);
    let has_approver = approver.is_some();

    if has_reason && has_approver {
        Ok(())
    } else {
        Err(Error::NotReadyForDischarge)
    }
}

/// Validate that a specific readiness assessment is `Ready`, or that an
/// explicit override is recorded.
pub fn validate_assessment_ready(
    env: &Env,
    plan_id: u64,
    assessment_id: u64,
    override_reason_hash: Option<u64>,
    approver: Option<soroban_sdk::Address>,
) -> Result<(), Error> {
    let assessment = get_readiness_assessment(env, plan_id, assessment_id)?;

    if assessment.readiness_level == ReadinessLevel::Ready {
        return Ok(());
    }

    let has_reason = matches!(override_reason_hash, Some(hash) if hash != 0);
    let has_approver = approver.is_some();

    if has_reason && has_approver {
        Ok(())
    } else {
        Err(Error::NotReadyForDischarge)
    }
}

/// Validate that `admission_date <= actual_discharge_date <= now`.
pub fn validate_actual_discharge_date(
    env: &Env,
    admission_date: u64,
    actual_discharge_date: u64,
) -> Result<(), Error> {
    let now = env.ledger().timestamp();

    if actual_discharge_date < admission_date {
        return Err(Error::InvalidDates);
    }
    if actual_discharge_date > now {
        return Err(Error::InvalidDates);
    }
    Ok(())
}

/// Validate that a date is not in the past relative to the ledger time.
pub fn validate_not_in_past(env: &Env, date: u64) -> Result<(), Error> {
    let now = env.ledger().timestamp();
    if date < now {
        return Err(Error::InvalidDates);
    }
    Ok(())
}

/// Validate that a start/delivery/transfer date is not in the past.
pub fn validate_future_date(env: &Env, date: u64) -> Result<(), Error> {
    validate_not_in_past(env, date)
}

/// Validate a readmission risk score (0-100).
pub fn validate_risk_score(risk_score: u32) -> Result<(), Error> {
    if risk_score > MAX_SCORE {
        return Err(Error::InvalidScore);
    }
    Ok(())
}

/// Validate a patient understanding level (0-100).
pub fn validate_patient_understanding_level(level: u32) -> Result<(), Error> {
    if level > MAX_SCORE {
        return Err(Error::InvalidScore);
    }
    Ok(())
}

/// Validate the size of a follow-up appointment batch.
pub fn validate_appointments_batch(count: u32) -> Result<(), Error> {
    if count > MAX_APPOINTMENTS_PER_BATCH {
        return Err(Error::TooManyAppointments);
    }
    Ok(())
}