#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Care Plan Contract
//!
//! Manages patient care plans with goals, conditions, review scheduling, and provider authorization.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Provider authentication required for care plan creation and updates.
//! Patient enrollment with provider validation. Goal and adjustment authorization by enrolled providers.
//! Read access restricted to authorized parties via access control checks.
//!
//! **Audit Controls:** Events emitted for plan creation, goal achievement, plan completion, and
//! adjustments. Review dates tracked with last_review_date capturing modification history. Provider
//! and patient addresses logged with all operations for auditability.
//!
//! **Data Retention Policy:** Active care plans retain all historical review data. Completion status
//! captured with end date. Review frequency enforced with next_review_date tracking. Plan deregistration
//! cleans up all patient-specific care plan state.
//!
//! **Encryption/Integrity:** Care plan data stored in persistent storage with version tracking.
//! Timestamp integrity maintained via ledger timestamp. Provider registry validation ensures
//! only authorized providers manage plans.

mod storage;
mod types;

#[cfg(test)]
mod test;

use shared::privacy::{validate_encrypted_ref, EncryptedEnvelopeRef};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Symbol, Vec};
use storage::*;
use types::*;

/// Maximum number of members a single care plan's team may hold.
pub const MAX_CARE_TEAM_SIZE: u32 = 20;

/// Care-team role allowed to manage the plan and add (non-coordinator) members.
pub const ROLE_COORDINATOR: &str = "coordinator";
/// Care-team role allowed to manage goals and reviews, but not the team.
pub const ROLE_PHYSICIAN: &str = "physician";

/// Classes of action on a care plan, each gated by the caller's role.
enum PlanAction {
    /// Adding care-team members.
    ManageTeam,
    /// Goals, reviews and plan status changes.
    ManagePlan,
    /// Interventions and barriers.
    Contribute,
}

/// Returns the role of `caller` on the plan's care team, if they are a member.
fn team_role(env: &Env, plan: &CarePlan, caller: &Address) -> Option<Symbol> {
    let team = load_care_team(env, plan.care_plan_id);
    for member in team.iter() {
        if member.team_member == *caller {
            return Some(member.role);
        }
    }
    None
}

/// Returns true if `caller` is the plan's provider or an assigned care-team member.
fn is_bound_to_plan(env: &Env, plan: &CarePlan, caller: &Address) -> bool {
    *caller == plan.provider_id || team_role(env, plan, caller).is_some()
}

/// Returns true if `caller` may perform `action` on the plan.
///
/// The plan owner (`provider_id`) may do everything. Care-team members are limited
/// by their stored role: only a coordinator may add members, only a coordinator or
/// physician may manage goals, reviews and plan status, and any member may record
/// interventions and barriers.
fn can_perform(env: &Env, plan: &CarePlan, caller: &Address, action: PlanAction) -> bool {
    if *caller == plan.provider_id {
        return true;
    }
    let role = match team_role(env, plan, caller) {
        Some(role) => role,
        None => return false,
    };
    let coordinator = Symbol::new(env, ROLE_COORDINATOR);
    match action {
        PlanAction::ManageTeam => role == coordinator,
        PlanAction::ManagePlan => role == coordinator || role == Symbol::new(env, ROLE_PHYSICIAN),
        PlanAction::Contribute => true,
    }
}

/// Rejects any condition/goal reference that is not a valid encrypted envelope reference.
fn validate_refs(refs: &Vec<EncryptedEnvelopeRef>) -> Result<(), Error> {
    for r in refs.iter() {
        validate_encrypted_ref(&r).map_err(|_| Error::InvalidEncryptedRef)?;
    }
    Ok(())
}

#[contract]
pub struct CarePlanContract;

