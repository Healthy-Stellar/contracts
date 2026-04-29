#![no_std]
#![allow(deprecated)]
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, symbol_short, Address, Bytes, BytesN,
    Env, String, Symbol, Vec,
};

mod storage;
mod types;
mod validation;

pub use storage::*;
pub use types::*;

/// Events for clinical trial operations
#[contractevent]
pub struct TrialRegistered {
    pub trial_record_id: u64,
    pub trial_id: String,
}

#[contractevent]
pub struct ParticipantEnrolled {
    pub enrollment_id: u64,
    pub trial_record_id: u64,
    pub participant_id: String,
}

#[contractevent]
pub struct AdverseEventReported {
    pub event_id: u64,
    pub enrollment_id: u64,
    pub severity: Symbol,
}

#[contractevent]
pub struct ParticipantWithdrawn {
    pub enrollment_id: u64,
    pub withdrawal_date: u64,
}

#[contractevent]
pub struct RetentionPolicyApplied {
    pub enrollment_id: u64,
    pub data_class: DataRetentionClass,
    pub consented: bool,
}

#[contractevent]
pub struct SafetyReportSubmitted {
    pub trial_record_id: u64,
    pub reporting_period: u64,
}

/// Error codes for clinical trial operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    TrialNotFound = 1,
    Unauthorized = 2,
    InvalidStudyPhase = 3,
    InvalidDate = 4,
    InvalidDateRange = 5,
    CriteriaNotFound = 6,
    EnrollmentNotFound = 7,
    NotEligible = 8,
    DuplicateEnrollment = 9,
    EnrollmentFull = 10,
    InvalidSeverity = 11,
    InvalidCausality = 12,
    AlreadyWithdrawn = 13,
    InvalidWithdrawalReason = 14,
    VisitNotFound = 15,
    EventNotFound = 16,
    TrialNotActive = 17,
    AlreadyInitialized = 18,
    WithdrawalRestricted = 19,
    InvalidConsent = 20,
}

#[contract]
pub struct ClinicalTrialContract;

#[contractimpl]
impl ClinicalTrialContract {
    /// Initialize the contract with an admin address
    pub fn initialize(env: Env, admin: Address, patient_registry: Address) -> Result<(), Error> {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PatientRegistry, &patient_registry);
        env.storage().instance().set(&DataKey::TrialCounter, &0u64);
        env.storage()
            .instance()
            .set(&DataKey::EnrollmentCounter, &0u64);
        env.storage().instance().set(&DataKey::EventCounter, &0u64);

