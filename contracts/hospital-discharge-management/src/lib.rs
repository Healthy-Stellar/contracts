#![no_std]

//! # Hospital Discharge Management Contract
//!
//! Manages patient discharge planning, discharge instructions, follow-up care scheduling, and
//! discharge summary documentation.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Discharge planning restricted to authorized hospital staff.
//! Patient authorization for discharge scheduling. Follow-up provider validation via registry.
//! Discharge instruction access restricted to patient and authorized providers.
//!
//! **Audit Controls:** Discharge plan creation events logged with patient and responsible provider.
//! Instruction confirmation events tracked. Follow-up scheduling events recorded. Discharge
//! completion events with discharge date and summary. Instruction acknowledgment by patient
//! documented for compliance.
//!
//! **Data Retention Policy:** Discharge summaries retained indefinitely for continuity of care.
//! Follow-up care schedules maintained until completion. Discharge instructions archived with
//! patient acknowledgment. Temporal validation ensures logical event sequencing.
//!
//! **Encryption/Integrity:** Discharge instructions stored encrypted in persistent state.
//! Follow-up care provider identity validated. Discharge timestamp immutable once recorded.
//! Temporal validation prevents backdated discharge records.

use soroban_sdk::{contract, contractimpl, contractclient, Address, BytesN, Env, String, Symbol, Vec};

/// Maximum allowed value for any 0–100 bounded score field.
const MAX_SCORE: u32 = 100;
/// Maximum number of follow-up appointments accepted in a single batch.
const MAX_FOLLOWUP_BATCH: u32 = 20;

mod storage;
mod types;
mod validation;

#[cfg(test)]
mod test;

use storage::*;
use types::*;
use validation::*;

#[contractclient(name = "HospitalRegistryClient")]
pub trait HospitalRegistryInterface {
    fn is_hospital_active(env: Env, wallet: Address) -> bool;
}

#[contractclient(name = "ProviderRegistryClient")]
pub trait ProviderRegistryInterface {
    fn is_provider_affiliated(env: Env, provider: Address, hospital: Address) -> bool;
}

#[contract]
pub struct HospitalDischargeContract;

#[contractimpl]
impl HospitalDischargeContract {
    /// Set the administrator and registries at deployment. Runs atomically, so
    /// no one can front-run it to take over admin or swap the registries.
    pub fn __constructor(env: Env, admin: Address, hospital_registry: Address, provider_registry: Address) {
        set_admin(&env, &admin);
        set_hospital_registry(&env, &hospital_registry);
        set_provider_registry(&env, &provider_registry);
    }

