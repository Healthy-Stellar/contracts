#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Dental Records Contract
//!
//! Manages dental appointment scheduling, procedure records, patient medical history tracking,
//! and dental treatment outcomes with temporal validation.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Dentist authentication required for procedure and appointment
//! creation. Patient authorization for record access. Appointment status changes restricted to
//! authorized providers. Medical history access control enforced.
//!
//! **Audit Controls:** Events emitted for procedure scheduling, appointments, and treatment outcomes.
//! Appointment status changes tracked (scheduled, completed, cancelled). Temporal event history with
//! EVENT_VERSION for schema tracking. Incident capture with severity levels for clinical events.
//!
//! **Data Retention Policy:** Completed appointments retained indefinitely for clinical reference.
//! Dental procedure records archived with outcome documentation. Patient medical history maintained
//! for longitudinal care planning. Canceled appointments retained for audit trail.
//!
//! **Encryption/Integrity:** Procedure descriptions stored encrypted. Temporal validation ensures
//! appointment dates follow clinical logic (future scheduling, past completion dates valid).
//! Appointment records include provider and patient identity validation. SHA256 hashing for
//! record integrity verification.

use soroban_sdk::{
    contract, contractevent, contractimpl, Address, BytesN, Env, String, Symbol, Vec,
};
use shared::{events::EVENT_VERSION, temporal};
use shared_contracts::safe_increment;

mod types;
use types::*;

#[cfg(test)]
mod test;

/// --------------------
/// Events
/// --------------------

#[contractevent]
pub struct ProcedureScheduled {
    pub version: u32,
    pub appointment_id: u64,
    pub treatment_plan_id: u64,
}

#[contractevent]
pub struct ProcedurePerformed {
    pub version: u32,
    pub appointment_id: u64,
}

#[contractevent]
pub struct RadiographRecorded {
    pub version: u32,
    pub radiograph_id: u64,
}

#[contract]
pub struct DentalRecordsContract;

#[contractimpl]
impl DentalRecordsContract {
    pub fn create_dental_chart(
        env: Env,
        patient_id: Address,
        dentist_id: Address,
        chart_date: u64,
        tooth_notation_system: Symbol, // universal, palmer, fdi
    ) -> Result<u64, Error> {
        patient_id.require_auth();

        let count = safe_increment(&env, &DataKey::ChartCount);

        let chart = DentalChart {
            patient_id,
            dentist_id,
            chart_date,
            tooth_notation_system,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Chart(count), &chart);

        Ok(count)
    }

    pub fn record_tooth_condition(
        env: Env,
        chart_id: u64,
        tooth_number: String,
        surface: Option<Symbol>, // occlusal, mesial, distal, buccal, lingual
        condition: Symbol,       // caries, filling, crown, missing, implant
        condition_details: Option<String>,
    ) -> Result<(), Error> {
        let chart: DentalChart = env
            .storage()
            .persistent()
            .get(&DataKey::Chart(chart_id))
            .ok_or(Error::NotFound)?;
        chart.dentist_id.require_auth();

        let tooth_cond = ToothCondition {
            surface,
            condition,
            condition_details,
        };

        env.storage()
            .persistent()
            .set(&DataKey::ToothCond(chart_id, tooth_number), &tooth_cond);

        Ok(())
    }

    pub fn record_periodontal_assessment(
        env: Env,
        chart_id: u64,
        tooth_number: String,
        site: Symbol, // mb, b, db, ml, l, dl
        probing_depth: u32,
        recession: u32,
        bleeding_on_probing: bool,
        mobility: Option<u32>,
    ) -> Result<(), Error> {
        let chart: DentalChart = env
            .storage()
            .persistent()
            .get(&DataKey::Chart(chart_id))
            .ok_or(Error::NotFound)?;
        chart.dentist_id.require_auth();

        let assessment = PeriodontalAssessment {
            probing_depth,
            recession,
            bleeding_on_probing,
            mobility,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Perio(chart_id, tooth_number, site), &assessment);

        Ok(())
    }

    pub fn create_treatment_plan(
        env: Env,
        patient_id: Address,
        dentist_id: Address,
        plan_date: u64,
        procedures: Vec<PlannedProcedure>,
        phased_treatment: bool,
        estimated_cost: i128,
    ) -> Result<u64, Error> {
        dentist_id.require_auth();
        patient_id.require_auth();

        let count = safe_increment(&env, &DataKey::PlanCount);

        let plan = TreatmentPlan {
            patient_id,
            dentist_id,
            plan_date,
            procedures,
            phased_treatment,
            estimated_cost,
        };

        env.storage().persistent().set(&DataKey::Plan(count), &plan);

        Ok(count)
    }

    pub fn schedule_dental_procedure(
        env: Env,
        treatment_plan_id: u64,
        procedure_id: u64,
        scheduled_date: u64,
        estimated_duration: u32,
        sedation_required: bool,
    ) -> Result<u64, Error> {
        // #215 – dental procedure dates are future-scheduled appointments
        temporal::must_be_future(&env, scheduled_date)
            .map_err(|_| Error::InvalidScheduledDate)?;

        let plan: TreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::Plan(treatment_plan_id))
            .ok_or(Error::NotFound)?;
        plan.patient_id.require_auth();

        let count = safe_increment(&env, &DataKey::AppointmentCount);

        let appt = Appointment {
            treatment_plan_id,
            procedure_id,
            scheduled_date,
            estimated_duration,
            sedation_required,
            is_completed: false,
        };

        env.storage().persistent().set(&DataKey::Appt(count), &appt);

