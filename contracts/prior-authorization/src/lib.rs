#![no_std]
#![allow(clippy::too_many_arguments)]

mod storage;
mod types;

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_enhanced;

use soroban_sdk::{contract, contractclient, contractimpl, xdr::ToXdr, Address, Bytes, BytesN, Env, String, Symbol, Vec};
use storage::*;
use types::*;
use shared::temporal;

/// Cross-contract interface for insurer-registry coverage validation (#526).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoveragePlan {
    pub plan_id: u64,
    pub plan_name: String,
    pub service_codes: Vec<String>,
    pub is_active: bool,
    pub effective_from: u64,
    pub effective_until: Option<u64>,
}

#[contractclient(name = "InsurerRegistryClient")]
pub trait InsurerRegistryInterface {
    fn get_coverage_plans(env: Env, insurer_wallet: Address) -> Vec<CoveragePlan>;
}

/// Shorter deadline (hours) assigned to escalated requests.
const ESCALATION_DEADLINE_HOURS: u64 = 4;

const MAX_APPEAL_LEVEL: u32 = 3;

fn compute_review_entry_hash(env: &Env, review: &ReviewRecord) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_array(&review.review_id.to_be_bytes());
    data.extend_from_array(&review.auth_request_id.to_be_bytes());
    data.append(&review.reviewer_id.clone().to_xdr(env));
    data.append(&review.decision.clone().to_xdr(env));
    data.append(&review.review_notes_hash.clone().to_xdr(env));
    if let Some(prev_hash) = &review.prior_review_hash {
        data.append(&prev_hash.clone().to_xdr(env));
    }
    data.extend_from_array(&review.timestamp.to_be_bytes());
    env.crypto().sha256(&data).into()
}

fn compute_appeal_chain_hash(
    env: &Env,
    previous_appeal_hash: Option<BytesN<32>>,
    ruling_dependency_hash: BytesN<32>,
    appeal_reason_hash: BytesN<32>,
    additional_evidence_hash: Option<BytesN<32>>,
    provider_id: &Address,
    appeal_level: u32,
    submitted_at: u64,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    if let Some(prev) = previous_appeal_hash {
        data.append(&prev.clone().to_xdr(env));
    }
    data.append(&ruling_dependency_hash.clone().to_xdr(env));
    data.append(&appeal_reason_hash.clone().to_xdr(env));
    if let Some(additional) = additional_evidence_hash {
        data.append(&additional.clone().to_xdr(env));
    }
    data.append(&provider_id.clone().to_xdr(env));
    data.extend_from_array(&appeal_level.to_be_bytes());
    data.extend_from_array(&submitted_at.to_be_bytes());
    env.crypto().sha256(&data).into()
}

fn service_codes_covered(
    env: &Env,
    insurer_registry_id: &Address,
    insurer_wallet: &Address,
    service_codes: &Vec<String>,
) -> bool {
    let registry = InsurerRegistryClient::new(env, insurer_registry_id);
    let plans = registry.get_coverage_plans(insurer_wallet);
    let now = env.ledger().timestamp();

    for i in 0..service_codes.len() {
        let code = match service_codes.get(i) {
            Some(c) => c,
            None => return false,
        };
        let mut covered = false;

        for j in 0..plans.len() {
            let plan = match plans.get(j) {
                Some(p) => p,
                None => continue,
            };
            if !plan.is_active || plan.effective_from > now {
                continue;
            }
            if let Some(until) = plan.effective_until {
                if now > until {
                    continue;
                }
            }
            for k in 0..plan.service_codes.len() {
                if let Some(plan_code) = plan.service_codes.get(k) {
                    if plan_code == code {
                        covered = true;
                        break;
                    }
                }
            }
            if covered {
                break;
            }
        }

        if !covered {
            return false;
        }
    }

    true
}

#[contract]
pub struct PriorAuthorizationContract;

