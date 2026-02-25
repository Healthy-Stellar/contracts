#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, BytesN, Env, String, Symbol, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RehabGoal {
    pub goal_id: u64,
    pub goal_type: Symbol,
    pub goal_description: String,
    pub target_date: u64,
    pub measurement_method: String,
    pub achieved: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TherapyIntervention {
    pub intervention_type: Symbol,
    pub description: String,
    pub sets: Option<u32>,
    pub reps: Option<u32>,
    pub duration: Option<u32>,
    pub resistance: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PTEvaluation {
    pub evaluation_id: u64,
    pub patient_id: Address,
    pub therapist_id: Address,
    pub evaluation_date: u64,
    pub diagnosis: String,
    pub chief_complaint: String,
    pub functional_limitations: Vec<String>,
    pub prior_level_of_function: String,
    pub evaluation_findings_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ROMAssessment {
    pub joint: String,
    pub movement: String,
    pub degrees: u32,
    pub pain_level: Option<u32>,
    pub limitation: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrengthAssessment {
    pub muscle_group: String,
    pub manual_muscle_test_grade: String,
    pub side: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BalanceMobilityAssessment {
    pub test_type: Symbol,
    pub score: u32,
    pub fall_risk: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RehabTreatmentPlan {
    pub plan_id: u64,
    pub evaluation_id: u64,
    pub therapist_id: Address,
    pub stg_goals: Vec<RehabGoal>,
    pub ltg_goals: Vec<RehabGoal>,
    pub interventions: Vec<TherapyIntervention>,
    pub frequency: String,
    pub duration_weeks: u32,
    pub prognosis: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TherapySession {
    pub session_date: u64,
    pub interventions_performed: Vec<TherapyIntervention>,
    pub session_duration_minutes: u32,
    pub patient_response: String,
    pub homework_assigned: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PainMeasurement {
    pub measurement_date: u64,
    pub pain_scale_type: Symbol,
    pub pain_score: u32,
    pub location: String,
    pub quality: Vec<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionalOutcome {
    pub measurement_date: u64,
    pub outcome_tool: Symbol,
    pub score: u32,
    pub minimal_detectable_change: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TherapyAuthorization {
    pub auth_id: u64,
    pub treatment_plan_id: u64,
    pub requested_visits: u32,
    pub justification_hash: BytesN<32>,
    pub status: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressNote {
    pub note_date: u64,
    pub subjective: String,
    pub objective_findings: Vec<String>,
    pub assessment: String,
    pub plan_modifications: Vec<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DischargeRecord {
    pub discharge_date: u64,
    pub discharge_reason: Symbol,
    pub goals_met: Vec<u64>,
    pub final_outcomes_hash: BytesN<32>,
    pub home_exercise_program_hash: BytesN<32>,
}

#[contracttype]
pub enum DataKey {
    EvaluationCounter,
    Evaluation(u64),
    ROMAssessments(u64),
    StrengthAssessments(u64),
    BalanceMobilityAssessments(u64),
    TreatmentPlanCounter,
    TreatmentPlan(u64),
    TherapySessions(u64),
    PainMeasurements(u64),
    FunctionalOutcomes(u64),
    AuthorizationCounter,
    Authorization(u64),
    ProgressNotes(u64),
    Discharge(u64),
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    Unauthorized = 2,
    InvalidInput = 3,
    AlreadyExists = 4,
}

impl From<Error> for soroban_sdk::Error {
    fn from(e: Error) -> Self {
        soroban_sdk::Error::from_contract_error(e as u32)
    }
}

impl From<&Error> for soroban_sdk::Error {
    fn from(e: &Error) -> Self {
        soroban_sdk::Error::from_contract_error(*e as u32)
    }
}

impl TryFrom<soroban_sdk::Error> for Error {
    type Error = soroban_sdk::Error;
    
    fn try_from(value: soroban_sdk::Error) -> Result<Self, Self::Error> {
        Err(value)
    }
}

#[contract]
pub struct RehabilitationServicesContract;

#[contractimpl]
impl RehabilitationServicesContract {
    pub fn conduct_pt_evaluation(
        env: Env,
        patient_id: Address,
        therapist_id: Address,
        evaluation_date: u64,
        diagnosis: String,
        chief_complaint: String,
        functional_limitations: Vec<String>,
        prior_level_of_function: String,
        evaluation_findings_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        therapist_id.require_auth();

        let eval_id = env
            .storage()
            .instance()
            .get(&DataKey::EvaluationCounter)
            .unwrap_or(0u64)
            + 1;

        let evaluation = PTEvaluation {
            evaluation_id: eval_id,
            patient_id,
            therapist_id,
            evaluation_date,
            diagnosis,
            chief_complaint,
            functional_limitations,
            prior_level_of_function,
            evaluation_findings_hash,
        };

        env.storage()
            .instance()
            .set(&DataKey::Evaluation(eval_id), &evaluation);
        env.storage()
            .instance()
            .set(&DataKey::EvaluationCounter, &eval_id);

        Ok(eval_id)
    }

    pub fn assess_range_of_motion(
        env: Env,
        evaluation_id: u64,
        joint: String,
        movement: String,
        degrees: u32,
        pain_level: Option<u32>,
        limitation: Option<String>,
    ) -> Result<(), Error> {
        let eval: PTEvaluation = env
            .storage()
            .instance()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;

        eval.therapist_id.require_auth();

        let assessment = ROMAssessment {
            joint,
            movement,
            degrees,
            pain_level,
            limitation,
        };

        let mut assessments: Vec<ROMAssessment> = env
            .storage()
            .instance()
            .get(&DataKey::ROMAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env));

        assessments.push_back(assessment);
        env.storage()
            .instance()
            .set(&DataKey::ROMAssessments(evaluation_id), &assessments);

        Ok(())
    }

    pub fn assess_strength(
        env: Env,
        evaluation_id: u64,
        muscle_group: String,
        manual_muscle_test_grade: String,
        side: Symbol,
    ) -> Result<(), Error> {
        let eval: PTEvaluation = env
            .storage()
            .instance()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;

        eval.therapist_id.require_auth();

        let assessment = StrengthAssessment {
            muscle_group,
            manual_muscle_test_grade,
            side,
        };

        let mut assessments: Vec<StrengthAssessment> = env
            .storage()
            .instance()
            .get(&DataKey::StrengthAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env));

        assessments.push_back(assessment);
        env.storage()
            .instance()
            .set(&DataKey::StrengthAssessments(evaluation_id), &assessments);

        Ok(())
    }

    pub fn assess_balance_mobility(
        env: Env,
        evaluation_id: u64,
        test_type: Symbol,
        score: u32,
        fall_risk: Symbol,
    ) -> Result<(), Error> {
        let eval: PTEvaluation = env
            .storage()
            .instance()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;

        eval.therapist_id.require_auth();

        let assessment = BalanceMobilityAssessment {
            test_type,
            score,
            fall_risk,
        };

        let mut assessments: Vec<BalanceMobilityAssessment> = env
            .storage()
            .instance()
            .get(&DataKey::BalanceMobilityAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env));

        assessments.push_back(assessment);
        env.storage()
            .instance()
            .set(
                &DataKey::BalanceMobilityAssessments(evaluation_id),
                &assessments,
            );

        Ok(())
    }

    pub fn create_rehab_treatment_plan(
        env: Env,
        evaluation_id: u64,
        therapist_id: Address,
        stg_goals: Vec<RehabGoal>,
        ltg_goals: Vec<RehabGoal>,
        interventions: Vec<TherapyIntervention>,
        frequency: String,
        duration_weeks: u32,
        prognosis: Symbol,
    ) -> Result<u64, Error> {
        therapist_id.require_auth();

        let _eval: PTEvaluation = env
            .storage()
            .instance()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;

        let plan_id = env
            .storage()
            .instance()
            .get(&DataKey::TreatmentPlanCounter)
            .unwrap_or(0u64)
            + 1;

        let plan = RehabTreatmentPlan {
            plan_id,
            evaluation_id,
            therapist_id,
            stg_goals,
            ltg_goals,
            interventions,
            frequency,
            duration_weeks,
            prognosis,
        };

        env.storage()
            .instance()
            .set(&DataKey::TreatmentPlan(plan_id), &plan);
        env.storage()
            .instance()
            .set(&DataKey::TreatmentPlanCounter, &plan_id);

        Ok(plan_id)
    }

    pub fn document_therapy_session(
        env: Env,
        treatment_plan_id: u64,
        session_date: u64,
        interventions_performed: Vec<TherapyIntervention>,
        session_duration_minutes: u32,
        patient_response: String,
        homework_assigned: Option<String>,
    ) -> Result<(), Error> {
        let plan: RehabTreatmentPlan = env
            .storage()
            .instance()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();

        let session = TherapySession {
            session_date,
            interventions_performed,
            session_duration_minutes,
            patient_response,
            homework_assigned,
        };

        let mut sessions: Vec<TherapySession> = env
            .storage()
            .instance()
            .get(&DataKey::TherapySessions(treatment_plan_id))
            .unwrap_or(Vec::new(&env));

        sessions.push_back(session);
        env.storage()
            .instance()
            .set(&DataKey::TherapySessions(treatment_plan_id), &sessions);

        Ok(())
    }

    pub fn track_pain_level(
        env: Env,
        treatment_plan_id: u64,
        measurement_date: u64,
        pain_scale_type: Symbol,
        pain_score: u32,
        location: String,
        quality: Vec<String>,
    ) -> Result<(), Error> {
        let plan: RehabTreatmentPlan = env
            .storage()
            .instance()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();

        let measurement = PainMeasurement {
            measurement_date,
            pain_scale_type,
            pain_score,
            location,
            quality,
        };

        let mut measurements: Vec<PainMeasurement> = env
            .storage()
            .instance()
            .get(&DataKey::PainMeasurements(treatment_plan_id))
            .unwrap_or(Vec::new(&env));

        measurements.push_back(measurement);
        env.storage()
            .instance()
            .set(&DataKey::PainMeasurements(treatment_plan_id), &measurements);

        Ok(())
    }

    pub fn measure_functional_outcome(
        env: Env,
        treatment_plan_id: u64,
        measurement_date: u64,
        outcome_tool: Symbol,
        score: u32,
        minimal_detectable_change: bool,
    ) -> Result<(), Error> {
        let plan: RehabTreatmentPlan = env
            .storage()
            .instance()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();

        let outcome = FunctionalOutcome {
            measurement_date,
            outcome_tool,
            score,
            minimal_detectable_change,
        };

        let mut outcomes: Vec<FunctionalOutcome> = env
            .storage()
            .instance()
            .get(&DataKey::FunctionalOutcomes(treatment_plan_id))
            .unwrap_or(Vec::new(&env));

        outcomes.push_back(outcome);
        env.storage()
            .instance()
            .set(&DataKey::FunctionalOutcomes(treatment_plan_id), &outcomes);

        Ok(())
    }

    pub fn request_therapy_authorization(
        env: Env,
        treatment_plan_id: u64,
        requested_visits: u32,
        justification_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        let plan: RehabTreatmentPlan = env
            .storage()
            .instance()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();

        let auth_id = env
            .storage()
            .instance()
            .get(&DataKey::AuthorizationCounter)
            .unwrap_or(0u64)
            + 1;

        let authorization = TherapyAuthorization {
            auth_id,
            treatment_plan_id,
            requested_visits,
            justification_hash,
            status: Symbol::new(&env, "pending"),
        };

        env.storage()
            .instance()
            .set(&DataKey::Authorization(auth_id), &authorization);
        env.storage()
            .instance()
            .set(&DataKey::AuthorizationCounter, &auth_id);

        Ok(auth_id)
    }

    pub fn document_progress_note(
        env: Env,
        treatment_plan_id: u64,
        note_date: u64,
        subjective: String,
        objective_findings: Vec<String>,
        assessment: String,
        plan_modifications: Vec<String>,
    ) -> Result<(), Error> {
        let plan: RehabTreatmentPlan = env
            .storage()
            .instance()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();

        let note = ProgressNote {
            note_date,
            subjective,
            objective_findings,
            assessment,
            plan_modifications,
        };

        let mut notes: Vec<ProgressNote> = env
            .storage()
            .instance()
            .get(&DataKey::ProgressNotes(treatment_plan_id))
            .unwrap_or(Vec::new(&env));

        notes.push_back(note);
        env.storage()
            .instance()
            .set(&DataKey::ProgressNotes(treatment_plan_id), &notes);

        Ok(())
    }

    pub fn discharge_from_therapy(
        env: Env,
        treatment_plan_id: u64,
        discharge_date: u64,
        discharge_reason: Symbol,
        goals_met: Vec<u64>,
        final_outcomes_hash: BytesN<32>,
        home_exercise_program_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let plan: RehabTreatmentPlan = env
            .storage()
            .instance()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();

        let discharge = DischargeRecord {
            discharge_date,
            discharge_reason,
            goals_met,
            final_outcomes_hash,
            home_exercise_program_hash,
        };

        env.storage()
            .instance()
            .set(&DataKey::Discharge(treatment_plan_id), &discharge);

        Ok(())
    }

    // Query functions
    pub fn get_evaluation(env: Env, evaluation_id: u64) -> Result<PTEvaluation, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)
    }

    pub fn get_treatment_plan(env: Env, plan_id: u64) -> Result<RehabTreatmentPlan, Error> {
        env.storage()
            .instance()
            .get(&DataKey::TreatmentPlan(plan_id))
            .ok_or(Error::NotFound)
    }

    pub fn get_rom_assessments(env: Env, evaluation_id: u64) -> Vec<ROMAssessment> {
        env.storage()
            .instance()
            .get(&DataKey::ROMAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_strength_assessments(env: Env, evaluation_id: u64) -> Vec<StrengthAssessment> {
        env.storage()
            .instance()
            .get(&DataKey::StrengthAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_therapy_sessions(env: Env, treatment_plan_id: u64) -> Vec<TherapySession> {
        env.storage()
            .instance()
            .get(&DataKey::TherapySessions(treatment_plan_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_pain_measurements(env: Env, treatment_plan_id: u64) -> Vec<PainMeasurement> {
        env.storage()
            .instance()
            .get(&DataKey::PainMeasurements(treatment_plan_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_functional_outcomes(env: Env, treatment_plan_id: u64) -> Vec<FunctionalOutcome> {
        env.storage()
            .instance()
            .get(&DataKey::FunctionalOutcomes(treatment_plan_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_progress_notes(env: Env, treatment_plan_id: u64) -> Vec<ProgressNote> {
        env.storage()
            .instance()
            .get(&DataKey::ProgressNotes(treatment_plan_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_discharge_record(env: Env, treatment_plan_id: u64) -> Result<DischargeRecord, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Discharge(treatment_plan_id))
            .ok_or(Error::NotFound)
    }

    pub fn get_balance_mobility_assessments(env: Env, evaluation_id: u64) -> Vec<BalanceMobilityAssessment> {
        env.storage()
            .instance()
            .get(&DataKey::BalanceMobilityAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env))
    }
}

#[cfg(test)]
mod test;
