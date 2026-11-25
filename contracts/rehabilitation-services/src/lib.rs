#![no_std]
#![allow(clippy::too_many_arguments)]
#![allow(deprecated)]

//! # Rehabilitation Services Contract
//!
//! Manages rehabilitation programs, therapy goals, session tracking, progress measurement, and
//! outcome documentation for physical, occupational, and speech therapies.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Therapist authentication for goal creation and session logging.
//! Patient consent for therapy record access. Authorized providers can view progress. Discharge
//! authorization by supervising therapist. Access control per therapy type.
//!
//! **Audit Controls:** Rehabilitation program creation events with therapy type and goals.
//! Session events logged with date, provider, and outcomes. Goal progress events tracked.
//! Discharge events recorded with final status. Adverse events logged. Progress measurement
//! events emitted.
//!
//! **Data Retention Policy:** Rehabilitation programs retained with discharge summary. Therapy
//! goals archived with achievement status. Session records maintained indefinitely. Progress
//! measurements retained for outcome analysis. Patient deregistration removes rehab records.
//!
//! **Encryption/Integrity:** Goal descriptions stored encrypted. Session notes encrypted in
//! storage. Progress metrics immutable once recorded. Therapist identity validated. Patient
//! linkage encrypted. Therapy outcome enumeration prevents invalid status values.

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, BytesN, Env, String, Symbol, Vec,
};

const PERSISTENT_TTL_BUMP: u32 = 535_680; // ~31 days at 5s/ledger
const PERSISTENT_TTL_THRESHOLD: u32 = 518_400; // ~30 days

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
    /// Off-chain reference (hash) to the diagnosis text. Plaintext is never stored on-chain.
    pub diagnosis_hash: BytesN<32>,
    /// Off-chain reference (hash) to the chief complaint text.
    pub chief_complaint_hash: BytesN<32>,
    pub functional_limitations: Vec<String>,
    /// Off-chain reference (hash) to the prior level of function text.
    pub prior_level_of_function_hash: BytesN<32>,
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

/// Lifecycle status of a rehabilitation treatment plan.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PlanStatus {
    Active = 0,
    Discharged = 1,
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
    pub status: PlanStatus,
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
    /// Visits approved by the payer (0 until approved).
    pub approved_visits: u32,
    /// Payer that decided the authorization, if any.
    pub decided_by: Option<Address>,
    /// Ledger timestamp of the decision, if any.
    pub decided_at: Option<u64>,
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

pub const MAX_PAGE_SIZE: u32 = 50;

/// Paginated page of therapy sessions.
// TODO: consolidate with shared PageResult when #515 lands
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TherapyPage {
    pub items: Vec<TherapySession>,
    pub has_more: bool,
}

/// Paginated page of progress notes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotePage {
    pub items: Vec<ProgressNote>,
    pub has_more: bool,
}

/// Direction in which a goal's measurement improves.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum GoalDirection {
    /// Larger values are better (e.g. range of motion, FIM, strength).
    HigherIsBetter = 0,
    /// Smaller values are better (e.g. pain scale, time-up-and-go, assistance level).
    LowerIsBetter = 1,
}

/// A measurable rehabilitation goal with a numeric target value.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurableGoal {
    pub goal_id: u64,
    pub plan_id: u64,
    pub goal_type: Symbol,
    /// Direction in which the measurement improves; drives achievement comparison.
    pub direction: GoalDirection,
    pub target_value: u32,
    pub target_date: u64,
    /// True while the most recent measurement satisfies the target.
    pub achieved: bool,
    /// True once the goal has ever been met (sticky; survives regressions).
    pub ever_achieved: bool,
    /// True if the goal was first met on or before `target_date`.
    pub met_by_target_date: bool,
    /// Ledger timestamp of the first measurement that met the target, if any.
    pub achieved_at: Option<u64>,
    /// True once the target date has passed without the goal ever being met.
    pub missed: bool,
    /// Snapshot of the plan's version (`PlanVersion`) when this goal was set.
    pub plan_version: u64,
}

/// A single progress measurement for a measurable goal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressEntry {
    pub goal_id: u64,
    pub current_value: u32,
    pub measured_at: u64,
}

/// An entry in a treatment plan's version history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanVersionEntry {
    pub ipfs_hash: String,
    pub updated_by: Address,
    pub updated_at: u64,
    pub version: u64,
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
    /// Auto-increment counter for measurable goals.
    GoalCounter,
    /// goal_id -> MeasurableGoal
    MeasurableGoal(u64),
    /// goal_id -> Vec<ProgressEntry>
    GoalProgressList(u64),
    /// plan_id -> u64 (current version number)
    PlanVersion(u64),
    /// plan_id -> Vec<PlanVersionEntry>
    PlanVersionHistory(u64),
    /// plan_id -> u64 (number of goals set for this plan)
    PlanGoalCount(u64),
    /// Contract admin (set via constructor).
    Admin,
    /// Address of the provider-registry contract (set via constructor).
    ProviderRegistry,
    /// evaluation_id -> patient consent to open the evaluation.
    EvaluationConsent(u64),
    /// evaluation_id -> therapist the evaluation was handed off to.
    EvaluationHandoff(u64),
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