#[contractimpl]
impl PriorAuthorizationContract {
    /// One-time setup: store the insurer-registry contract address used for
    /// coverage-plan validation during authorization submission (#526).
    pub fn initialize(env: Env, insurer_registry_id: Address) -> Result<(), Error> {
        if env
            .storage()
            .instance()
            .has(&DataKey::InsurerRegistryId)
        {
            return Err(Error::AlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&DataKey::InsurerRegistryId, &insurer_registry_id);
        Ok(())
    }

    /// Submit a new prior authorization request.
    pub fn submit_prior_authorization(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        insurer_wallet: Address,
        policy_id: u64,
        authorization_type: Symbol,
        requested_service: String,
        service_codes: Vec<String>,
        diagnosis_codes: Vec<String>,
        clinical_justification_hash: BytesN<32>,
        urgency: Symbol,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let insurer_registry_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::InsurerRegistryId)
            .ok_or(Error::NotInitialized)?;

        if !service_codes_covered(
            &env,
            &insurer_registry_id,
            &insurer_wallet,
            &service_codes,
        ) {
            return Err(Error::ServiceNotCovered);
        }

        let auth_request_id = next_auth_id(&env);

        // Calculate SLA deadline based on urgency
        let sla_config = load_sla_config(&env, &urgency).unwrap_or(SLAConfig {
            urgency: urgency.clone(),
            standard_deadline_hours: 72,  // 3 days default
            expedited_deadline_hours: 24, // 1 day default
            auto_approval_threshold: 30,  // 30 days default
            requires_medical_director: false,
        });

        let is_expedited =
            urgency == Symbol::new(&env, "urgent") || urgency == Symbol::new(&env, "emergency");
        let deadline_hours = if is_expedited {
            sla_config.expedited_deadline_hours
        } else {
            sla_config.standard_deadline_hours
        };

        let sla_deadline = env.ledger().timestamp() + (deadline_hours * 3600); // Convert hours to seconds

        let req = AuthorizationRequest {
            auth_request_id,
            provider_id: provider_id.clone(),
            patient_id: patient_id.clone(),
            policy_id,
            authorization_type,
            requested_service,
            service_codes,
            diagnosis_codes,
            clinical_justification_hash,
            urgency: urgency.clone(),
            status: AuthStatus::Submitted,
            decision: None,
            approved_units: None,
            units_used: 0,
            valid_from: None,
            valid_until: None,
            submitted_at: env.ledger().timestamp(),
            decision_date: None,
            expedited: is_expedited,
            reviewer_id: None,
            reviewer_role: None,
            sla_deadline,
            auto_review_eligible: !sla_config.requires_medical_director,
        };

        save_auth_request(&env, &req);
        add_provider_auth(&env, &provider_id, auth_request_id);
        add_patient_auth(&env, &patient_id, auth_request_id);

        env.events().publish(
            (Symbol::new(&env, "auth_submitted"),),
            (auth_request_id, provider_id, patient_id, sla_deadline),
        );

        Ok(auth_request_id)
    }

    /// Attach a supporting document to an authorization request.
    pub fn attach_supporting_documentation(
        env: Env,
        auth_request_id: u64,
        provider_id: Address,
        document_hash: BytesN<32>,
        document_type: Symbol,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        if req.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        let doc = SupportingDocument {
            auth_request_id,
            provider_id: provider_id.clone(),
            document_hash,
            document_type,
            attached_at: env.ledger().timestamp(),
        };

        save_document(&env, auth_request_id, &doc);

        env.events().publish(
            (Symbol::new(&env, "document_attached"),),
            (auth_request_id, provider_id),
        );

        Ok(())
    }

