use soroban_sdk::{symbol_short, Env, Symbol};

use crate::Error;

/// Validate study phase
pub fn validate_study_phase(phase: &Symbol) -> Result<(), Error> {
    let valid_phases = [
        symbol_short!("phase1"),
        symbol_short!("phase2"),
        symbol_short!("phase3"),
        symbol_short!("phase4"),
        symbol_short!("pilot"),
    ];

    if valid_phases.contains(phase) {
        Ok(())
    } else {
        Err(Error::InvalidStudyPhase)
    }
}

/// Validate severity level
pub fn validate_severity(severity: &Symbol) -> Result<(), Error> {
    let valid_severities = [
        symbol_short!("mild"),
        symbol_short!("moderate"),
        symbol_short!("severe"),
        symbol_short!("critical"),
    ];

    if valid_severities.contains(severity) {
        Ok(())
    } else {
        Err(Error::InvalidSeverity)
    }
}

/// Validate causality assessment
pub fn validate_causality(causality: &Symbol) -> Result<(), Error> {
    let valid_causalities = [
        symbol_short!("unrelated"),
        symbol_short!("unlikely"),
        symbol_short!("possible"),
        symbol_short!("probable"),
        symbol_short!("definite"),
    ];

    if valid_causalities.contains(causality) {
        Ok(())
    } else {
        Err(Error::InvalidCausality)
    }
}

/// Validate date is not in the future
pub fn validate_date_not_future(env: &Env, date: u64) -> Result<(), Error> {
    if date > env.ledger().timestamp() {
        Err(Error::InvalidDate)
    } else {
        Ok(())
    }
}

/// Validate date range
pub fn validate_date_range(start_date: u64, end_date: u64) -> Result<(), Error> {
    if start_date >= end_date {
        Err(Error::InvalidDateRange)
    } else {
        Ok(())
    }
}

/// Validate withdrawal reason
pub fn validate_withdrawal_reason(reason: &Symbol) -> Result<(), Error> {
    let valid_reasons = [
        symbol_short!("adverse"),
        symbol_short!("consent"),
        symbol_short!("protocol"),
        symbol_short!("lost"),
        symbol_short!("complete"),
        symbol_short!("other"),
    ];

    if valid_reasons.contains(reason) {
        Ok(())
    } else {
        Err(Error::InvalidWithdrawalReason)
    }
}