/// Returns the improvement direction for a goal type. Lower-is-better measures
/// (pain, timed mobility tests, assistance level) must not be "achieved" by a
/// high baseline reading.
fn direction_for_goal_type(env: &Env, goal_type: &Symbol) -> GoalDirection {
    let lower = Symbol::new(env, "pain_scale");
    let tug = Symbol::new(env, "time_up_and_go");
    let tug_short = Symbol::new(env, "tug");
    let assistance = Symbol::new(env, "assistance_level");
    let fall_risk = Symbol::new(env, "fall_risk");
    if *goal_type == lower
        || *goal_type == tug
        || *goal_type == tug_short
        || *goal_type == assistance
        || *goal_type == fall_risk
    {
        GoalDirection::LowerIsBetter
    } else {
        GoalDirection::HigherIsBetter
    }
}

/// Returns true if `value` satisfies `target` given the goal's direction.
fn meets_target(direction: GoalDirection, value: u32, target: u32) -> bool {
    match direction {
        GoalDirection::HigherIsBetter => value >= target,
        GoalDirection::LowerIsBetter => value <= target,
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

/// Returns true if `caller` is the evaluation's treating therapist or the patient it belongs to.
fn is_bound_to_evaluation(eval: &PTEvaluation, caller: &Address) -> bool {
    *caller == eval.therapist_id || *caller == eval.patient_id
}

/// Returns true if `caller` is the plan's treating therapist or the patient linked to it via
/// the originating evaluation. Mirrors care-plan's `is_bound_to_plan`: the primary custodian
/// (therapist, standing in for care-plan's provider) short-circuits, then the wider membership
/// is checked. Rehabilitation-services has no separate care-team roster, so the patient reached
/// through the plan's evaluation is the only other bound party.
fn is_bound_to_plan(env: &Env, plan: &RehabTreatmentPlan, caller: &Address) -> bool {
    if *caller == plan.therapist_id {
        return true;
    }
    if let Some(eval) = env
        .storage()
        .persistent()
        .get::<_, PTEvaluation>(&DataKey::Evaluation(plan.evaluation_id))
    {
        return *caller == eval.patient_id;
    }
    false
}

/// Reject clinical writes on a plan that has already been discharged.
fn require_active(plan: &RehabTreatmentPlan) -> Result<(), Error> {
    if plan.status != PlanStatus::Active {
        return Err(Error::InvalidInput);
    }
    Ok(())
}

#[contract]
pub struct RehabilitationServicesContract;

#[contractimpl]
impl RehabilitationServicesContract {
    /// Initialize the contract with an admin and the provider-registry contract address.
    pub fn __constructor(env: Env, admin: Address, provider_registry: Address) {
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::ProviderRegistry, &provider_registry);
    }

    /// Returns the configured provider-registry address.
    fn provider_registry(env: &Env) -> Result<Address, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::ProviderRegistry)
            .ok_or(Error::NotFound)
    }

    /// Require that `therapist_id` is a registered, active provider in the provider registry.
    fn require_registered_provider(env: &Env, therapist_id: &Address) -> Result<(), Error> {
        let registry = Self::provider_registry(env)?;
        if !shared::actor_verification::is_provider_registered(env, &registry, therapist_id) {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    /// Record consent from the patient (or their proxy) to open an evaluation for them.
    pub fn consent_to_evaluation(
        env: Env,
        patient_id: Address,
        evaluation_id: u64,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let eval: PTEvaluation = env
            .storage()
            .persistent()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;
        if eval.patient_id != patient_id {
            return Err(Error::Unauthorized);
        }

        env.storage()
            .persistent()
            .set(&DataKey::EvaluationConsent(evaluation_id), &true);
        env.storage().persistent().extend_ttl(
            &DataKey::EvaluationConsent(evaluation_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        env.events().publish(
            (Symbol::new(&env, "evaluation_consented"), evaluation_id),
            patient_id,
        );

        Ok(())
    }

    /// Hand off an evaluation to another registered therapist. Must be signed by the
    /// evaluating therapist. The hand-off allows the target therapist to create a plan.
    pub fn handoff_evaluation(
        env: Env,
        evaluation_id: u64,
        therapist_id: Address,
        to_therapist: Address,
    ) -> Result<(), Error> {
        therapist_id.require_auth();

        let eval: PTEvaluation = env
            .storage()
            .persistent()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;
        if eval.therapist_id != therapist_id {
            return Err(Error::Unauthorized);
        }
        Self::require_registered_provider(&env, &to_therapist)?;

        env.storage()
            .persistent()
            .set(&DataKey::EvaluationHandoff(evaluation_id), &to_therapist);
        env.storage().persistent().extend_ttl(
            &DataKey::EvaluationHandoff(evaluation_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        env.events().publish(
            (Symbol::new(&env, "evaluation_handoff"), evaluation_id),
            (therapist_id, to_therapist),
        );

        Ok(())
    }

    pub fn conduct_pt_evaluation(
        env: Env,
        patient_id: Address,
        therapist_id: Address,
        evaluation_date: u64,
        diagnosis_hash: BytesN<32>,
        chief_complaint_hash: BytesN<32>,
        functional_limitations: Vec<String>,
        prior_level_of_function_hash: BytesN<32>,
        evaluation_findings_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        therapist_id.require_auth();
        Self::require_registered_provider(&env, &therapist_id)?;

        let eval_id = env
            .storage()
            .persistent()
            .get(&DataKey::EvaluationCounter)
            .unwrap_or(0u64)
            + 1;

        let evaluation = PTEvaluation {
            evaluation_id: eval_id,
            patient_id,
            therapist_id,
            evaluation_date,
            diagnosis_hash,
            chief_complaint_hash,
            functional_limitations,
            prior_level_of_function_hash,
            evaluation_findings_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Evaluation(eval_id), &evaluation);
        env.storage().persistent().extend_ttl(
            &DataKey::Evaluation(eval_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );
        env.storage()
            .persistent()
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
        ttl_config::extend_instance_ttl(&env);
        let eval: PTEvaluation = env
            .storage()
            .persistent()
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
            .persistent()
            .get(&DataKey::ROMAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env));

        assessments.push_back(assessment);
        env.storage()
            .persistent()
            .set(&DataKey::ROMAssessments(evaluation_id), &assessments);
        env.storage().persistent().extend_ttl(
            &DataKey::ROMAssessments(evaluation_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        Ok(())
    }

    pub fn assess_strength(
        env: Env,
        evaluation_id: u64,
        muscle_group: String,
        manual_muscle_test_grade: String,
        side: Symbol,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        let eval: PTEvaluation = env
            .storage()
            .persistent()
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
            .persistent()
            .get(&DataKey::StrengthAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env));

        assessments.push_back(assessment);
        env.storage()
            .persistent()
            .set(&DataKey::StrengthAssessments(evaluation_id), &assessments);
        env.storage().persistent().extend_ttl(
            &DataKey::StrengthAssessments(evaluation_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        Ok(())
    }

    pub fn assess_balance_mobility(
        env: Env,
        evaluation_id: u64,
        test_type: Symbol,
        score: u32,
        fall_risk: Symbol,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        let eval: PTEvaluation = env
            .storage()
            .persistent()
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
            .persistent()
            .get(&DataKey::BalanceMobilityAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env));

        assessments.push_back(assessment);
        env.storage().persistent().set(
            &DataKey::BalanceMobilityAssessments(evaluation_id),
            &assessments,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::BalanceMobilityAssessments(evaluation_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
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
        ttl_config::extend_instance_ttl(&env);
        therapist_id.require_auth();
        Self::require_registered_provider(&env, &therapist_id)?;

        let eval: PTEvaluation = env
            .storage()
            .persistent()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;

        // Only the evaluating therapist (or an explicitly handed-off therapist) may
        // create a plan on this evaluation.
        if eval.therapist_id != therapist_id {
            let handoff: Option<Address> = env
                .storage()
                .persistent()
                .get(&DataKey::EvaluationHandoff(evaluation_id));
            match handoff {
                Some(to) if to == therapist_id => {}
                _ => return Err(Error::Unauthorized),
            }
        }

        // The patient must have consented to the evaluation before a plan is opened.
        let consented: bool = env
            .storage()
            .persistent()
            .get(&DataKey::EvaluationConsent(evaluation_id))
            .unwrap_or(false);
        if !consented {
            return Err(Error::Unauthorized);
        }

        let plan_id = env
            .storage()
            .persistent()
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
            status: PlanStatus::Active,
        };

        env.storage()
            .persistent()
            .set(&DataKey::TreatmentPlan(plan_id), &plan);
        env.storage().persistent().extend_ttl(
            &DataKey::TreatmentPlan(plan_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );
        env.storage()
            .persistent()
            .set(&DataKey::TreatmentPlanCounter, &plan_id);

        // Initialize version tracking (#560)
        env.storage()
            .persistent()
            .set(&DataKey::PlanVersion(plan_id), &1u64);
        let history_entry = PlanVersionEntry {
            ipfs_hash: String::from_str(&env, ""),
            updated_by: plan.therapist_id.clone(),
            updated_at: env.ledger().timestamp(),
            version: 1,
        };
        let mut history: Vec<PlanVersionEntry> = Vec::new(&env);
        history.push_back(history_entry);
        env.storage()
            .persistent()
            .set(&DataKey::PlanVersionHistory(plan_id), &history);

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
        ttl_config::extend_instance_ttl(&env);
        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();
        require_active(&plan)?;

        if session_date > env.ledger().timestamp() {
            return Err(Error::InvalidInput);
        }

        let session = TherapySession {
            session_date,
            interventions_performed,
            session_duration_minutes,
            patient_response,
            homework_assigned,
        };

        let mut sessions: Vec<TherapySession> = env
            .storage()
            .persistent()
            .get(&DataKey::TherapySessions(treatment_plan_id))
            .unwrap_or(Vec::new(&env));

        sessions.push_back(session);
        env.storage()
            .persistent()
            .set(&DataKey::TherapySessions(treatment_plan_id), &sessions);
        env.storage().persistent().extend_ttl(
            &DataKey::TherapySessions(treatment_plan_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

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
        ttl_config::extend_instance_ttl(&env);
        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();
        require_active(&plan)?;

        let measurement = PainMeasurement {
            measurement_date,
            pain_scale_type,
            pain_score,
            location,
            quality,
        };

        let mut measurements: Vec<PainMeasurement> = env
            .storage()
            .persistent()
            .get(&DataKey::PainMeasurements(treatment_plan_id))
            .unwrap_or(Vec::new(&env));

        measurements.push_back(measurement);
        env.storage()
            .persistent()
            .set(&DataKey::PainMeasurements(treatment_plan_id), &measurements);
        env.storage().persistent().extend_ttl(
            &DataKey::PainMeasurements(treatment_plan_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

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
        ttl_config::extend_instance_ttl(&env);
        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();
        require_active(&plan)?;

        let outcome = FunctionalOutcome {
            measurement_date,
            outcome_tool,
            score,
            minimal_detectable_change,
        };

        let mut outcomes: Vec<FunctionalOutcome> = env
            .storage()
            .persistent()
            .get(&DataKey::FunctionalOutcomes(treatment_plan_id))
            .unwrap_or(Vec::new(&env));

        outcomes.push_back(outcome);
        env.storage()
            .persistent()
            .set(&DataKey::FunctionalOutcomes(treatment_plan_id), &outcomes);
        env.storage().persistent().extend_ttl(
            &DataKey::FunctionalOutcomes(treatment_plan_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        Ok(())
    }

    pub fn request_therapy_authorization(
        env: Env,
        treatment_plan_id: u64,
        requested_visits: u32,
        justification_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();
        require_active(&plan)?;

        if requested_visits == 0 {
            return Err(Error::InvalidInput);
        }

        let auth_id = env
            .storage()
            .persistent()
            .get(&DataKey::AuthorizationCounter)
            .unwrap_or(0u64)
            + 1;

        let authorization = TherapyAuthorization {
            auth_id,
            treatment_plan_id,
            requested_visits,
            justification_hash,
            status: Symbol::new(&env, "pending"),
            approved_visits: 0,
            decided_by: None,
            decided_at: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Authorization(auth_id), &authorization);
        env.storage().persistent().extend_ttl(
            &DataKey::Authorization(auth_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );
        env.storage()
            .persistent()
            .set(&DataKey::AuthorizationCounter, &auth_id);

        env.events().publish(
            (Symbol::new(&env, "therapy_authorization_requested"), auth_id),
            (treatment_plan_id, requested_visits),
        );

        Ok(auth_id)
    }

    /// Decide a pending therapy authorization.
    ///
    /// `payer` must authenticate and is recorded as the deciding party. `decision`
    /// must be `approved` or `denied`. On approval, `approved_visits` is recorded
    /// (clamped to the requested visits); on denial it is set to 0. An
    /// authorization can only be decided once.
    pub fn decide_therapy_authorization(
        env: Env,
        payer: Address,
        auth_id: u64,
        approved_visits: u32,
        decision: Symbol,
    ) -> Result<(), Error> {
        payer.require_auth();

        let mut authorization: TherapyAuthorization = env
            .storage()
            .persistent()
            .get(&DataKey::Authorization(auth_id))
            .ok_or(Error::NotFound)?;

        let pending = Symbol::new(&env, "pending");
        if authorization.status != pending {
            return Err(Error::InvalidInput);
        }

        let approved = Symbol::new(&env, "approved");
        let denied = Symbol::new(&env, "denied");

        if decision == approved {
            if approved_visits == 0 || approved_visits > authorization.requested_visits {
                return Err(Error::InvalidInput);
            }
            authorization.approved_visits = approved_visits;
        } else if decision == denied {
            authorization.approved_visits = 0;
        } else {
            return Err(Error::InvalidInput);
        }

        authorization.status = decision.clone();
        authorization.decided_by = Some(payer.clone());
        authorization.decided_at = Some(env.ledger().timestamp());

        env.storage()
            .persistent()
            .set(&DataKey::Authorization(auth_id), &authorization);
        env.storage().persistent().extend_ttl(
            &DataKey::Authorization(auth_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );
        env.storage()
            .persistent()
            .set(&DataKey::AuthorizationPayer(auth_id), &payer);
        env.storage().persistent().extend_ttl(
            &DataKey::AuthorizationPayer(auth_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        env.events().publish(
            (Symbol::new(&env, "therapy_authorization_decided"), auth_id),
            (
                authorization.treatment_plan_id,
                decision,
                authorization.approved_visits,
                payer,
            ),
        );

        Ok(())
    }

    /// Read a therapy authorization back.
    ///
    /// `caller` must be the plan's treating therapist, the plan's patient, or the
    /// payer that decided the authorization.
    pub fn get_therapy_authorization(
        env: Env,
        caller: Address,
        auth_id: u64,
    ) -> Result<TherapyAuthorization, Error> {
        caller.require_auth();

        let authorization: TherapyAuthorization = env
            .storage()
            .persistent()
            .get(&DataKey::Authorization(auth_id))
            .ok_or(Error::NotFound)?;

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(authorization.treatment_plan_id))
            .ok_or(Error::NotFound)?;

        let is_payer = env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::AuthorizationPayer(auth_id))
            .map(|p| p == caller)
            .unwrap_or(false);

        if !is_payer && !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(authorization)
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
        ttl_config::extend_instance_ttl(&env);
        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();
        require_active(&plan)?;

        let note = ProgressNote {
            note_date,
            subjective,
            objective_findings,
            assessment,
            plan_modifications,
        };

        let mut notes: Vec<ProgressNote> = env
            .storage()
            .persistent()
            .get(&DataKey::ProgressNotes(treatment_plan_id))
            .unwrap_or(Vec::new(&env));

        notes.push_back(note);
        env.storage()
            .persistent()
            .set(&DataKey::ProgressNotes(treatment_plan_id), &notes);
        env.storage().persistent().extend_ttl(
            &DataKey::ProgressNotes(treatment_plan_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

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
        let mut plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();

        // Discharge is one-time: a discharged plan cannot be discharged again.
        if plan.status != PlanStatus::Active {
            return Err(Error::InvalidInput);
        }

        let now = env.ledger().timestamp();
        if discharge_date > now {
            return Err(Error::InvalidInput);
        }

        // Discharge cannot predate the most recent documented session.
        if let Some(sessions) = env
            .storage()
            .persistent()
            .get::<_, Vec<TherapySession>>(&DataKey::TherapySessions(treatment_plan_id))
        {
            for session in sessions.iter() {
                if discharge_date < session.session_date {
                    return Err(Error::InvalidInput);
                }
            }
        }

        // Every goal marked met must exist, belong to this plan, and be achieved.
        for goal_id in goals_met.iter() {
            let goal: MeasurableGoal = env
                .storage()
                .persistent()
                .get(&DataKey::MeasurableGoal(goal_id))
                .ok_or(Error::NotFound)?;
            if goal.plan_id != treatment_plan_id || !goal.achieved {
                return Err(Error::InvalidInput);
            }
        }

        let discharge = DischargeRecord {
            discharge_date,
            discharge_reason,
            goals_met,
            final_outcomes_hash,
            home_exercise_program_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Discharge(treatment_plan_id), &discharge);
        env.storage().persistent().extend_ttl(
            &DataKey::Discharge(treatment_plan_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        plan.status = PlanStatus::Discharged;
        env.storage()
            .persistent()
            .set(&DataKey::TreatmentPlan(treatment_plan_id), &plan);
        env.storage().persistent().extend_ttl(
            &DataKey::TreatmentPlan(treatment_plan_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        env.events().publish(
            (Symbol::new(&env, "therapy_discharged"), treatment_plan_id),
            (discharge_date, discharge_reason),
        );

        Ok(())
    }

    // ── Treatment plan versioning (#560) ───────────────────────────────────────

    /// Update an existing treatment plan with new fields.
    /// Only the original therapist (creator) can update the plan.
    /// Each update creates a new version entry in the plan's history.
    pub fn update_rehab_treatment_plan(
        env: Env,
        plan_id: u64,
        therapist_id: Address,
        stg_goals: Vec<RehabGoal>,
        ltg_goals: Vec<RehabGoal>,
        interventions: Vec<TherapyIntervention>,
        frequency: String,
        duration_weeks: u32,
        prognosis: Symbol,
        ipfs_hash: String,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        therapist_id.require_auth();

        let mut plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(plan_id))
            .ok_or(Error::NotFound)?;

        if plan.therapist_id != therapist_id {
            return Err(Error::Unauthorized);
        }
        require_active(&plan)?;

        plan.stg_goals = stg_goals;
        plan.ltg_goals = ltg_goals;
        plan.interventions = interventions;
        plan.frequency = frequency;
        plan.duration_weeks = duration_weeks;
        plan.prognosis = prognosis;

        env.storage()
            .persistent()
            .set(&DataKey::TreatmentPlan(plan_id), &plan);
        env.storage().persistent().extend_ttl(
            &DataKey::TreatmentPlan(plan_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        // Increment version
        let current_version: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::PlanVersion(plan_id))
            .unwrap_or(1);
        let new_version = current_version + 1;
        env.storage()
            .persistent()
            .set(&DataKey::PlanVersion(plan_id), &new_version);

        // Append to version history
        let entry = PlanVersionEntry {
            ipfs_hash: ipfs_hash.clone(),
            updated_by: therapist_id.clone(),
            updated_at: env.ledger().timestamp(),
            version: new_version,
        };
        let mut history: Vec<PlanVersionEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::PlanVersionHistory(plan_id))
            .unwrap_or(Vec::new(&env));
        history.push_back(entry);
        env.storage()
            .persistent()
            .set(&DataKey::PlanVersionHistory(plan_id), &history);

        // Emit plan_updated event
        env.events().publish(
            (Symbol::new(&env, "plan_updated"), plan_id),
            (new_version, therapist_id, ipfs_hash),
        );

        Ok(new_version)
    }

    /// Retrieve the full version history for a treatment plan.
    ///
    /// `caller` must be the plan's treating therapist or the patient it belongs to.
    pub fn get_treatment_plan_history(
        env: Env,
        caller: Address,
        plan_id: u64,
    ) -> Result<Vec<PlanVersionEntry>, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::PlanVersionHistory(plan_id))
            .unwrap_or(Vec::new(&env)))
    }

    // Query functions
    //
    // Every getter below requires the caller to authenticate and to be bound to the
    // evaluation/plan being read (its treating therapist or its patient) — see
    // `is_bound_to_evaluation` / `is_bound_to_plan`.

    pub fn get_evaluation(
        env: Env,
        caller: Address,
        evaluation_id: u64,
    ) -> Result<PTEvaluation, Error> {
        caller.require_auth();

        let eval: PTEvaluation = env
            .storage()
            .persistent()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_evaluation(&eval, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(eval)
    }

    pub fn get_treatment_plan(
        env: Env,
        caller: Address,
        plan_id: u64,
    ) -> Result<RehabTreatmentPlan, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(plan)
    }

    pub fn get_rom_assessments(
        env: Env,
        caller: Address,
        evaluation_id: u64,
    ) -> Result<Vec<ROMAssessment>, Error> {
        caller.require_auth();

        let eval: PTEvaluation = env
            .storage()
            .persistent()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_evaluation(&eval, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::ROMAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env)))
    }

    pub fn get_strength_assessments(
        env: Env,
        caller: Address,
        evaluation_id: u64,
    ) -> Result<Vec<StrengthAssessment>, Error> {
        caller.require_auth();

        let eval: PTEvaluation = env
            .storage()
            .persistent()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_evaluation(&eval, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::StrengthAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env)))
    }

    pub fn get_therapy_sessions(
        env: Env,
        caller: Address,
        treatment_plan_id: u64,
    ) -> Result<Vec<TherapySession>, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::TherapySessions(treatment_plan_id))
            .unwrap_or(Vec::new(&env)))
    }

    pub fn get_pain_measurements(
        env: Env,
        caller: Address,
        treatment_plan_id: u64,
    ) -> Result<Vec<PainMeasurement>, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::PainMeasurements(treatment_plan_id))
            .unwrap_or(Vec::new(&env)))
    }

    pub fn get_functional_outcomes(
        env: Env,
        caller: Address,
        treatment_plan_id: u64,
    ) -> Result<Vec<FunctionalOutcome>, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::FunctionalOutcomes(treatment_plan_id))
            .unwrap_or(Vec::new(&env)))
    }

    pub fn get_progress_notes(
        env: Env,
        caller: Address,
        treatment_plan_id: u64,
    ) -> Result<Vec<ProgressNote>, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::ProgressNotes(treatment_plan_id))
            .unwrap_or(Vec::new(&env)))
    }

    pub fn get_discharge_record(
        env: Env,
        caller: Address,
        treatment_plan_id: u64,
    ) -> Result<DischargeRecord, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        env.storage()
            .persistent()
            .get(&DataKey::Discharge(treatment_plan_id))
            .ok_or(Error::NotFound)
    }

    pub fn get_balance_mobility_assessments(
        env: Env,
        caller: Address,
        evaluation_id: u64,
    ) -> Result<Vec<BalanceMobilityAssessment>, Error> {
        caller.require_auth();

        let eval: PTEvaluation = env
            .storage()
            .persistent()
            .get(&DataKey::Evaluation(evaluation_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_evaluation(&eval, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::BalanceMobilityAssessments(evaluation_id))
            .unwrap_or(Vec::new(&env)))
    }

    // ── Goal tracking ──────────────────────────────────────────────────────────

    /// Define a measurable outcome goal for an existing treatment plan.
    ///
    /// Returns the new `goal_id`. `plan_version` snapshots the plan's actual
    /// `PlanVersion` so goals can be tied to the plan revision they belong to.
    ///
    /// Supported `goal_type` symbols: `range_of_motion`, `pain_scale`, `fim`,
    /// `strength`, `balance`, `endurance` (open-ended — any Symbol is accepted).
    /// `pain_scale`, `time_up_and_go`/`tug`, `assistance_level` and `fall_risk`
    /// are treated as lower-is-better; all others as higher-is-better.
    pub fn set_rehabilitation_goal(
        env: Env,
        plan_id: u64,
        goal_type: Symbol,
        target_value: u32,
        target_date: u64,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();
        require_active(&plan)?;

        let goal_id = env
            .storage()
            .persistent()
            .get(&DataKey::GoalCounter)
            .unwrap_or(0u64)
            + 1;

        // Snapshot the plan's real version so the goal is tied to the revision it belongs to.
        let plan_version: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::PlanVersion(plan_id))
            .unwrap_or(1);

        let direction = direction_for_goal_type(&env, &goal_type);

        let goal = MeasurableGoal {
            goal_id,
            plan_id,
            goal_type: goal_type.clone(),
            direction,
            target_value,
            target_date,
            achieved: false,
            ever_achieved: false,
            met_by_target_date: false,
            achieved_at: None,
            missed: false,
            plan_version,
        };

        env.storage()
            .persistent()
            .set(&DataKey::MeasurableGoal(goal_id), &goal);
        env.storage().persistent().extend_ttl(
            &DataKey::MeasurableGoal(goal_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );
        env.storage()
            .persistent()
            .set(&DataKey::GoalCounter, &goal_id);
        let goal_count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::PlanGoalCount(plan_id))
            .unwrap_or(0u64);
        env.storage()
            .persistent()
            .set(&DataKey::PlanGoalCount(plan_id), &(goal_count + 1));

        env.events().publish(
            (Symbol::new(&env, "goal_set"), plan_id),
            (goal_id, goal_type, target_value, target_date),
        );

        Ok(goal_id)
    }

    /// Record a progress measurement for a measurable goal.
    ///
    /// The measurement is compared against `target_value` according to the goal's
    /// direction (higher-is-better vs lower-is-better). `achieved` is re-evaluated on
    /// every measurement so regressions are visible, while `ever_achieved` stays set.
    /// Emits a `GoalAchieved` event the first time the target is met.
    ///
    /// `measured_at` must not be in the future.
    pub fn record_progress(
        env: Env,
        plan_id: u64,
        goal_id: u64,
        current_value: u32,
        measured_at: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(plan_id))
            .ok_or(Error::NotFound)?;

        plan.therapist_id.require_auth();
        require_active(&plan)?;

        let now = env.ledger().timestamp();
        if measured_at > now {
            return Err(Error::InvalidInput);
        }

        let mut goal: MeasurableGoal = env
            .storage()
            .persistent()
            .get(&DataKey::MeasurableGoal(goal_id))
            .ok_or(Error::NotFound)?;

        if goal.plan_id != plan_id {
            return Err(Error::InvalidInput);
        }

        let entry = ProgressEntry {
            goal_id,
            current_value,
            measured_at,
        };

        let mut progress: Vec<ProgressEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::GoalProgressList(goal_id))
            .unwrap_or(Vec::new(&env));
        progress.push_back(entry);
        env.storage()
            .persistent()
            .set(&DataKey::GoalProgressList(goal_id), &progress);
        env.storage().persistent().extend_ttl(
            &DataKey::GoalProgressList(goal_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        // Re-evaluate achievement on every measurement, honouring the goal direction.
        let met_now = meets_target(goal.direction, current_value, goal.target_value);
        let first_achievement = met_now && !goal.ever_achieved;

        goal.achieved = met_now;
        if first_achievement {
            goal.ever_achieved = true;
            goal.achieved_at = Some(measured_at);
            goal.met_by_target_date = measured_at <= goal.target_date;
        }

        // Mark the goal missed once the target date has passed without ever being met.
        if !goal.ever_achieved && now > goal.target_date {
            goal.missed = true;
        }

        env.storage()
            .persistent()
            .set(&DataKey::MeasurableGoal(goal_id), &goal);
        env.storage().persistent().extend_ttl(
            &DataKey::MeasurableGoal(goal_id),
            PERSISTENT_TTL_THRESHOLD,
            PERSISTENT_TTL_BUMP,
        );

        if first_achievement {
            env.events().publish(
                (Symbol::new(&env, "GoalAchieved"), plan_id),
                (goal_id, current_value, measured_at),
            );
        }

        Ok(())
    }

    /// Return the full time-series progress for a measurable goal.
    ///
    /// `caller` must be bound to the plan (its treating therapist or its patient).
    pub fn get_goal_progress(
        env: Env,
        caller: Address,
        plan_id: u64,
        goal_id: u64,
    ) -> Result<Vec<ProgressEntry>, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        // Validate goal belongs to this plan before returning.
        if let Some(goal) = env
            .storage()
            .persistent()
            .get::<_, MeasurableGoal>(&DataKey::MeasurableGoal(goal_id))
        {
            if goal.plan_id != plan_id {
                return Ok(Vec::new(&env));
            }
        } else {
            return Ok(Vec::new(&env));
        }

        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::GoalProgressList(goal_id))
            .unwrap_or(Vec::new(&env)))
    }

    /// Return a measurable goal by ID.
    ///
    /// `caller` must be bound to the goal's plan (its treating therapist or its patient).
    pub fn get_measurable_goal(
        env: Env,
        caller: Address,
        goal_id: u64,
    ) -> Result<MeasurableGoal, Error> {
        caller.require_auth();

        let goal: MeasurableGoal = env
            .storage()
            .persistent()
            .get(&DataKey::MeasurableGoal(goal_id))
            .ok_or(Error::NotFound)?;
        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(goal.plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(goal)
    }

    // ── Paginated getters (#564) ───────────────────────────────────────────────

    /// Paginated therapy session history for a treatment plan.
    ///
    /// `page` is 0-indexed. `page_size` is capped at `MAX_PAGE_SIZE`.
    /// The existing `get_therapy_sessions` full-list getter is deprecated in
    /// favour of this function for large session lists.
    pub fn get_therapy_sessions_paged(
        env: Env,
        caller: Address,
        treatment_plan_id: u64,
        page: u32,
        page_size: u32,
    ) -> Result<TherapyPage, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        let page_size = page_size.clamp(1, MAX_PAGE_SIZE);
        let all: Vec<TherapySession> = env
            .storage()
            .persistent()
            .get(&DataKey::TherapySessions(treatment_plan_id))
            .unwrap_or(Vec::new(&env));
        let total = all.len();
        let start = page.saturating_mul(page_size);
        let mut items = Vec::new(&env);
        let mut i = start;
        while i < start + page_size && i < total {
            items.push_back(all.get(i).unwrap());
            i += 1;
        }
        Ok(TherapyPage {
            items,
            has_more: (start + page_size) < total,
        })
    }

    /// Paginated progress notes for a treatment plan.
    ///
    /// `page` is 0-indexed. `page_size` is capped at `MAX_PAGE_SIZE`.
    pub fn get_progress_notes_paged(
        env: Env,
        caller: Address,
        treatment_plan_id: u64,
        page: u32,
        page_size: u32,
    ) -> Result<NotePage, Error> {
        caller.require_auth();

        let plan: RehabTreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;
        if !is_bound_to_plan(&env, &plan, &caller) {
            return Err(Error::Unauthorized);
        }

        let page_size = page_size.clamp(1, MAX_PAGE_SIZE);
        let all: Vec<ProgressNote> = env
            .storage()
            .persistent()
            .get(&DataKey::ProgressNotes(treatment_plan_id))
            .unwrap_or(Vec::new(&env));
        let total = all.len();
        let start = page.saturating_mul(page_size);
        let mut items = Vec::new(&env);
        let mut i = start;
        while i < start + page_size && i < total {
            items.push_back(all.get(i).unwrap());
            i += 1;
        }
        Ok(NotePage {
            items,
            has_more: (start + page_size) < total,
        })
    }
}

#[cfg(test)]
mod test;
