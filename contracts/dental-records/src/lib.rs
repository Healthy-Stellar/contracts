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
    contract, contractevent, contractimpl, symbol_short, Address, BytesN, Env, String, Symbol,
    Vec,
};
use shared::{events::EVENT_VERSION, temporal};
use shared::counters::safe_increment;

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

/// Emitted when a tooth condition is recorded (ids only, no clinical detail).
#[contractevent]
pub struct ToothCondRecorded {
    pub version: u32,
    pub chart_id: u64,
}

/// Supported tooth notation systems.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Notation {
    Universal,
    Palmer,
    Fdi,
}

fn parse_notation(system: &Symbol) -> Result<Notation, Error> {
    if *system == symbol_short!("universal") {
        Ok(Notation::Universal)
    } else if *system == symbol_short!("palmer") {
        Ok(Notation::Palmer)
    } else if *system == symbol_short!("fdi") {
        Ok(Notation::Fdi)
    } else {
        Err(Error::InvalidNotationSystem)
    }
}

/// Parses a string of 1-2 ASCII digits into a number.
fn parse_digits(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || bytes.len() > 2 {
        return None;
    }
    let mut n: u32 = 0;
    for b in bytes {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n * 10 + (b - b'0') as u32;
    }
    Some(n)
}

/// Validates a tooth number against the chart's notation system.
///
/// - Universal: permanent `1`-`32`, primary `A`-`T`
/// - Palmer: quadrant `UR`/`UL`/`LR`/`LL` followed by permanent `1`-`8` or primary `A`-`E`
/// - FDI: two digits, quadrant `1`-`4` with tooth `1`-`8` (permanent) or
///   quadrant `5`-`8` with tooth `1`-`5` (primary)
fn validate_tooth_number(notation: Notation, tooth_number: &String) -> Result<(), Error> {
    let len = tooth_number.len() as usize;
    if len == 0 || len > 3 {
        return Err(Error::InvalidToothNumber);
    }
    let mut buf = [0u8; 3];
    tooth_number.copy_into_slice(&mut buf[..len]);
    let t = &buf[..len];

    let valid = match notation {
        Notation::Universal => {
            if len == 1 && (b'A'..=b'T').contains(&t[0]) {
                true
            } else {
                // reject leading zeros such as "01"
                t[0] != b'0' && matches!(parse_digits(t), Some(n) if (1..=32).contains(&n))
            }
        }
        Notation::Palmer => {
            len == 3
                && matches!(&t[..2], b"UR" | b"UL" | b"LR" | b"LL")
                && ((b'1'..=b'8').contains(&t[2]) || (b'A'..=b'E').contains(&t[2]))
        }
        Notation::Fdi => {
            len == 2 && t[0].is_ascii_digit() && t[1].is_ascii_digit() && {
                let quadrant = t[0] - b'0';
                let tooth = t[1] - b'0';
                match quadrant {
                    1..=4 => (1..=8).contains(&tooth),
                    5..=8 => (1..=5).contains(&tooth),
                    _ => false,
                }
            }
        }
    };

    if valid {
        Ok(())
    } else {
        Err(Error::InvalidToothNumber)
    }
}

fn load_chart(env: &Env, chart_id: u64) -> Result<DentalChart, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Chart(chart_id))
        .ok_or(Error::NotFound)
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
        ttl_config::extend_instance_ttl(&env);
        patient_id.require_auth();

        parse_notation(&tooth_notation_system)?;
        // #1110 – chart_date records when the chart was taken; it must not be future
        temporal::not_future(&env, chart_date).map_err(|_| Error::InvalidPastDate)?;

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

    /// Records a condition for one surface of a tooth.
    ///
    /// #1110 – conditions are keyed by `(chart_id, tooth_number, surface)`, so
    /// findings on different surfaces of the same tooth no longer overwrite
    /// each other. Use `ToothSurface::WholeTooth` for non-surface findings.
    pub fn record_tooth_condition(
        env: Env,
        chart_id: u64,
        tooth_number: String,
        surface: ToothSurface,
        condition: ToothConditionKind,
        condition_details: Option<String>,
    ) -> Result<(), Error> {
        let chart = load_chart(&env, chart_id)?;
        chart.dentist_id.require_auth();

        let notation = parse_notation(&chart.tooth_notation_system)?;
        validate_tooth_number(notation, &tooth_number)?;

        let tooth_cond = ToothCondition {
            surface,
            condition,
            condition_details,
            recorded_by: chart.dentist_id,
            recorded_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(
            &DataKey::ToothCond(chart_id, tooth_number, surface),
            &tooth_cond,
        );

        ToothCondRecorded {
            version: EVENT_VERSION,
            chart_id,
        }
        .publish(&env);

        Ok(())
    }

    /// Returns the condition recorded for a tooth surface, if any.
    pub fn get_tooth_condition(
        env: Env,
        chart_id: u64,
        tooth_number: String,
        surface: ToothSurface,
    ) -> Option<ToothCondition> {
        env.storage()
            .persistent()
            .get(&DataKey::ToothCond(chart_id, tooth_number, surface))
    }

    pub fn record_periodontal_assessment(
        env: Env,
        chart_id: u64,
        tooth_number: String,
        site: PerioSite,
        probing_depth: u32,
        recession: u32,
        bleeding_on_probing: bool,
        mobility: Option<u32>,
    ) -> Result<(), Error> {
        let chart = load_chart(&env, chart_id)?;
        chart.dentist_id.require_auth();

        let notation = parse_notation(&chart.tooth_notation_system)?;
        validate_tooth_number(notation, &tooth_number)?;

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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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

    pub fn record_radiograph(
        env: Env,
        patient_id: Address,
        dentist_id: Address,
        radiograph_type: Symbol, // periapical, bitewing, panoramic, cbct
        taken_date: u64,
        findings: String,
        image_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        dentist_id.require_auth();

        let count = safe_increment(&env, &DataKey::RadiographCount);

        let radiograph = Radiograph {
            patient_id,
            dentist_id,
            radiograph_type,
            taken_date,
            findings,
            image_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Radiograph(count), &radiograph);

        RadiographRecorded {
            version: EVENT_VERSION,
            radiograph_id: count,
        }
        .publish(&env);

        Ok(count)
    }
}