    /// Review an authorization request and record a decision.
    ///
    /// Valid decisions: `approved`, `denied`, `more_info_needed`.
    pub fn review_authorization(
        env: Env,
        auth_request_id: u64,
        reviewer_id: Address,
        decision: Symbol,
        approved_units: Option<u32>,
        valid_from: Option<u64>,
        valid_until: Option<u64>,
        review_notes: String,
    ) -> Result<(), Error> {
        reviewer_id.require_auth();

        let mut req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        // Validate reviewer authorization
        let reviewer = load_reviewer(&env, &reviewer_id).ok_or(Error::ReviewerNotFound)?;

        if !reviewer.is_active {
            return Err(Error::ReviewerNotAuthorized);
        }

        // Check if reviewer has expired
        if let Some(expires_at) = reviewer.expires_at {
            if env.ledger().timestamp() > expires_at {
                return Err(Error::ReviewerNotAuthorized);
            }
        }

        // Validate reviewer role and case load
        if reviewer.current_cases >= reviewer.max_cases {
            return Err(Error::SLAViolation);
        }

        // Check SLA deadline compliance
        if env.ledger().timestamp() > req.sla_deadline {
            return Err(Error::DeadlineExceeded);
        }

        // Only Submitted, UnderReview, or MoreInfoNeeded can be reviewed
        match req.status {
            AuthStatus::Submitted
            | AuthStatus::UnderReview
            | AuthStatus::MoreInfoNeeded
            | AuthStatus::PeerToPeerScheduled => {}
            _ => return Err(Error::InvalidStatusTransition),
        }

        // Validate reviewer role requirements
        let medical_director_role = Symbol::new(&env, "medical_director");
        let specialist_role = Symbol::new(&env, "specialist");
        let reviewer_role_sym = Symbol::new(&env, "reviewer");
        let case_manager_role = Symbol::new(&env, "case_manager");

        // Check if medical director is required for this type
        let sla_config = load_sla_config(&env, &req.urgency);
        if let Some(config) = sla_config {
            if config.requires_medical_director && reviewer.role != medical_director_role {
                return Err(Error::InvalidReviewerRole);
            }
        }

        let approved_sym = Symbol::new(&env, "approved");
        let denied_sym = Symbol::new(&env, "denied");
        let more_info_sym = Symbol::new(&env, "more_info_needed");

        // Update reviewer case count
        update_reviewer_case_count(&env, &reviewer_id, 1)?;

        if decision == approved_sym {
            // #215 – validate the authorization validity window on approval
            let effective_from = valid_from.unwrap_or(env.ledger().timestamp());
            let effective_until =
                valid_until.unwrap_or(env.ledger().timestamp() + (30 * 24 * 60 * 60));

            temporal::must_be_future(&env, effective_until)
                .map_err(|_| Error::InvalidDecision)?;
            temporal::within_validity_window(
                effective_from,
                effective_until,
                shared::temporal::MAX_VALIDITY_WINDOW_SECS,
            )
            .map_err(|_| Error::InvalidDecision)?;

            req.status = AuthStatus::Approved;
            req.approved_units = approved_units;
            req.valid_from = Some(effective_from);
            req.valid_until = Some(effective_until);
            req.decision_date = Some(env.ledger().timestamp());

            // Remove from overdue tracking if present
            remove_overdue_auth(&env, auth_request_id);
        } else if decision == denied_sym {
            req.status = AuthStatus::Denied;
            req.decision_date = Some(env.ledger().timestamp());

            // Remove from overdue tracking if present
            remove_overdue_auth(&env, auth_request_id);
        } else if decision == more_info_sym {
            req.status = AuthStatus::MoreInfoNeeded;
        } else {
            // Revert case count increment for invalid decision
            update_reviewer_case_count(&env, &reviewer_id, -1)?;
            return Err(Error::InvalidDecision);
        }

        req.reviewer_id = Some(reviewer_id.clone());
        req.reviewer_role = Some(reviewer.role.clone());

        req.decision = Some(decision.clone());

        // Persist review history in an append-only sequence.
        let history = load_review_history(&env, auth_request_id);
        let prior_review_hash = if history.is_empty() {
            None
        } else {
            let last_review = history
                .get(history.len() - 1)
                .ok_or(Error::ReviewNotFound)?;
            Some(last_review.review_entry_hash.clone())
        };
        let review_notes_hash: BytesN<32> = env.crypto().sha256(&review_notes.clone().to_xdr(&env)).into();
        let review_id = next_review_id(&env);
        let mut review_record = ReviewRecord {
            review_id,
            auth_request_id,
            reviewer_id: reviewer_id.clone(),
            decision: decision.clone(),
            review_notes_hash,
            prior_review_hash,
            review_entry_hash: BytesN::from_array(&env, &[0u8; 32]),
            timestamp: env.ledger().timestamp(),
        };
        review_record.review_entry_hash = compute_review_entry_hash(&env, &review_record);
        save_review_record(&env, &review_record);

        save_auth_request(&env, &req);

        env.events().publish(
            (Symbol::new(&env, "auth_reviewed"),),
            (
                auth_request_id,
                decision,
                reviewer_id,
                review_record.review_entry_hash,
            ),
        );

        Ok(())
    }

    /// Request a peer-to-peer review for a pending or denied authorization.
    pub fn request_peer_to_peer(
        env: Env,
        auth_request_id: u64,
        provider_id: Address,
        requested_date: u64,
        preferred_times: Vec<String>,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        if req.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        if load_peer_to_peer(&env, auth_request_id).is_some() {
            return Err(Error::PeerToPeerAlreadyScheduled);
        }

        let p2p = PeerToPeerRequest {
            auth_request_id,
            provider_id: provider_id.clone(),
            requested_date,
            preferred_times,
            scheduled_time: None,
            medical_director: None,
        };

        save_peer_to_peer(&env, &p2p);

        // Transition to UnderReview if still in Submitted state
        if matches!(
            req.status,
            AuthStatus::Submitted | AuthStatus::MoreInfoNeeded
        ) {
            req.status = AuthStatus::UnderReview;
            save_auth_request(&env, &req);
        }

        env.events().publish(
            (Symbol::new(&env, "p2p_requested"),),
            (auth_request_id, provider_id),
        );

        Ok(())
    }