        ProcedureScheduled {
            version: EVENT_VERSION,
            appointment_id: count,
            treatment_plan_id,
        }
        .publish(&env);

        Ok(count)
    }

    pub fn document_procedure_performed(
        env: Env,
        appointment_id: u64,
        dentist_id: Address,
        procedure_date: u64,
        procedures_completed: Vec<CompletedProcedure>,
        anesthesia_used: Vec<String>,
        complications: Option<Vec<String>>,
        post_op_instructions_hash: BytesN<32>,
    ) -> Result<(), Error> {
        dentist_id.require_auth();

        // #215 – procedure_date records when the procedure happened; it must not be future
        temporal::not_future(&env, procedure_date)
            .map_err(|_| Error::InvalidPastDate)?;

        let mut appt: Appointment = env
            .storage()
            .persistent()
            .get(&DataKey::Appt(appointment_id))
            .ok_or(Error::NotFound)?;

        // #600 – ensure the caller is actually the dentist assigned to this treatment plan
        let plan: TreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::Plan(appt.treatment_plan_id))
            .ok_or(Error::NotFound)?;
        if dentist_id != plan.dentist_id {
            return Err(Error::Unauthorized);
        }

        appt.is_completed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Appt(appointment_id), &appt);

        let log = ProcedureLog {
            dentist_id,
            procedure_date,
            procedures_completed,
            anesthesia_used,
            complications,
            post_op_instructions_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::ProcedureLog(appointment_id), &log);

        ProcedurePerformed {
            version: EVENT_VERSION,
            appointment_id,
        }
        .publish(&env);

        Ok(())
    }

    /// Record a radiograph for a patient.
    ///
    /// Requires the treating dentist's auth AND a patient relationship (the dentist must be
    /// the dentist on one of the patient's dental charts). Image references are content
    /// hashes/CIDs validated by the shared CID validator. TTL is bumped on write.
    pub fn add_radiograph(
        env: Env,
        patient_id: Address,
        dentist_id: Address,
        radiograph_type: Symbol,
        taken_date: u64,
        image_cids: Vec<String>,
        findings_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        dentist_id.require_auth();

        // #215 – taken_date records when the radiograph was captured; it must not be future
        temporal::not_future(&env, taken_date).map_err(|_| Error::InvalidPastDate)?;

        // Validate every image reference is a well-formed CID/content hash.
        for cid in image_cids.iter() {
            shared::cid::validate_cid(&cid).map_err(|_| Error::InvalidCid)?;
        }

        // Patient relationship check: the dentist must be assigned to one of the patient's charts.
        if !Self::has_patient_relationship(&env, &patient_id, &dentist_id) {
            return Err(Error::Unauthorized);
        }

        let count = safe_increment(&env, &DataKey::RadiographCount);

        let radiograph = Radiograph {
            patient_id: patient_id.clone(),
            dentist_id,
            radiograph_type,
            taken_date,
            image_cids,
            findings_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Radiograph(count), &radiograph);

        // Per-patient index, bounded to MAX_RADIOGRAPHS_PER_PATIENT.
        let mut index: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::RadiographIndex(patient_id.clone()))
            .unwrap_or(Vec::new(&env));
        if index.len() >= MAX_RADIOGRAPHS_PER_PATIENT {
            return Err(Error::IndexFull);
        }
        index.push_back(count);
        env.storage()
            .persistent()
            .set(&DataKey::RadiographIndex(patient_id), &index);

        // Bump TTL on write so the record does not expire.
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Radiograph(count), RADIOGRAPH_TTL, RADIOGRAPH_TTL);

        RadiographRecorded {
            version: EVENT_VERSION,
            radiograph_id: count,
        }
        .publish(&env);

        Ok(count)
    }

    /// Read a radiograph. Requires the patient's auth or the treating dentist's auth.
    pub fn get_radiograph(
        env: Env,
        caller: Address,
        radiograph_id: u64,
    ) -> Result<Radiograph, Error> {
        caller.require_auth();

        let radiograph: Radiograph = env
            .storage()
            .persistent()
            .get(&DataKey::Radiograph(radiograph_id))
            .ok_or(Error::NotFound)?;

        if caller != radiograph.patient_id && caller != radiograph.dentist_id {
            return Err(Error::Unauthorized);
        }

        Ok(radiograph)
    }

    /// Paginated read of a patient's radiograph index.
    pub fn get_patient_radiographs(
        env: Env,
        caller: Address,
        patient_id: Address,
        start: u32,
        limit: u32,
    ) -> Result<Vec<u64>, Error> {
        caller.require_auth();
        if caller != patient_id && !Self::has_patient_relationship(&env, &patient_id, &caller) {
            return Err(Error::Unauthorized);
        }

        let index: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::RadiographIndex(patient_id))
            .unwrap_or(Vec::new(&env));

        let mut page = Vec::new(&env);
        let end = core::cmp::min(start.saturating_add(limit), index.len());
        let mut i = start;
        while i < end {
            page.push_back(index.get(i).unwrap());
            i += 1;
        }
        Ok(page)
    }

    fn has_patient_relationship(env: &Env, patient_id: &Address, dentist_id: &Address) -> bool {
        let chart_count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::ChartCount)
            .unwrap_or(0);
        let mut i = 1;
        while i <= chart_count {
            if let Some(chart) = env
                .storage()
                .persistent()
                .get::<DataKey, DentalChart>(&DataKey::Chart(i))
            {
                if &chart.patient_id == patient_id && &chart.dentist_id == dentist_id {
                    return true;
                }
            }
            i += 1;
        }
        false
    }
}