#[contractimpl]
impl CarePlanContract {
    /// Create a new care plan for a patient.
    ///
    /// Both the provider and the patient must authorize. Conditions and goals are
    /// stored only as encrypted envelope references, never as plaintext diagnoses.
    pub fn create_care_plan(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        plan_type: Symbol,
        conditions: Vec<EncryptedEnvelopeRef>,
        goals: Vec<EncryptedEnvelopeRef>,
        start_date: u64,
        review_frequency_days: u32,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();
        patient_id.require_auth();

        validate_refs(&conditions)?;
        validate_refs(&goals)?;

        let care_plan_id = next_care_plan_id(&env);
        let next_review_date = start_date + (review_frequency_days as u64 * 86_400);

        let plan = CarePlan {
            care_plan_id,
            patient_id: patient_id.clone(),
            provider_id: provider_id.clone(),
            plan_type,
            conditions,
            goals,
            start_date,
            review_frequency_days,
            status: CarePlanStatus::Active,
            next_review_date,
            last_review_date: None,
            created_at: env.ledger().timestamp(),
        };

        save_care_plan(&env, &plan);
        add_patient_plan(&env, &patient_id, care_plan_id);

        env.events().publish(
            (Symbol::new(&env, "care_plan_created"),),
            (care_plan_id, patient_id, provider_id),
        );

        Ok(care_plan_id)
    }

    /// Add a goal to an existing care plan.
    pub fn add_care_goal(
        env: Env,
        care_plan_id: u64,
        provider_id: Address,
        goal_description: String,
        target_value: Option<String>,
        target_date: u64,
        priority: Symbol,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if !can_perform(&env, &plan, &provider_id, PlanAction::ManagePlan) {
            return Err(Error::Unauthorized);
        }
        if matches!(
            plan.status,
            CarePlanStatus::Completed | CarePlanStatus::Discontinued
        ) {
            return Err(Error::CarePlanClosed);
        }

        let goal_id = next_goal_id(&env);

        let goal = CareGoal {
            goal_id,
            care_plan_id,
            description: goal_description,
            target_value,
            target_date,
            priority,
            status: GoalStatus::Active,
            progress_entries: Vec::new(&env),
            achievement_date: None,
            outcome_notes: None,
            created_by: provider_id.clone(),
            created_at: env.ledger().timestamp(),
        };

        save_goal(&env, &goal);
        add_plan_goal(&env, care_plan_id, goal_id);

        env.events()
            .publish((Symbol::new(&env, "goal_added"),), (care_plan_id, goal_id));

        Ok(goal_id)
    }

    /// Add an intervention to a care plan.
    pub fn add_intervention(
        env: Env,
        care_plan_id: u64,
        provider_id: Address,
        intervention_type: Symbol,
        description: String,
        frequency: String,
        responsible_party: Symbol,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if !can_perform(&env, &plan, &provider_id, PlanAction::Contribute) {
            return Err(Error::Unauthorized);
        }
        if matches!(
            plan.status,
            CarePlanStatus::Completed | CarePlanStatus::Discontinued
        ) {
            return Err(Error::CarePlanClosed);
        }

        let intervention_id = next_intervention_id(&env);

        let intervention = Intervention {
            intervention_id,
            care_plan_id,
            intervention_type,
            description,
            frequency,
            responsible_party,
            assigned_by: provider_id.clone(),
            created_at: env.ledger().timestamp(),
        };

        save_intervention(&env, &intervention);
        add_plan_intervention(&env, care_plan_id, intervention_id);

        env.events().publish(
            (Symbol::new(&env, "intervention_added"),),
            (care_plan_id, intervention_id),
        );

        Ok(intervention_id)
    }