    /// Schedule the peer-to-peer review (performed by the insurer side).
    pub fn schedule_peer_to_peer(
        env: Env,
        auth_request_id: u64,
        insurance_admin: Address,
        scheduled_time: u64,
        medical_director: Address,
    ) -> Result<(), Error> {
        insurance_admin.require_auth();

        load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        let mut p2p = load_peer_to_peer(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        p2p.scheduled_time = Some(scheduled_time);
        p2p.medical_director = Some(medical_director.clone());

        save_peer_to_peer(&env, &p2p);

        // Update auth status
        let mut req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;
        req.status = AuthStatus::PeerToPeerScheduled;
        save_auth_request(&env, &req);

        env.events().publish(
            (Symbol::new(&env, "p2p_scheduled"),),
            (auth_request_id, scheduled_time, medical_director),
        );

        Ok(())
    }

    /// Appeal a denied authorization. Maximum 3 appeal levels.
    pub fn appeal_denial(
        env: Env,
        auth_request_id: u64,
        provider_id: Address,
        appeal_level: u32,
        appeal_reason_hash: BytesN<32>,
        additional_evidence_hash: Option<BytesN<32>>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let mut req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        if req.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        // Only denied or already-appealed requests can be appealed
        match req.status {
            AuthStatus::Denied | AuthStatus::Appealed => {}
            _ => return Err(Error::NotDenied),
        }

        if appeal_level > MAX_APPEAL_LEVEL {
            return Err(Error::MaxAppealLevelReached);
        }

        // Verify level increases monotonically
        let existing = load_appeals_for_auth(&env, auth_request_id);
        if !existing.is_empty() {
            let last = existing
                .get(existing.len() - 1)
                .ok_or(Error::AppealNotFound)?;
            if appeal_level <= last.appeal_level {
                return Err(Error::MaxAppealLevelReached);
            }
        }

        let appeal_id = next_appeal_id(&env);

        let previous_appeal_id = if existing.is_empty() {
            None
        } else {
            Some(
                existing
                    .get(existing.len() - 1)
                    .ok_or(Error::AppealNotFound)?
                    .appeal_id,
            )
        };
        let previous_appeal_hash = if existing.is_empty() {
            None
        } else {
            Some(
                existing
                    .get(existing.len() - 1)
                    .ok_or(Error::AppealNotFound)?
                    .appeal_chain_hash
                    .clone(),
            )
        };

        let review_history = load_review_history(&env, auth_request_id);
        let ruling_dependency_hash: BytesN<32> = if review_history.is_empty() {
            env.crypto().sha256(&Bytes::new(&env)).into()
        } else {
            review_history
                .get(review_history.len() - 1)
                .ok_or(Error::ReviewNotFound)?
                .review_entry_hash
                .clone()
        };

        let appeal_chain_hash = compute_appeal_chain_hash(
            &env,
            previous_appeal_hash.clone(),
            ruling_dependency_hash.clone(),
            appeal_reason_hash.clone(),
            additional_evidence_hash.clone(),
            &provider_id,
            appeal_level,
            env.ledger().timestamp(),
        );

        let appeal = Appeal {
            appeal_id,
            auth_request_id,
            provider_id: provider_id.clone(),
            appeal_level,
            appeal_reason_hash,
            additional_evidence_hash,
            submitted_at: env.ledger().timestamp(),
            previous_appeal_id,
            previous_appeal_hash,
            ruling_dependency_hash,
            appeal_chain_hash,
        };

        save_appeal(&env, &appeal);

        req.status = AuthStatus::Appealed;
        save_auth_request(&env, &req);

        env.events().publish(
            (Symbol::new(&env, "denial_appealed"),),
            (auth_request_id, appeal_id, appeal_level),
        );

        Ok(appeal_id)
    }

    /// Flag an authorization request for expedited processing.
    pub fn expedite_authorization(
        env: Env,
        auth_request_id: u64,
        provider_id: Address,
        urgency_justification: String,
        expected_service_date: u64,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        if req.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        // Only unresolved requests can be expedited
        match req.status {
            AuthStatus::Submitted | AuthStatus::UnderReview | AuthStatus::MoreInfoNeeded => {}
            _ => return Err(Error::InvalidStatusTransition),
        }

        req.expedited = true;
        save_auth_request(&env, &req);

        env.events().publish(
            (Symbol::new(&env, "auth_expedited"),),
            (
                auth_request_id,
                expected_service_date,
                urgency_justification,
            ),
        );

        Ok(())
    }

    /// Request an extension for an approved authorization.
    pub fn extend_authorization(
        env: Env,
        auth_request_id: u64,
        provider_id: Address,
        extension_reason: String,
        requested_additional_units: u32,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        if req.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        if !matches!(req.status, AuthStatus::Approved) {
            return Err(Error::NotApproved);
        }

        let ext = ExtensionRequest {
            auth_request_id,
            provider_id: provider_id.clone(),
            extension_reason,
            requested_additional_units,
            requested_at: env.ledger().timestamp(),
        };

        save_extension(&env, &ext);

        env.events().publish(
            (Symbol::new(&env, "extension_requested"),),
            (auth_request_id, requested_additional_units),
        );

        Ok(())
    }

    /// Record units used against an approved authorization.
    pub fn track_authorization_usage(
        env: Env,
        auth_request_id: u64,
        provider_id: Address,
        units_used: u32,
        service_date: u64,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        if req.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        if !matches!(req.status, AuthStatus::Approved) {
            return Err(Error::NotApproved);
        }

        // Check expiry if valid_until is set
        if let Some(valid_until) = req.valid_until {
            if env.ledger().timestamp() > valid_until {
                req.status = AuthStatus::Expired;
                save_auth_request(&env, &req);
                return Err(Error::AuthorizationExpired);
            }
        }

        // Check units ceiling
        if let Some(approved) = req.approved_units {
            if req.units_used + units_used > approved {
                return Err(Error::ExceedsApprovedUnits);
            }
        }

        req.units_used += units_used;
        save_auth_request(&env, &req);

        let record = UsageRecord {
            auth_request_id,
            provider_id: provider_id.clone(),
            units_used,
            service_date,
            recorded_at: env.ledger().timestamp(),
        };

        save_usage_record(&env, &record);

        env.events().publish(
            (Symbol::new(&env, "usage_tracked"),),
            (auth_request_id, units_used, service_date),
        );

        Ok(())
    }

    /// Register a reviewer so they can be assigned to authorization requests.
    /// The insurer registers reviewers into their pool.
    pub fn register_reviewer(
        env: Env,
        insurer_id: Address,
        reviewer_id: Address,
        role: Symbol,
        specialties: Vec<Symbol>,
        max_cases: u32,
        expires_at: Option<u64>,
    ) -> Result<(), Error> {
        insurer_id.require_auth();

        let reviewer = Reviewer {
            reviewer_id: reviewer_id.clone(),
            insurer_id: insurer_id.clone(),
            role,
            specialties,
            max_cases,
            current_cases: 0,
            authorized_at: env.ledger().timestamp(),
            expires_at,
            is_active: true,
        };

        save_reviewer(&env, &reviewer);

        env.events().publish(
            (Symbol::new(&env, "reviewer_registered"),),
            (reviewer_id, insurer_id),
        );

        Ok(())
    }

    /// Configure SLA deadlines for a given urgency level.
    pub fn configure_sla(
        env: Env,
        insurer_id: Address,
        urgency: Symbol,
        standard_deadline_hours: u64,
        expedited_deadline_hours: u64,
        auto_approval_threshold: u32,
        requires_medical_director: bool,
    ) -> Result<(), Error> {
        insurer_id.require_auth();

        let config = SLAConfig {
            urgency: urgency.clone(),
            standard_deadline_hours,
            expedited_deadline_hours,
            auto_approval_threshold,
            requires_medical_director,
        };
        save_sla_config(&env, &config);

        env.events().publish(
            (Symbol::new(&env, "sla_configured"),),
            (insurer_id, urgency),
        );

        Ok(())
    }

    /// Get the current status and summary of an authorization request.
    ///
    /// Detects SLA deadline breaches on-read: if the deadline has passed and
    /// the request is still in an unresolved state an `SLABreached` event is
    /// emitted and the request is added to the overdue list.
    pub fn get_authorization_status(
        env: Env,
        auth_request_id: u64,
        requester: Address,
    ) -> Result<AuthorizationInfo, Error> {
        requester.require_auth();

        let req = load_auth_request(&env, auth_request_id).ok_or(Error::AuthRequestNotFound)?;

        // Detect SLA breach for unresolved requests.
        let unresolved = matches!(
            req.status,
            AuthStatus::Submitted
                | AuthStatus::UnderReview
                | AuthStatus::MoreInfoNeeded
                | AuthStatus::PeerToPeerScheduled
        );
        if unresolved && env.ledger().timestamp() > req.sla_deadline {
            let breach_duration = env.ledger().timestamp().saturating_sub(req.sla_deadline);
            add_overdue_auth(&env, auth_request_id);
            env.events().publish(
                (Symbol::new(&env, "SLABreached"),),
                (auth_request_id, req.sla_deadline, env.ledger().timestamp(), breach_duration),
            );
        }

        Ok(AuthorizationInfo {
            auth_request_id: req.auth_request_id,
            provider_id: req.provider_id,
            patient_id: req.patient_id,
            requested_service: req.requested_service,
            status: req.status,
            decision: req.decision,
            approved_units: req.approved_units,
            units_used: req.units_used,
            valid_from: req.valid_from,
            valid_until: req.valid_until,
            submitted_at: req.submitted_at,
            decision_date: req.decision_date,
        })
    }

    /// Scan overdue authorization requests for an insurer and escalate them to
    /// secondary reviewers.
    ///
    /// Each escalated request is assigned to an available reviewer from the
    /// insurer's pool and given a new `ESCALATION_DEADLINE_HOURS`-hour deadline.
    /// Emits `Escalated` events with the original deadline, actual elapsed time,
    /// and breach duration.
    pub fn escalate_expired_authorizations(
        env: Env,
        insurer_id: Address,
    ) -> Result<u32, Error> {
        insurer_id.require_auth();

        let overdue_ids = get_overdue_auths(&env);
        let reviewer_ids = load_insurer_reviewers(&env, &insurer_id);

        if reviewer_ids.is_empty() {
            return Ok(0);
        }

        let now = env.ledger().timestamp();
        let mut escalated_count: u32 = 0;
        let mut reviewer_idx: u32 = 0;

        for auth_id in overdue_ids.iter() {
            let mut req = match load_auth_request(&env, auth_id) {
                Some(r) => r,
                None => continue,
            };

            // Skip already-resolved or already-escalated requests.
            let unresolved = matches!(
                req.status,
                AuthStatus::Submitted
                    | AuthStatus::UnderReview
                    | AuthStatus::MoreInfoNeeded
                    | AuthStatus::PeerToPeerScheduled
            );
            if !unresolved {
                remove_overdue_auth(&env, auth_id);
                continue;
            }

            // Pick the next available reviewer (round-robin).
            let reviewer_id = reviewer_ids
                .get(reviewer_idx % reviewer_ids.len())
                .ok_or(Error::ReviewerNotFound)?;
            reviewer_idx += 1;

            let reviewer = load_reviewer(&env, &reviewer_id).ok_or(Error::ReviewerNotFound)?;
            if !reviewer.is_active {
                continue;
            }

            let original_deadline = req.sla_deadline;
            let breach_duration = now.saturating_sub(original_deadline);
            let new_deadline = now + (ESCALATION_DEADLINE_HOURS * 3600);

            req.status = AuthStatus::Escalated;
            req.sla_deadline = new_deadline;
            req.reviewer_id = Some(reviewer_id.clone());
            req.reviewer_role = Some(reviewer.role.clone());
            save_auth_request(&env, &req);

            remove_overdue_auth(&env, auth_id);

            env.events().publish(
                (Symbol::new(&env, "Escalated"),),
                (
                    auth_id,
                    reviewer_id,
                    original_deadline,
                    now,
                    breach_duration,
                    new_deadline,
                ),
            );

            escalated_count += 1;
        }

        Ok(escalated_count)
    }

    /// Return the full appeal timeline for an authorization request.
    pub fn get_appeal_history(
        env: Env,
        auth_request_id: u64,
        requester: Address,
    ) -> Result<Vec<Appeal>, Error> {
        requester.require_auth();
        Ok(load_appeals_for_auth(&env, auth_request_id))
    }

    /// Return the full review history for an authorization request.
    pub fn get_review_history(
        env: Env,
        auth_request_id: u64,
        requester: Address,
    ) -> Result<Vec<ReviewRecord>, Error> {
        requester.require_auth();
        Ok(load_review_history(&env, auth_request_id))
    }
}