        Ok(())
    }

    /// Register a new clinical trial
    pub fn register_clinical_trial(
        env: Env,
        principal_investigator: Address,
        trial_id: String,
        trial_name: String,
        study_phase: Symbol,
        protocol_hash: BytesN<32>,
        start_date: u64,
        estimated_end_date: u64,
        enrollment_target: u32,
        irb_approval_number: String,
    ) -> Result<u64, Error> {
        principal_investigator.require_auth();

        // Validate inputs
        validation::validate_study_phase(&study_phase)?;
        validation::validate_date_range(start_date, estimated_end_date)?;

        // Generate unique trial record ID
        let trial_record_id = storage::get_next_trial_id(&env);

        let trial = types::ClinicalTrial {
            trial_record_id,
            principal_investigator: principal_investigator.clone(),
            trial_id: trial_id.clone(),
            trial_name,
            study_phase,
            protocol_hash,
            start_date,
            estimated_end_date,
            enrollment_target,
            irb_approval_number,
            current_enrollment: 0,
            status: TrialStatus::Active,
        };

        // Store trial record
        storage::save_trial(&env, &trial);

        // Emit event
        TrialRegistered {
            trial_record_id,
            trial_id: trial_id.clone(),
        }
        .publish(&env);

        Ok(trial_record_id)
    }

    /// Define eligibility criteria for a trial
    pub fn define_eligibility_criteria(
        env: Env,
        trial_record_id: u64,
        principal_investigator: Address,
        inclusion_criteria: Vec<CriteriaRule>,
        exclusion_criteria: Vec<CriteriaRule>,
    ) -> Result<(), Error> {
        principal_investigator.require_auth();

        // Verify trial exists and PI is authorized
        let trial = storage::get_trial(&env, trial_record_id)?;
        if trial.principal_investigator != principal_investigator {
            return Err(Error::Unauthorized);
        }

        let criteria = EligibilityCriteria {
            trial_record_id,
            inclusion_criteria,
            exclusion_criteria,
        };

        storage::save_criteria(&env, &criteria);

        Ok(())
    }

    /// Check patient eligibility for a trial
    pub fn check_patient_eligibility(
        env: Env,
        trial_record_id: u64,
        patient_id: Address,
        _patient_data_hash: BytesN<32>,
    ) -> Result<EligibilityResult, Error> {
        patient_id.require_auth();

        // Verify trial exists
        let _trial = storage::get_trial(&env, trial_record_id)?;

        // Get eligibility criteria
        let criteria = storage::get_criteria(&env, trial_record_id)?;

        let mut met_inclusion = Vec::new(&env);
        let mut met_exclusion = Vec::new(&env);
        let disqualifying_factors = Vec::new(&env);
        let mut evaluation_artifacts = Vec::new(&env);

        for rule in criteria.inclusion_criteria.iter() {
            let (passed, artifact) = Self::evaluate_rule(
                &env,
                trial_record_id,
                &patient_data_hash,
                &rule,
                &claim_evidence,
            );
            met_inclusion.push_back(passed);
            evaluation_artifacts.push_back(artifact);
        }

        for rule in criteria.exclusion_criteria.iter() {
            let (matched, artifact) = Self::evaluate_rule(
                &env,
                trial_record_id,
                &patient_data_hash,
                &rule,
                &claim_evidence,
            );
            met_exclusion.push_back(matched);
            evaluation_artifacts.push_back(artifact);
        }

        let eligible = met_inclusion.iter().all(|x| x) && met_exclusion.iter().all(|x| !x);

        Ok(EligibilityResult {
            eligible,
            met_inclusion,
            met_exclusion,
            disqualifying_factors,
            evaluation_artifacts,
        })
    }

    /// Enroll a participant in a trial
    pub fn enroll_participant(
        env: Env,
        trial_record_id: u64,
        patient_id: Address,
        study_arm: Symbol,
        enrollment_date: u64,
        informed_consent_hash: BytesN<32>,
        participant_id: String,
    ) -> Result<u64, Error> {
        patient_id.require_auth();

        // Validate date
        validation::validate_date_not_future(&env, enrollment_date)?;

        // Verify informed consent hash matches current consent version
        if !Self::is_valid_consent(&env, &informed_consent_hash) {
            return Err(Error::InvalidConsent);
        }

        // Verify trial exists and is active
        let mut trial = storage::get_trial(&env, trial_record_id)?;
        if trial.status != TrialStatus::Active {
            return Err(Error::TrialNotActive);
        }

        // Check enrollment capacity
        if trial.current_enrollment >= trial.enrollment_target {
            return Err(Error::EnrollmentFull);
        }

        // Check for duplicate enrollment
        if storage::check_duplicate_enrollment(&env, trial_record_id, &patient_id) {
            return Err(Error::DuplicateEnrollment);
        }

        // Generate unique enrollment ID
        let enrollment_id = storage::get_next_enrollment_id(&env);

        let enrollment = ParticipantEnrollment {
            enrollment_id,
            trial_record_id,
            patient_id: patient_id.clone(),
            study_arm,
            enrollment_date,
            informed_consent_hash,
            participant_id: participant_id.clone(),
            status: EnrollmentStatus::Active,
            withdrawal_date: None,
            withdrawal_reason: None,
            data_retention_consent: true,
            retention_class: DataRetentionClass::Optional,
        };

        // Store enrollment record
        storage::save_enrollment(&env, &enrollment);
        storage::add_trial_enrollment(&env, trial_record_id, enrollment_id);
        storage::add_patient_enrollment(&env, &patient_id, enrollment_id);

        // Update trial enrollment count
        trial.current_enrollment += 1;
        storage::save_trial(&env, &trial);

        // Emit event
        ParticipantEnrolled {
            enrollment_id,
            trial_record_id,
            participant_id: participant_id.clone(),
        }
        .publish(&env);

        Ok(enrollment_id)
    }

    /// Check if the informed consent hash matches the current consent version
    fn is_valid_consent(env: &Env, informed_consent_hash: &BytesN<32>) -> bool {
        // In a real implementation, this would invoke the patient registry contract
        // For now, return true as placeholder
        // let patient_registry: Address = env.storage().instance().get(&DataKey::PatientRegistry).unwrap();
        // let current_version: BytesN<32> = env.invoke_contract(&patient_registry, &symbol_short!("get_consent_version"), ()).unwrap();
        // informed_consent_hash == &current_version
        true
    }

    /// Record a study visit
    pub fn record_study_visit(
        env: Env,
        enrollment_id: u64,
        visit_number: u32,
        visit_date: u64,
        visit_type: Symbol,
        data_collected_hash: BytesN<32>,
        adverse_events: Vec<AdverseEvent>,
    ) -> Result<(), Error> {
        // Verify enrollment exists
        let enrollment = storage::get_enrollment(&env, enrollment_id)?;
        enrollment.patient_id.require_auth();

        // Validate date
        validation::validate_date_not_future(&env, visit_date)?;

        if enrollment.status == EnrollmentStatus::Withdrawn && !enrollment.data_retention_consent {
            return Err(Error::WithdrawalRestricted);
        }

        let visit = StudyVisit {
            enrollment_id,
            visit_number,
            visit_date,
            visit_type,
            data_collected_hash,
            adverse_events,
            retention_class: DataRetentionClass::Optional,
        };

        storage::save_study_visit(&env, &visit);

        Ok(())
    }

    /// Report an adverse event
    pub fn report_adverse_event(
        env: Env,
        enrollment_id: u64,
        event_type: Symbol,
        severity: Symbol,
        event_description_hash: BytesN<32>,
        onset_date: u64,
        resolution_date: Option<u64>,
        causality_assessment: Symbol,
    ) -> Result<u64, Error> {
        // Verify enrollment exists
        let enrollment = storage::get_enrollment(&env, enrollment_id)?;

        // Get trial to verify PI authorization
        let trial = storage::get_trial(&env, enrollment.trial_record_id)?;
        trial.principal_investigator.require_auth();

        // Validate inputs
        validation::validate_severity(&severity)?;
        validation::validate_causality(&causality_assessment)?;
        validation::validate_date_not_future(&env, onset_date)?;

        if let Some(res_date) = resolution_date {
            validation::validate_date_not_future(&env, res_date)?;
            if res_date < onset_date {
                return Err(Error::InvalidDateRange);
            }
        }

        // Generate unique event ID
        let event_id = storage::get_next_event_id(&env);

        let event = AdverseEventReport {
            event_id,
            enrollment_id,
            event_type: event_type.clone(),
            severity: severity.clone(),
            event_description_hash,
            onset_date,
            resolution_date,
            causality_assessment,
            retention_class: DataRetentionClass::RegulatoryRequired,
        };

        storage::save_adverse_event(&env, &event);

        // Emit event
        AdverseEventReported {
            event_id,
            enrollment_id,
            severity: severity.clone(),
        }
        .publish(&env);

        Ok(event_id)
    }

    /// Withdraw a participant from the trial
    pub fn withdraw_participant(
        env: Env,
        enrollment_id: u64,
        withdrawal_date: u64,
        withdrawal_reason: Symbol,
        data_retention_consent: bool,
    ) -> Result<(), Error> {
        // Verify enrollment exists
        let mut enrollment = storage::get_enrollment(&env, enrollment_id)?;
        enrollment.patient_id.require_auth();

        // Check if already withdrawn
        if enrollment.status == EnrollmentStatus::Withdrawn {
            return Err(Error::AlreadyWithdrawn);
        }

        // Validate inputs
        validation::validate_date_not_future(&env, withdrawal_date)?;
        validation::validate_withdrawal_reason(&withdrawal_reason)?;

        // Update enrollment status
        enrollment.status = EnrollmentStatus::Withdrawn;
        enrollment.withdrawal_date = Some(withdrawal_date);
        enrollment.withdrawal_reason = Some(withdrawal_reason.clone());
        enrollment.data_retention_consent = data_retention_consent;

        storage::save_enrollment(&env, &enrollment);

        // Update trial enrollment count
        let mut trial = storage::get_trial(&env, enrollment.trial_record_id)?;
        if trial.current_enrollment > 0 {
            trial.current_enrollment -= 1;
        }
        storage::save_trial(&env, &trial);

        // Emit withdrawal event
        ParticipantWithdrawn {
            enrollment_id,
            withdrawal_date,
        }
        .publish(&env);

        // Emit retention policy events to reflect required versus optional retention
        RetentionPolicyApplied {
            enrollment_id,
            data_class: DataRetentionClass::Optional,
            consented: data_retention_consent,
        }
        .publish(&env);

        RetentionPolicyApplied {
            enrollment_id,
            data_class: DataRetentionClass::RegulatoryRequired,
            consented: true,
        }
        .publish(&env);

        Ok(())
    }

    /// Record a protocol deviation
    pub fn record_protocol_deviation(
        env: Env,
        enrollment_id: u64,
        deviation_type: Symbol,
        deviation_description_hash: BytesN<32>,
        corrective_action_hash: BytesN<32>,
        reported_to_irb: bool,
    ) -> Result<(), Error> {
        // Verify enrollment exists
        let enrollment = storage::get_enrollment(&env, enrollment_id)?;

        // Get trial to verify PI authorization
        let trial = storage::get_trial(&env, enrollment.trial_record_id)?;
        trial.principal_investigator.require_auth();

        let deviation = ProtocolDeviation {
            enrollment_id,
            deviation_type,
            deviation_description_hash,
            corrective_action_hash,
            reported_to_irb,
            reported_date: env.ledger().timestamp(),
            retention_class: DataRetentionClass::RegulatoryRequired,
        };

        storage::save_protocol_deviation(&env, enrollment_id, &deviation);

        Ok(())
    }

    /// Submit a safety report for the trial
    pub fn submit_safety_report(
        env: Env,
        trial_record_id: u64,
        principal_investigator: Address,
        reporting_period: u64,
        safety_data_hash: BytesN<32>,
        serious_adverse_events: u32,
    ) -> Result<(), Error> {
        principal_investigator.require_auth();

        // Verify trial exists and PI is authorized
        let trial = storage::get_trial(&env, trial_record_id)?;
        if trial.principal_investigator != principal_investigator {
            return Err(Error::Unauthorized);
        }

        let report = SafetyReport {
            trial_record_id,
            reporting_period,
            safety_data_hash,
            serious_adverse_events,
            submitted_by: principal_investigator.clone(),
            submitted_date: env.ledger().timestamp(),
            retention_class: DataRetentionClass::RegulatoryRequired,
        };

        storage::save_safety_report(&env, trial_record_id, &report);

        // Emit event
        SafetyReportSubmitted {
            trial_record_id,
            reporting_period,
        }
        .publish(&env);

        Ok(())
    }

    /// Export de-identified data for analysis
    pub fn export_deidentified_data(
        env: Env,
        trial_record_id: u64,
        principal_investigator: Address,
        data_filters: DataFilters,
    ) -> Result<BytesN<32>, Error> {
        principal_investigator.require_auth();

        // Verify trial exists and PI is authorized
        let trial = storage::get_trial(&env, trial_record_id)?;
        if trial.principal_investigator != principal_investigator {
            return Err(Error::Unauthorized);
        }

        // In a real implementation, this would:
        // 1. Collect enrollment data based on filters
        // 2. Remove all identifying information
        // 3. Generate a dataset hash
        // 4. Store the dataset securely
        // For now, we'll return a placeholder hash

        let enrollments = storage::get_trial_enrollments(&env, trial_record_id);
        let mut export_count = 0u32;

        for enrollment_id in enrollments.iter() {
            if let Ok(enrollment) = storage::get_enrollment(&env, enrollment_id) {
                // Apply filters
                let include = match enrollment.status {
                    EnrollmentStatus::Withdrawn => {
                        data_filters.include_withdrawn && enrollment.data_retention_consent
                    }
                    _ => true,
                };

                if include {
                    export_count += 1;
                }
            }
        }

        // Generate a hash representing the exported dataset
        // In production, this would be a hash of the actual de-identified data
        let export_hash = env.crypto().sha256(&soroban_sdk::Bytes::from_slice(
            &env,
            &export_count.to_be_bytes(),
        ));

        Ok(export_hash.into())
    }

    /// Get trial information
    pub fn get_trial(env: Env, trial_record_id: u64) -> Result<types::ClinicalTrial, Error> {
        storage::get_trial(&env, trial_record_id)
    }

    /// Get enrollment information
    pub fn get_enrollment(
        env: Env,
        enrollment_id: u64,
        requester: Address,
    ) -> Result<ParticipantEnrollment, Error> {
        requester.require_auth();

        let enrollment = storage::get_enrollment(&env, enrollment_id)?;

        // Check authorization (patient or PI)
        let trial = storage::get_trial(&env, enrollment.trial_record_id)?;
        if requester != enrollment.patient_id && requester != trial.principal_investigator {
            return Err(Error::Unauthorized);
        }

        Ok(enrollment)
    }

    /// Get adverse event report
    pub fn get_adverse_event(
        env: Env,
        event_id: u64,
        requester: Address,
    ) -> Result<AdverseEventReport, Error> {
        requester.require_auth();

        let event = storage::get_adverse_event(&env, event_id)?;
        let enrollment = storage::get_enrollment(&env, event.enrollment_id)?;
        let trial = storage::get_trial(&env, enrollment.trial_record_id)?;

        // Check authorization
        if requester != trial.principal_investigator {
            return Err(Error::Unauthorized);
        }

        Ok(event)
    }

    fn evaluate_rule(
        env: &Env,
        trial_record_id: u64,
        patient_data_hash: &BytesN<32>,
        rule: &CriteriaRule,
        claim_evidence: &Vec<EligibilityClaimEvidence>,
    ) -> (bool, RuleEvaluationArtifact) {
        let expected_claim_hash =
            Self::compute_expected_claim_hash(env, trial_record_id, patient_data_hash, rule);

        for evidence in claim_evidence.iter() {
            if evidence.claim_hash == expected_claim_hash {
                return (
                    true,
                    RuleEvaluationArtifact {
                        criteria_type: rule.criteria_type.clone(),
                        parameter: rule.parameter.clone(),
                        operator: rule.operator.clone(),
                        value: rule.value.clone(),
                        expected_claim_hash: expected_claim_hash.clone(),
                        matched_claim_hash: Some(evidence.claim_hash),
                        evidence_type: Some(evidence.evidence_type),
                        passed: true,
                        explanation: String::from_str(
                            env,
                            "Matched deterministic claim hash from attestation/zk evidence",
                        ),
                    },
                );
            }
        }

        (
            false,
            RuleEvaluationArtifact {
                criteria_type: rule.criteria_type.clone(),
                parameter: rule.parameter.clone(),
                operator: rule.operator.clone(),
                value: rule.value.clone(),
                expected_claim_hash,
                matched_claim_hash: None,
                evidence_type: None,
                passed: false,
                explanation: String::from_str(
                    env,
                    "No matching deterministic claim hash found for rule",
                ),
            },
        )
    }

    fn compute_expected_claim_hash(
        env: &Env,
        trial_record_id: u64,
        patient_data_hash: &BytesN<32>,
        rule: &CriteriaRule,
    ) -> BytesN<32> {
        let mut payload = Bytes::new(env);
        payload.append(&Bytes::from_slice(env, b"trial-eligibility-v1"));
        payload.append(&Bytes::from_slice(env, &trial_record_id.to_be_bytes()));
        payload.append(&patient_data_hash.clone().into());
        payload.append(&rule.criteria_type.to_string().into());
        payload.append(&rule.parameter.to_xdr(env));
        payload.append(&rule.operator.to_string().into());
        payload.append(&rule.value.to_xdr(env));
        env.crypto().sha256(&payload).into()
    }
}

#[cfg(test)]
mod test;