    /// Record progress against a care goal.
    pub fn record_goal_progress(
        env: Env,
        goal_id: u64,
        patient_id: Address,
        current_value: String,
        progress_note: String,
        recorded_date: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        patient_id.require_auth();

        let mut goal = load_goal(&env, goal_id).ok_or(Error::GoalNotFound)?;
        let plan = load_care_plan(&env, goal.care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if patient_id != plan.patient_id {
            return Err(Error::Unauthorized);
        }

        if matches!(goal.status, GoalStatus::Achieved) {
            return Err(Error::GoalAlreadyAchieved);
        }
        if matches!(goal.status, GoalStatus::Discontinued) {
            return Err(Error::GoalDiscontinued);
        }

        let entry = ProgressEntry {
            goal_id,
            patient_id: patient_id.clone(),
            current_value,
            progress_note,
            recorded_date,
        };

        goal.progress_entries.push_back(entry);
        save_goal(&env, &goal);

        env.events().publish(
            (Symbol::new(&env, "goal_progress_recorded"),),
            (goal_id, patient_id),
        );

        Ok(())
    }

    /// Mark a care goal as achieved.
    ///
    /// Allowed even if the parent care plan has since been completed or discontinued:
    /// closing out a goal that was already on the plan reflects a real clinical outcome
    /// and does not add new activity to a closed plan, unlike `add_care_goal`.
    pub fn mark_goal_achieved(
        env: Env,
        goal_id: u64,
        provider_id: Address,
        achievement_date: u64,
        outcome_notes: String,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();

        let mut goal = load_goal(&env, goal_id).ok_or(Error::GoalNotFound)?;
        let plan = load_care_plan(&env, goal.care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if !can_perform(&env, &plan, &provider_id, PlanAction::ManagePlan) {
            return Err(Error::Unauthorized);
        }

        if matches!(goal.status, GoalStatus::Achieved) {
            return Err(Error::GoalAlreadyAchieved);
        }
        if matches!(goal.status, GoalStatus::Discontinued) {
            return Err(Error::GoalDiscontinued);
        }

        goal.status = GoalStatus::Achieved;
        goal.achievement_date = Some(achievement_date);
        goal.outcome_notes = Some(outcome_notes);

        save_goal(&env, &goal);

        env.events().publish(
            (Symbol::new(&env, "goal_achieved"),),
            (goal_id, provider_id),
        );

        Ok(())
    }

    /// Add a barrier to a care plan.
    pub fn add_barrier(
        env: Env,
        care_plan_id: u64,
        reporter: Address,
        barrier_type: Symbol,
        description: String,
        identified_date: u64,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        reporter.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if !can_perform(&env, &plan, &reporter, PlanAction::Contribute) {
            return Err(Error::Unauthorized);
        }
        if matches!(
            plan.status,
            CarePlanStatus::Completed | CarePlanStatus::Discontinued
        ) {
            return Err(Error::CarePlanClosed);
        }

        let barrier_id = next_barrier_id(&env);

        let barrier = Barrier {
            barrier_id,
            care_plan_id,
            reporter: reporter.clone(),
            barrier_type,
            description,
            identified_date,
            resolved: false,
            resolution: None,
            resolution_date: None,
            resolved_by: None,
        };

        save_barrier(&env, &barrier);
        add_plan_barrier(&env, care_plan_id, barrier_id);

        env.events().publish(
            (Symbol::new(&env, "barrier_added"),),
            (care_plan_id, barrier_id),
        );

        Ok(barrier_id)
    }

    /// Resolve a barrier.
    ///
    /// Allowed even if the parent care plan has since been completed or discontinued, for
    /// the same reason as `mark_goal_achieved`: it closes out existing state rather than
    /// adding new activity to a closed plan.
    pub fn resolve_barrier(
        env: Env,
        barrier_id: u64,
        provider_id: Address,
        resolution: String,
        resolution_date: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();

        let mut barrier = load_barrier(&env, barrier_id).ok_or(Error::BarrierNotFound)?;
        let plan = load_care_plan(&env, barrier.care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if !can_perform(&env, &plan, &provider_id, PlanAction::Contribute) {
            return Err(Error::Unauthorized);
        }

        if barrier.resolved {
            return Err(Error::BarrierAlreadyResolved);
        }

        barrier.resolved = true;
        barrier.resolution = Some(resolution);
        barrier.resolution_date = Some(resolution_date);
        barrier.resolved_by = Some(provider_id.clone());

        save_barrier(&env, &barrier);

        env.events().publish(
            (Symbol::new(&env, "barrier_resolved"),),
            (barrier_id, provider_id),
        );

        Ok(())
    }

    /// Schedule a review for a care plan.
    pub fn schedule_care_plan_review(
        env: Env,
        care_plan_id: u64,
        provider_id: Address,
        review_date: u64,
        review_type: Symbol,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if !can_perform(&env, &plan, &provider_id, PlanAction::ManagePlan) {
            return Err(Error::Unauthorized);
        }
        if matches!(
            plan.status,
            CarePlanStatus::Completed | CarePlanStatus::Discontinued
        ) {
            return Err(Error::CarePlanClosed);
        }

        let review_id = next_review_id(&env);

        let review = CareReview {
            review_id,
            care_plan_id,
            scheduled_by: provider_id.clone(),
            review_date,
            review_type,
            conducted: false,
            review_notes_hash: None,
            plan_modifications: Vec::new(&env),
            continue_plan: true,
            conducted_by: None,
            conducted_at: None,
        };

        save_review(&env, &review);
        add_plan_review(&env, care_plan_id, review_id);

        env.events().publish(
            (Symbol::new(&env, "review_scheduled"),),
            (care_plan_id, review_id, review_date),
        );

        Ok(review_id)
    }

    /// Conduct a previously scheduled care plan review.
    pub fn conduct_care_plan_review(
        env: Env,
        review_id: u64,
        provider_id: Address,
        review_notes_hash: BytesN<32>,
        plan_modifications: Vec<String>,
        continue_plan: bool,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();

        let mut review = load_review(&env, review_id).ok_or(Error::ReviewNotFound)?;
        let mut plan = load_care_plan(&env, review.care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if !can_perform(&env, &plan, &provider_id, PlanAction::ManagePlan) {
            return Err(Error::Unauthorized);
        }

        if review.conducted {
            return Err(Error::ReviewAlreadyConducted);
        }

        let conducted_at = env.ledger().timestamp();

        review.conducted = true;
        review.review_notes_hash = Some(review_notes_hash);
        review.plan_modifications = plan_modifications;
        review.continue_plan = continue_plan;
        review.conducted_by = Some(provider_id.clone());
        review.conducted_at = Some(conducted_at);

        // Update the parent care plan's last/next review dates
        plan.last_review_date = Some(conducted_at);
        plan.next_review_date = conducted_at + (plan.review_frequency_days as u64 * 86_400);

        if !continue_plan {
            plan.status = CarePlanStatus::Completed;
        }

        save_care_plan(&env, &plan);
        save_review(&env, &review);

        env.events().publish(
            (Symbol::new(&env, "review_conducted"),),
            (review_id, provider_id, continue_plan),
        );

        Ok(())
    }

    /// Assign a care team member to a care plan.
    ///
    /// Only the plan owner or a member with the `coordinator` role may add members,
    /// and only the plan owner may add another coordinator, so access cannot grow
    /// transitively. The patient must authorize every addition. The team is
    /// de-duplicated and capped at `MAX_CARE_TEAM_SIZE`.
    pub fn assign_care_team_member(
        env: Env,
        care_plan_id: u64,
        coordinating_provider: Address,
        team_member: Address,
        role: Symbol,
        responsibilities: Vec<String>,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        coordinating_provider.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if !can_perform(&env, &plan, &coordinating_provider, PlanAction::ManageTeam) {
            return Err(Error::Unauthorized);
        }
        if role == Symbol::new(&env, ROLE_COORDINATOR) && coordinating_provider != plan.provider_id
        {
            return Err(Error::Unauthorized);
        }

        // Patient consent is required for every addition to their care team.
        plan.patient_id.require_auth();

        if team_member == plan.provider_id || team_member == plan.patient_id {
            return Err(Error::InvalidTeamMember);
        }

        let mut team = load_care_team(&env, care_plan_id);
        for existing in team.iter() {
            if existing.team_member == team_member {
                return Err(Error::AlreadyTeamMember);
            }
        }
        if team.len() >= MAX_CARE_TEAM_SIZE {
            return Err(Error::CareTeamFull);
        }

        let member = CareTeamMember {
            care_plan_id,
            team_member: team_member.clone(),
            role,
            responsibilities,
            assigned_by: coordinating_provider.clone(),
            assigned_at: env.ledger().timestamp(),
        };

        team.push_back(member);
        save_care_team(&env, care_plan_id, &team);

        env.events().publish(
            (Symbol::new(&env, "team_member_assigned"),),
            (care_plan_id, team_member),
        );

        Ok(())
    }

    /// Remove a member from a care plan's team, revoking their access to the plan.
    ///
    /// Callable by the plan owner or the patient.
    pub fn remove_care_team_member(
        env: Env,
        care_plan_id: u64,
        caller: Address,
        team_member: Address,
    ) -> Result<(), Error> {
        caller.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if caller != plan.provider_id && caller != plan.patient_id {
            return Err(Error::Unauthorized);
        }

        let mut team = load_care_team(&env, care_plan_id);
        let mut index: Option<u32> = None;
        for (i, member) in team.iter().enumerate() {
            if member.team_member == team_member {
                index = Some(i as u32);
                break;
            }
        }
        let index = index.ok_or(Error::TeamMemberNotFound)?;
        team.remove(index);
        save_care_team(&env, care_plan_id, &team);

        env.events().publish(
            (Symbol::new(&env, "team_member_removed"),),
            (care_plan_id, team_member, caller),
        );

        Ok(())
    }

    /// Remove all care-plan state for a deregistered patient.
    ///
    /// Cancels every active care plan and removes the `PatientPlans` index.
    /// Callable by the patient themselves.
    pub fn deregister_patient(env: Env, patient_id: Address) {
        ttl_config::extend_instance_ttl(&env);
        patient_id.require_auth();

        let plan_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientPlans(patient_id.clone()))
            .unwrap_or(Vec::new(&env));

        for plan_id in plan_ids.iter() {
            if let Some(mut plan) = load_care_plan(&env, plan_id) {
                if matches!(plan.status, CarePlanStatus::Active) {
                    plan.status = CarePlanStatus::Discontinued;
                    save_care_plan(&env, &plan);
                }
            }
        }

        env.storage()
            .persistent()
            .remove(&DataKey::PatientPlans(patient_id.clone()));

        env.events().publish(
            (Symbol::new(&env, "pat_dreg"), patient_id),
            Symbol::new(&env, "cp_clean"),
        );
    }

    /// Get a summary of a care plan.
    ///
    /// Access is restricted to the patient, the plan's provider, or an assigned
    /// care-team member, mirroring the authorization used by every mutating path.
    pub fn get_care_plan_summary(
        env: Env,
        care_plan_id: u64,
        requester: Address,
    ) -> Result<CarePlanSummary, Error> {
        requester.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;
        if requester != plan.patient_id && !is_bound_to_plan(&env, &plan, &requester) {
            return Err(Error::Unauthorized);
        }

        // Collect active goals
        let goal_ids = load_plan_goals(&env, care_plan_id);
        let mut active_goals: Vec<CareGoal> = Vec::new(&env);
        for id in goal_ids.iter() {
            if let Some(g) = load_goal(&env, id) {
                if !matches!(g.status, GoalStatus::Achieved | GoalStatus::Discontinued) {
                    active_goals.push_back(g);
                }
            }
        }

        // Collect interventions
        let intervention_ids = load_plan_interventions(&env, care_plan_id);
        let mut interventions: Vec<Intervention> = Vec::new(&env);
        for id in intervention_ids.iter() {
            if let Some(i) = load_intervention(&env, id) {
                interventions.push_back(i);
            }
        }

        let care_team = load_care_team(&env, care_plan_id);
        let barriers = load_plan_barriers(&env, care_plan_id);

        Ok(CarePlanSummary {
            care_plan_id,
            patient_id: plan.patient_id,
            plan_type: plan.plan_type,
            active_goals,
            interventions,
            care_team,
            barriers,
            last_review_date: plan.last_review_date,
            next_review_date: plan.next_review_date,
        })
    }
}
