#![no_std]
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

#[contractevent]
pub struct SafetyHaltProposed {
    pub trial_record_id: u64,
    pub proposed_by: Address,
}

#[contractevent]
pub struct SafetyHaltApproved {
    pub trial_record_id: u64,
    pub approval_count: u32,
}

#[contractevent]
pub struct TrialPhaseAdvanced {
    pub trial_record_id: u64,
    pub new_phase: Symbol,
    pub new_protocol_hash: BytesN<32>,
}

#[contractevent]
pub struct ProtocolAmendmentRecorded {
    pub trial_record_id: u64,
    pub new_hash: BytesN<32>,
    pub amended_by: Address,
}

#[contractevent]
pub struct ParticipantReEnrolled {
    pub enrollment_id: u64,
    pub prior_enrollment_id: u64,
    pub trial_record_id: u64,
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
    NoDsmBoardConfigured = 21,
    NotDsmBoardMember = 22,
    SafetyHaltAlreadyActive = 23,
    NoSafetyHaltPending = 24,
    AlreadyVoted = 25,
    /// The specified site does not exist
    SiteNotFound = 26,
    /// The site has reached its per-site enrollment quota
    SiteEnrollmentFull = 27,
    /// A Vec parameter exceeds its maximum allowed length.
    InputTooLarge = 28,
    /// Trial is permanently closed; no new enrolments permitted.
    TrialClosed = 29,
    /// Prior enrolment is still active; cannot re-enrol.
    PriorEnrollmentActive = 30,
}