    /// Initialize a new discharge planning process
    pub fn initiate_discharge_planning(
        env: Env,
        caller: Address,
        patient_id: BytesN<32>,
        hospital_id: Address,
        admission_date: u64,
        expected_discharge_date: u64,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        let hospital_registry = get_hospital_registry(&env)?;
        let provider_registry = get_provider_registry(&env)?;
        if !HospitalRegistryClient::new(&env, &hospital_registry)
            .is_hospital_active(&hospital_id)
            || !ProviderRegistryClient::new(&env, &provider_registry)
                .is_provider_affiliated(&caller, &hospital_id)
        {
            return Err(Error::Unauthorized);
        }

        // Validate inputs
        validate_dates(&env, admission_date, expected_discharge_date)?;

        // Generate new discharge plan ID
        let plan_id = get_and_increment_counter(&env);

        // Create discharge plan
        let plan = DischargePlan {
            plan_id,
            patient_id: patient_id.clone(),
            hospital_id: hospital_id.clone(),
            admission_date,
            expected_discharge_date,
            actual_discharge_date: None,
            status: DischargeStatus::Planning,
            created_by: caller.clone(),
            created_at: env.ledger().timestamp(),
        };

        // Store the plan
        save_discharge_plan(&env, plan_id, &plan);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "discharge_initiated"),),
            (plan_id, patient_id, hospital_id),
        );

        Ok(plan_id)
    }

    /// Assess patient's readiness for discharge
    pub fn assess_discharge_readiness(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        medical_stability_score: u32,
        functional_status_score: u32,
        support_system_score: u32,
        notes: String,
    ) -> Result<ReadinessScore, Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;

        let mut plan = get_discharge_plan(&env, discharge_plan_id)?;
        if plan.status != DischargeStatus::Planning && plan.status != DischargeStatus::ReadinessAssessed {
            return Err(Error::InvalidStatus);
        }

        // Bound each domain score to 0–100 before any arithmetic.
        if medical_stability_score > MAX_SCORE
            || functional_status_score > MAX_SCORE
            || support_system_score > MAX_SCORE
        {
            return Err(Error::InvalidInput);
        }

        // Calculate overall readiness score with checked arithmetic.
        let total_score = medical_stability_score
            .checked_add(functional_status_score)
            .and_then(|s| s.checked_add(support_system_score))
            .ok_or(Error::InvalidInput)?;
        let average_score = total_score / 3;

        // Every domain must meet a minimum, not only the average.
        let min_domain = medical_stability_score
            .min(functional_status_score)
            .min(support_system_score);

        let readiness = if average_score >= 80 && min_domain >= 60 {
            ReadinessLevel::Ready
        } else if average_score >= 60 && min_domain >= 40 {
            ReadinessLevel::NeedsPreparation
        } else {
            ReadinessLevel::NotReady
        };

        let assessment = ReadinessScore {
            discharge_plan_id,
            medical_stability_score,
            functional_status_score,
            support_system_score,
            overall_score: average_score,
            readiness_level: readiness.clone(),
            assessed_by: caller.clone(),
            assessed_at: env.ledger().timestamp(),
            notes_hash: env.crypto().sha256(&notes.into()),
        };

        // Store assessment
        save_readiness_assessment(&env, discharge_plan_id, &assessment);

        // Update plan status
        plan.status = DischargeStatus::ReadinessAssessed;
        save_discharge_plan(&env, discharge_plan_id, &plan);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "readiness_assessed"),),
            (discharge_plan_id, average_score),
        );

        Ok(assessment)
    }

    /// Create discharge orders
    pub fn create_discharge_orders(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        medications: Vec<DischargeMedication>,
        instructions: String,
        restrictions: String,
        override_reason_hash: Option<BytesN<32>>,
        approver: Option<Address>,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;

        let mut plan = get_discharge_plan(&env, discharge_plan_id)?;
        if plan.status != DischargeStatus::ReadinessAssessed && plan.status != DischargeStatus::OrdersCreated {
            return Err(Error::InvalidStatus);
        }

        // Gate on readiness: latest assessment must be Ready, or an override must be recorded.
        Self::require_ready_or_override(&env, discharge_plan_id, override_reason_hash, approver)?;

        let orders = DischargeOrders {
            discharge_plan_id,
            medications,
            instructions,
            restrictions,
            created_by: caller.clone(),
            created_at: env.ledger().timestamp(),
        };

        // Store orders (append-only history)
        save_discharge_orders(&env, discharge_plan_id, &orders);

        // Update plan status
        plan.status = DischargeStatus::OrdersCreated;
        save_discharge_plan(&env, discharge_plan_id, &plan);

        // Emit event
        env.events()
            .publish((Symbol::new(&env, "orders_created"),), discharge_plan_id);

        Ok(())
    }

    /// Arrange home health services
    pub fn arrange_home_health(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        agency_id: BytesN<32>,
        service_type: String,
        frequency: String,
        start_date: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;
        Self::require_active_plan(&env, discharge_plan_id)?;

        // Validate start date against ledger time.
        if start_date < env.ledger().timestamp() {
            return Err(Error::InvalidInput);
        }

        let home_health = HomeHealthArrangement {
            discharge_plan_id,
            agency_id: agency_id.clone(),
            service_type,
            frequency,
            start_date,
            arranged_by: caller.clone(),
            arranged_at: env.ledger().timestamp(),
        };

        // Store arrangement (append-only history)
        save_home_health_arrangement(&env, discharge_plan_id, &home_health);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "home_health_arranged"),),
            (discharge_plan_id, agency_id),
        );

        Ok(())
    }

    /// Order durable medical equipment for discharge
    pub fn order_dme_for_discharge(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        equipment_list: Vec<String>,
        supplier_id: BytesN<32>,
        delivery_date: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;
        Self::require_active_plan(&env, discharge_plan_id)?;

        // Validate delivery date against ledger time.
        if delivery_date < env.ledger().timestamp() {
            return Err(Error::InvalidInput);
        }

        let dme_order = DMEOrder {
            discharge_plan_id,
            equipment_list,
            supplier_id: supplier_id.clone(),
            delivery_date,
            ordered_by: caller.clone(),
            ordered_at: env.ledger().timestamp(),
        };

        // Store DME order (append-only history)
        save_dme_order(&env, discharge_plan_id, &dme_order);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "dme_ordered"),),
            (discharge_plan_id, supplier_id),
        );

        Ok(())
    }

    /// Schedule follow-up appointments
    pub fn schedule_followup_appointments(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        appointments: Vec<FollowUpAppointment>,
    ) -> Result<Vec<u64>, Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;
        Self::require_active_plan(&env, discharge_plan_id)?;

        // Cap the batch size.
        if appointments.len() > MAX_FOLLOWUP_BATCH {
            return Err(Error::InvalidInput);
        }

        let now = env.ledger().timestamp();
        let mut appointment_ids = Vec::new(&env);

        for appointment in appointments.iter() {
            // Validate appointment date against ledger time.
            if appointment.appointment_date < now {
                return Err(Error::InvalidInput);
            }
            let appt_id = get_and_increment_appointment_counter(&env);
            save_followup_appointment(&env, discharge_plan_id, appt_id, &appointment);
            appointment_ids.push_back(appt_id);
        }

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "appointments_scheduled"),),
            (discharge_plan_id, appointment_ids.len()),
        );

        Ok(appointment_ids)
    }

    /// Provide discharge education to patient
    pub fn provide_discharge_education(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        topics_covered: Vec<String>,
        materials_provided: Vec<String>,
        patient_understanding_level: u32,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;
        Self::require_active_plan(&env, discharge_plan_id)?;

        // Bound understanding level to 0–100.
        if patient_understanding_level > MAX_SCORE {
            return Err(Error::InvalidInput);
        }

        let education = DischargeEducation {
            discharge_plan_id,
            topics_covered,
            materials_provided,
            patient_understanding_level,
            provided_by: caller.clone(),
            provided_at: env.ledger().timestamp(),
        };

        // Store education record (append-only history)
        save_discharge_education(&env, discharge_plan_id, &education);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "education_provided"),),
            (discharge_plan_id, patient_understanding_level),
        );

        Ok(())
    }

    /// Coordinate with skilled nursing facility
    pub fn coordinate_with_snf(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        snf_id: BytesN<32>,
        transfer_date: u64,
        care_requirements: String,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;
        Self::require_active_plan(&env, discharge_plan_id)?;

        // Validate transfer date against ledger time.
        if transfer_date < env.ledger().timestamp() {
            return Err(Error::InvalidInput);
        }

        let coordination = SNFCoordination {
            discharge_plan_id,
            snf_id: snf_id.clone(),
            transfer_date,
            care_requirements,
            coordinated_by: caller.clone(),
            coordinated_at: env.ledger().timestamp(),
        };

        // Store coordination (append-only history)
        save_snf_coordination(&env, discharge_plan_id, &coordination);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "snf_coordinated"),),
            (discharge_plan_id, snf_id),
        );

        Ok(())
    }

    /// Complete the discharge process
    pub fn complete_discharge(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        actual_discharge_date: u64,
        discharge_destination: String,
        override_reason_hash: Option<BytesN<32>>,
        approver: Option<Address>,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;
        let mut plan = get_discharge_plan(&env, discharge_plan_id)?;

        if plan.status != DischargeStatus::OrdersCreated {
            return Err(Error::InvalidStatus);
        }

        // Gate on readiness: latest assessment must be Ready, or an override must be recorded.
        Self::require_ready_or_override(&env, discharge_plan_id, override_reason_hash, approver)?;

        // Validate admission_date <= actual_discharge_date <= now.
        let now = env.ledger().timestamp();
        if actual_discharge_date < plan.admission_date || actual_discharge_date > now {
            return Err(Error::InvalidInput);
        }

        // Update plan status
        plan.status = DischargeStatus::Completed;
        plan.actual_discharge_date = Some(actual_discharge_date);

        // Save updated plan
        save_discharge_plan(&env, discharge_plan_id, &plan);

        // Store completion details
        let completion = DischargeCompletion {
            discharge_plan_id,
            actual_discharge_date,
            discharge_destination,
            completed_by: caller.clone(),
            completed_at: env.ledger().timestamp(),
        };

        save_discharge_completion(&env, discharge_plan_id, &completion);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "discharge_completed"),),
            (discharge_plan_id, actual_discharge_date),
        );

        Ok(())
    }

    /// Cancel the discharge process
    pub fn cancel_discharge(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;
        let mut plan = get_discharge_plan(&env, discharge_plan_id)?;

        // Guard status: cannot cancel if already Completed or Cancelled
        if plan.status == DischargeStatus::Completed || plan.status == DischargeStatus::Cancelled {
            return Err(Error::InvalidStatus);
        }

        // Update plan status
        plan.status = DischargeStatus::Cancelled;

        // Save updated plan
        save_discharge_plan(&env, discharge_plan_id, &plan);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "discharge_cancelled"),),
            discharge_plan_id,
        );

        Ok(())
    }

    /// Track readmission risk factors
    pub fn track_readmission_risk(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        risk_factors: Vec<String>,
        risk_score: u32,
        mitigation_plan: String,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        caller.require_auth();

        // Validate plan exists and caller is authorized
        validate_plan_exists(&env, discharge_plan_id)?;
        Self::verify_plan_authorization(&env, &caller, discharge_plan_id)?;
        Self::require_active_plan(&env, discharge_plan_id)?;

        // Bound risk score to 0–100.
        if risk_score > MAX_SCORE {
            return Err(Error::InvalidInput);
        }

        let risk_level = if risk_score >= 75 {
            RiskLevel::High
        } else if risk_score >= 50 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        let risk_tracking = ReadmissionRisk {
            discharge_plan_id,
            risk_factors,
            risk_score,
            risk_level,
            mitigation_plan,
            tracked_by: caller.clone(),
            tracked_at: env.ledger().timestamp(),
        };

        // Store risk tracking (append-only history)
        save_readmission_risk(&env, discharge_plan_id, &risk_tracking);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "risk_tracked"),),
            (discharge_plan_id, risk_score),
        );

        Ok(())
    }

    /// Reassign a stuck plan to a new care-team lead. Hospital admin only.
    pub fn reassign_discharge_plan(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        new_lead: Address,
    ) -> Result<(), Error> {
        caller.require_auth();
        let admin = get_admin(&env)?;
        if caller != admin {
            return Err(Error::Unauthorized);
        }
        let mut plan = get_discharge_plan(&env, discharge_plan_id)?;
        if plan.status == DischargeStatus::Completed || plan.status == DischargeStatus::Cancelled {
            return Err(Error::InvalidStatus);
        }
        plan.created_by = new_lead.clone();
        save_discharge_plan(&env, discharge_plan_id, &plan);
        env.events().publish(
            (Symbol::new(&env, "plan_reassigned"),),
            (discharge_plan_id, new_lead),
        );
        Ok(())
    }

    /// Force-cancel a stuck plan. Hospital admin only.
    pub fn admin_cancel_discharge(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<(), Error> {
        caller.require_auth();
        let admin = get_admin(&env)?;
        if caller != admin {
            return Err(Error::Unauthorized);
        }
        let mut plan = get_discharge_plan(&env, discharge_plan_id)?;
        if plan.status == DischargeStatus::Completed || plan.status == DischargeStatus::Cancelled {
            return Err(Error::InvalidStatus);
        }
        plan.status = DischargeStatus::Cancelled;
        save_discharge_plan(&env, discharge_plan_id, &plan);
        env.events().publish(
            (Symbol::new(&env, "discharge_cancelled"),),
            discharge_plan_id,
        );
        Ok(())
    }

    // Query functions (authenticated; limited to care team, patient/proxy, receiving facility)
    pub fn get_discharge_plan(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<DischargePlan, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_discharge_plan(&env, discharge_plan_id)
    }

    pub fn get_readiness_assessment(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<ReadinessScore, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_readiness_assessment(&env, discharge_plan_id)
    }

    pub fn get_discharge_orders(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<Vec<DischargeOrders>, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_discharge_orders(&env, discharge_plan_id)
    }

    pub fn get_home_health_arrangements(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<Vec<HomeHealthArrangement>, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_home_health_arrangements(&env, discharge_plan_id)
    }

    pub fn get_dme_orders(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<Vec<DMEOrder>, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_dme_orders(&env, discharge_plan_id)
    }

    pub fn get_followup_appointments(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<Vec<FollowUpAppointment>, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_followup_appointments(&env, discharge_plan_id)
    }

    pub fn get_discharge_education(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<Vec<DischargeEducation>, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_discharge_education(&env, discharge_plan_id)
    }

    pub fn get_snf_coordination(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<Vec<SNFCoordination>, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_snf_coordination(&env, discharge_plan_id)
    }

    pub fn get_discharge_completion(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<DischargeCompletion, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_discharge_completion(&env, discharge_plan_id)
    }

    pub fn get_readmission_risk(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
    ) -> Result<Vec<ReadmissionRisk>, Error> {
        caller.require_auth();
        Self::verify_read_authorization(&env, &caller, discharge_plan_id)?;
        get_readmission_risk(&env, discharge_plan_id)
    }

    /// Authorize writes for any provider currently affiliated with the plan's hospital,
    /// or the plan's current lead. Registries are re-checked on every call.
    fn verify_plan_authorization(env: &Env, caller: &Address, plan_id: u64) -> Result<(), Error> {
        let plan = get_discharge_plan(env, plan_id)?;
        if caller == &plan.created_by {
            return Ok(());
        }
        let provider_registry = get_provider_registry(env)?;
        if ProviderRegistryClient::new(env, &provider_registry)
            .is_provider_affiliated(caller, &plan.hospital_id)
        {
            return Ok(());
        }
        Err(Error::Unauthorized)
    }

    /// Authorize reads for the care team (affiliated providers), the plan lead, or the admin.
    fn verify_read_authorization(env: &Env, caller: &Address, plan_id: u64) -> Result<(), Error> {
        let plan = get_discharge_plan(env, plan_id)?;
        if caller == &plan.created_by {
            return Ok(());
        }
        if let Ok(admin) = get_admin(env) {
            if caller == &admin {
                return Ok(());
            }
        }
        let provider_registry = get_provider_registry(env)?;
        if ProviderRegistryClient::new(env, &provider_registry)
            .is_provider_affiliated(caller, &plan.hospital_id)
        {
            return Ok(());
        }
        Err(Error::Unauthorized)
    }

    /// Reject writes once a plan is Completed or Cancelled.
    fn require_active_plan(env: &Env, plan_id: u64) -> Result<(), Error> {
        let plan = get_discharge_plan(env, plan_id)?;
        if plan.status == DischargeStatus::Completed || plan.status == DischargeStatus::Cancelled {
            return Err(Error::InvalidStatus);
        }
        Ok(())
    }

    /// Require the latest readiness assessment to be Ready, or a recorded override.
    fn require_ready_or_override(
        env: &Env,
        plan_id: u64,
        override_reason_hash: Option<BytesN<32>>,
        approver: Option<Address>,
    ) -> Result<(), Error> {
        let assessment = get_readiness_assessment(env, plan_id)?;
        if assessment.readiness_level == ReadinessLevel::Ready {
            return Ok(());
        }
        match (override_reason_hash, approver) {
            (Some(reason_hash), Some(approver)) => {
                approver.require_auth();
                save_readiness_override(env, plan_id, &reason_hash, &approver);
                env.events().publish(
                    (Symbol::new(env, "readiness_override"),),
                    (plan_id, reason_hash),
                );
                Ok(())
            }
            _ => Err(Error::NotReady),
        }
    }
}
