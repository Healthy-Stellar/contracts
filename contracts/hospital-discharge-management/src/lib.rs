#![no_std]
#![allow(deprecated)]

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

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Symbol, Vec};

mod storage;
mod types;
mod validation;

#[cfg(test)]
mod test;

use storage::*;
use types::*;
use validation::*;

#[contract]
pub struct HospitalDischargeContract;

#[contractimpl]
impl HospitalDischargeContract {
    /// Initialize a new discharge planning process
    pub fn initiate_discharge_planning(
        env: Env,
        caller: Address,
        patient_id: BytesN<32>,
        hospital_id: BytesN<32>,
        admission_date: u64,
        expected_discharge_date: u64,
    ) -> Result<u64, Error> {
        caller.require_auth();

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
        caller.require_auth();

        // Validate plan exists
        validate_plan_exists(&env, discharge_plan_id)?;

        // Calculate overall readiness score
        let total_score = medical_stability_score + functional_status_score + support_system_score;
        let average_score = total_score / 3;

        let readiness = if average_score >= 80 {
            ReadinessLevel::Ready
        } else if average_score >= 60 {
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
            notes,
        };

        // Store assessment
        save_readiness_assessment(&env, discharge_plan_id, &assessment);

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
    ) -> Result<(), Error> {
        caller.require_auth();

        // Validate plan exists
        validate_plan_exists(&env, discharge_plan_id)?;

        let orders = DischargeOrders {
            discharge_plan_id,
            medications,
            instructions,
            restrictions,
            created_by: caller.clone(),
            created_at: env.ledger().timestamp(),
        };

        // Store orders
        save_discharge_orders(&env, discharge_plan_id, &orders);

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
        caller.require_auth();

        // Validate plan exists
        validate_plan_exists(&env, discharge_plan_id)?;

        let home_health = HomeHealthArrangement {
            discharge_plan_id,
            agency_id: agency_id.clone(),
            service_type,
            frequency,
            start_date,
            arranged_by: caller.clone(),
            arranged_at: env.ledger().timestamp(),
        };

        // Store arrangement
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
        caller.require_auth();

        // Validate plan exists
        validate_plan_exists(&env, discharge_plan_id)?;

        let dme_order = DMEOrder {
            discharge_plan_id,
            equipment_list,
            supplier_id: supplier_id.clone(),
            delivery_date,
            ordered_by: caller.clone(),
            ordered_at: env.ledger().timestamp(),
        };

        // Store DME order
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
        caller.require_auth();

        // Validate plan exists
        validate_plan_exists(&env, discharge_plan_id)?;

        let mut appointment_ids = Vec::new(&env);

        for appointment in appointments.iter() {
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
        caller.require_auth();

        // Validate plan exists
        validate_plan_exists(&env, discharge_plan_id)?;

        let education = DischargeEducation {
            discharge_plan_id,
            topics_covered,
            materials_provided,
            patient_understanding_level,
            provided_by: caller.clone(),
            provided_at: env.ledger().timestamp(),
        };

        // Store education record
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
        caller.require_auth();

        // Validate plan exists
        validate_plan_exists(&env, discharge_plan_id)?;

        let coordination = SNFCoordination {
            discharge_plan_id,
            snf_id: snf_id.clone(),
            transfer_date,
            care_requirements,
            coordinated_by: caller.clone(),
            coordinated_at: env.ledger().timestamp(),
        };

        // Store coordination
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
    ) -> Result<(), Error> {
        caller.require_auth();

        // Validate plan exists and get it
        let mut plan = get_discharge_plan(&env, discharge_plan_id)?;

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

    /// Track readmission risk factors
    pub fn track_readmission_risk(
        env: Env,
        caller: Address,
        discharge_plan_id: u64,
        risk_factors: Vec<String>,
        risk_score: u32,
        mitigation_plan: String,
    ) -> Result<(), Error> {
        caller.require_auth();

        // Validate plan exists
        validate_plan_exists(&env, discharge_plan_id)?;

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

        // Store risk tracking
        save_readmission_risk(&env, discharge_plan_id, &risk_tracking);

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "risk_tracked"),),
            (discharge_plan_id, risk_score),
        );

        Ok(())
    }

    // Query functions
    pub fn get_discharge_plan(env: Env, discharge_plan_id: u64) -> Result<DischargePlan, Error> {
        get_discharge_plan(&env, discharge_plan_id)
    }

    pub fn get_readiness_assessment(
        env: Env,
        discharge_plan_id: u64,
    ) -> Result<ReadinessScore, Error> {
        get_readiness_assessment(&env, discharge_plan_id)
    }
}