/// Maximum number of inclusion or exclusion criteria rules per trial.
pub const MAX_CRITERIA_RULES: u32 = 64;
/// Maximum number of evidence items per eligibility check.
pub const MAX_EVIDENCE_ITEMS: u32 = 32;
/// Maximum number of DSMB members per trial.
pub const MAX_DSMB_MEMBERS: u32 = 20;
/// Maximum number of adverse events per study visit.
pub const MAX_ADVERSE_EVENTS_PER_VISIT: u32 = 50;

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
        if inclusion_criteria.len() > MAX_CRITERIA_RULES {
            return Err(Error::InputTooLarge);
        }
        if exclusion_criteria.len() > MAX_CRITERIA_RULES {
            return Err(Error::InputTooLarge);
        }

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
        patient_data_hash: BytesN<32>,
        claim_evidence: Vec<EligibilityClaimEvidence>,
    ) -> Result<EligibilityResult, Error> {
        patient_id.require_auth();
        if claim_evidence.len() > MAX_EVIDENCE_ITEMS {
            return Err(Error::InputTooLarge);
        }

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
            site_id: None,
            prior_enrollment_id: None,
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
        if adverse_events.len() > MAX_ADVERSE_EVENTS_PER_VISIT {
            return Err(Error::InputTooLarge);
        }
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

    /// Appoint DSMB members for a trial (PI only)
    pub fn appoint_dsmb(
        env: Env,
        trial_record_id: u64,
        principal_investigator: Address,
        members: Vec<Address>,
    ) -> Result<(), Error> {
        principal_investigator.require_auth();
        if members.len() > MAX_DSMB_MEMBERS {
            return Err(Error::InputTooLarge);
        }

        let trial = storage::get_trial(&env, trial_record_id)?;
        if trial.principal_investigator != principal_investigator {
            return Err(Error::Unauthorized);
        }

        storage::save_dsmb_members(&env, trial_record_id, &members);

        Ok(())
    }

    /// Propose a safety halt for a trial (DSMB member only)
    /// The proposer's vote is automatically counted as the first approval.
    pub fn propose_safety_halt(
        env: Env,
        trial_record_id: u64,
        dsmb_member: Address,
        reason_hash: BytesN<32>,
    ) -> Result<(), Error> {
        dsmb_member.require_auth();

        let members = storage::get_dsmb_members(&env, trial_record_id)
            .ok_or(Error::NoDsmBoardConfigured)?;
        if !members.contains(&dsmb_member) {
            return Err(Error::NotDsmBoardMember);
        }

        if let Some(existing) = storage::get_safety_halt(&env, trial_record_id) {
            if existing.status == types::SafetyHaltStatus::Pending {
                return Err(Error::SafetyHaltAlreadyActive);
            }
        }

        let mut approvals = Vec::new(&env);
        approvals.push_back(dsmb_member.clone());

        let proposal = types::SafetyHaltProposal {
            trial_record_id,
            proposed_by: dsmb_member.clone(),
            reason_hash,
            approvals,
            status: types::SafetyHaltStatus::Pending,
            proposed_at: env.ledger().timestamp(),
        };

        storage::save_safety_halt(&env, &proposal);

        SafetyHaltProposed {
            trial_record_id,
            proposed_by: dsmb_member,
        }
        .publish(&env);

        Ok(())
    }

    /// Cast an approval vote on the pending safety halt (DSMB member only)
    /// When votes reach majority of the board the trial is suspended and the
    /// proposal status transitions to Approved.
    pub fn approve_safety_halt(
        env: Env,
        trial_record_id: u64,
        dsmb_member: Address,
    ) -> Result<(), Error> {
        dsmb_member.require_auth();

        let members = storage::get_dsmb_members(&env, trial_record_id)
            .ok_or(Error::NoDsmBoardConfigured)?;
        if !members.contains(&dsmb_member) {
            return Err(Error::NotDsmBoardMember);
        }

        let mut proposal = storage::get_safety_halt(&env, trial_record_id)
            .ok_or(Error::NoSafetyHaltPending)?;
        if proposal.status != types::SafetyHaltStatus::Pending {
            return Err(Error::NoSafetyHaltPending);
        }

        if proposal.approvals.contains(&dsmb_member) {
            return Err(Error::AlreadyVoted);
        }

        proposal.approvals.push_back(dsmb_member);

        let threshold = members.len() / 2 + 1;
        if proposal.approvals.len() >= threshold {
            proposal.status = types::SafetyHaltStatus::Approved;

            let mut trial = storage::get_trial(&env, trial_record_id)?;
            trial.status = TrialStatus::Suspended;
            storage::save_trial(&env, &trial);

            SafetyHaltApproved {
                trial_record_id,
                approval_count: proposal.approvals.len(),
            }
            .publish(&env);
        }

        storage::save_safety_halt(&env, &proposal);

        Ok(())
    }

    /// Get the DSMB member list for a trial
    pub fn get_dsmb_members(env: Env, trial_record_id: u64) -> Result<Vec<Address>, Error> {
        storage::get_dsmb_members(&env, trial_record_id).ok_or(Error::NoDsmBoardConfigured)
    }

    /// Get the current safety-halt proposal for a trial, if any
    pub fn get_safety_halt(
        env: Env,
        trial_record_id: u64,
    ) -> Option<types::SafetyHaltProposal> {
        storage::get_safety_halt(&env, trial_record_id)
    }

    /// Add a site to a trial (PI only).
    /// The coordinator must already be a registered provider if a provider-registry is configured.
    /// Returns the new site_id.
    pub fn add_site(
        env: Env,
        trial_record_id: u64,
        principal_investigator: Address,
        coordinator: Address,
        max_enrollment: u32,
    ) -> Result<u64, Error> {
        principal_investigator.require_auth();

        let trial = storage::get_trial(&env, trial_record_id)?;
        if trial.principal_investigator != principal_investigator {
            return Err(Error::Unauthorized);
        }

        let site_id = storage::get_next_site_id(&env, trial_record_id);

        let site = types::Site {
            site_id,
            trial_record_id,
            coordinator: coordinator.clone(),
            max_enrollment,
            enrolled: 0,
        };

        storage::save_site(&env, &site);
        storage::add_trial_site(&env, trial_record_id, site_id);

        Ok(site_id)
    }

    /// Enrol a participant at a specific site.
    /// Enforces both the per-site cap and the overall trial cap.
    /// The caller must be the site coordinator.
    pub fn enrol_participant_at_site(
        env: Env,
        trial_record_id: u64,
        site_id: u64,
        coordinator: Address,
        patient_id: Address,
        study_arm: Symbol,
        enrollment_date: u64,
        informed_consent_hash: BytesN<32>,
        participant_id: String,
    ) -> Result<u64, Error> {
        coordinator.require_auth();

        // Validate date
        validation::validate_date_not_future(&env, enrollment_date)?;

        // Validate consent
        if !Self::is_valid_consent(&env, &informed_consent_hash) {
            return Err(Error::InvalidConsent);
        }

        // Verify trial is active
        let mut trial = storage::get_trial(&env, trial_record_id)?;
        if trial.status != TrialStatus::Active {
            return Err(Error::TrialNotActive);
        }

        // Fetch and validate site
        let mut site = storage::get_site(&env, trial_record_id, site_id)
            .ok_or(Error::SiteNotFound)?;

        if site.coordinator != coordinator {
            return Err(Error::Unauthorized);
        }

        // Per-site cap check
        if site.enrolled >= site.max_enrollment {
            return Err(Error::SiteEnrollmentFull);
        }

        // Overall trial cap check
        if trial.current_enrollment >= trial.enrollment_target {
            return Err(Error::EnrollmentFull);
        }

        // Duplicate enrollment check
        if storage::check_duplicate_enrollment(&env, trial_record_id, &patient_id) {
            return Err(Error::DuplicateEnrollment);
        }

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
            site_id: Some(site_id),
            prior_enrollment_id: None,
        };

        storage::save_enrollment(&env, &enrollment);
        storage::add_trial_enrollment(&env, trial_record_id, enrollment_id);
        storage::add_patient_enrollment(&env, &patient_id, enrollment_id);

        site.enrolled += 1;
        storage::save_site(&env, &site);

        trial.current_enrollment += 1;
        storage::save_trial(&env, &trial);

        ParticipantEnrolled {
            enrollment_id,
            trial_record_id,
            participant_id,
        }
        .publish(&env);

        Ok(enrollment_id)
    }

    /// Re-enrol a previously withdrawn participant.
    ///
    /// Creates a brand-new enrolment record linked to the prior enrolment via
    /// `prior_enrollment_id`.  The prior enrolment's data-deletion decision is
    /// never reversed — this function only creates a fresh record.
    ///
    /// # Errors
    /// - `TrialClosed`           — trial status is `Closed`
    /// - `TrialNotActive`        — trial is not `Active`
    /// - `EnrollmentNotFound`    — `prior_enrollment_id` does not exist
    /// - `PriorEnrollmentActive` — prior enrolment is still `Active`
    /// - `EnrollmentFull`        — trial has reached its target
    /// - `DuplicateEnrollment`   — patient already has an active enrolment
    pub fn re_enroll_participant(
        env: Env,
        trial_record_id: u64,
        patient_id: Address,
        prior_enrollment_id: u64,
        new_consent_hash: BytesN<32>,
        study_arm: Symbol,
        enrollment_date: u64,
        participant_id: String,
    ) -> Result<u64, Error> {
        patient_id.require_auth();

        validation::validate_date_not_future(&env, enrollment_date)?;

        if !Self::is_valid_consent(&env, &new_consent_hash) {
            return Err(Error::InvalidConsent);
        }

        // Verify trial
        let mut trial = storage::get_trial(&env, trial_record_id)?;
        if trial.status == TrialStatus::Closed {
            return Err(Error::TrialClosed);
        }
        if trial.status != TrialStatus::Active {
            return Err(Error::TrialNotActive);
        }

        // Validate prior enrolment
        let prior = storage::get_enrollment(&env, prior_enrollment_id)?;
        if prior.trial_record_id != trial_record_id {
            return Err(Error::EnrollmentNotFound);
        }
        if prior.status == EnrollmentStatus::Active {
            return Err(Error::PriorEnrollmentActive);
        }

        if trial.current_enrollment >= trial.enrollment_target {
            return Err(Error::EnrollmentFull);
        }

        if storage::check_duplicate_enrollment(&env, trial_record_id, &patient_id) {
            return Err(Error::DuplicateEnrollment);
        }

        let enrollment_id = storage::get_next_enrollment_id(&env);

        let enrollment = ParticipantEnrollment {
            enrollment_id,
            trial_record_id,
            patient_id: patient_id.clone(),
            study_arm,
            enrollment_date,
            informed_consent_hash: new_consent_hash,
            participant_id: participant_id.clone(),
            status: EnrollmentStatus::Active,
            withdrawal_date: None,
            withdrawal_reason: None,
            data_retention_consent: true,
            retention_class: DataRetentionClass::Optional,
            site_id: prior.site_id,
            prior_enrollment_id: Some(prior_enrollment_id),
        };

        storage::save_enrollment(&env, &enrollment);
        storage::add_trial_enrollment(&env, trial_record_id, enrollment_id);
        storage::add_patient_enrollment(&env, &patient_id, enrollment_id);

        trial.current_enrollment += 1;
        storage::save_trial(&env, &trial);

        ParticipantReEnrolled {
            enrollment_id,
            prior_enrollment_id,
            trial_record_id,
        }
        .publish(&env);

        Ok(enrollment_id)
    }

    /// Advance a trial to the next study phase (PI only).
    ///
    /// Updates the trial's `study_phase` and `protocol_hash`.  New enrolments
    /// after this call use the updated eligibility criteria (the PI must call
    /// `define_eligibility_criteria` separately to update criteria for the new
    /// phase).  Existing enrolments are unaffected.
    ///
    /// # Errors
    /// - `Unauthorized`      — caller is not the trial's PI
    /// - `TrialNotFound`     — trial does not exist
    /// - `TrialNotActive`    — trial is not `Active`
    /// - `InvalidStudyPhase` — `new_phase` is not a valid phase symbol
    pub fn advance_trial_phase(
        env: Env,
        principal_investigator: Address,
        trial_record_id: u64,
        new_phase: Symbol,
        new_protocol_hash: BytesN<32>,
    ) -> Result<(), Error> {
        principal_investigator.require_auth();

        validation::validate_study_phase(&new_phase)?;

        let mut trial = storage::get_trial(&env, trial_record_id)?;
        if trial.principal_investigator != principal_investigator {
            return Err(Error::Unauthorized);
        }
        if trial.status != TrialStatus::Active {
            return Err(Error::TrialNotActive);
        }

        trial.study_phase = new_phase.clone();
        trial.protocol_hash = new_protocol_hash.clone();
        storage::save_trial(&env, &trial);

        TrialPhaseAdvanced {
            trial_record_id,
            new_phase,
            new_protocol_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Update the protocol hash for a trial, appending an amendment record to
    /// the on-chain audit log (#485 — ICH E6(R2) GCP compliance).
    ///
    /// # Arguments
    /// * `principal_investigator` - Must be the trial's PI.
    /// * `trial_record_id`        - Target trial.
    /// * `new_hash`               - SHA-256 hash of the amended protocol document.
    /// * `reason_hash`            - SHA-256 hash of the written amendment rationale.
    pub fn update_protocol(
        env: Env,
        principal_investigator: Address,
        trial_record_id: u64,
        new_hash: BytesN<32>,
        reason_hash: BytesN<32>,
    ) -> Result<(), Error> {
        principal_investigator.require_auth();

        let mut trial = storage::get_trial(&env, trial_record_id)?;
        if trial.principal_investigator != principal_investigator {
            return Err(Error::Unauthorized);
        }
        if trial.status != TrialStatus::Active {
            return Err(Error::TrialNotActive);
        }

        let amendment = ProtocolAmendment {
            trial_record_id,
            old_hash: trial.protocol_hash.clone(),
            new_hash: new_hash.clone(),
            amended_by: principal_investigator.clone(),
            reason_hash,
            timestamp: env.ledger().timestamp(),
        };

        storage::append_amendment(&env, &amendment);

        trial.protocol_hash = new_hash.clone();
        storage::save_trial(&env, &trial);

        ProtocolAmendmentRecorded {
            trial_record_id,
            new_hash,
            amended_by: principal_investigator,
        }
        .publish(&env);

        Ok(())
    }

    /// Return the full amendment history for a trial (#485).
    /// Returns an empty vec for trials that have never been amended.
    pub fn get_amendment_history(
        env: Env,
        trial_record_id: u64,
    ) -> Vec<ProtocolAmendment> {
        storage::get_amendment_log(&env, trial_record_id)
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
        payload.append(&Bytes::from_slice(env, &rule.mandatory.then(|| 1u8).unwrap_or(0u8).to_be_bytes()));
        env.crypto().sha256(&payload).into()
    }
}

#[cfg(test)]
mod test;
