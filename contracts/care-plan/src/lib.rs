#![no_std]

mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Symbol, Vec};
use storage::*;
use types::*;

#[contract]
pub struct CarePlanContract;

#[contractimpl]
impl CarePlanContract {
    /// Create a new care plan for a patient.
    pub fn create_care_plan(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        plan_type: Symbol,
        conditions: Vec<String>,
        goals: Vec<String>,
        start_date: u64,
        review_frequency_days: u32,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

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
        provider_id.require_auth();

        if load_care_plan(&env, care_plan_id).is_none() {
            return Err(Error::CarePlanNotFound);
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

        env.events().publish(
            (Symbol::new(&env, "goal_added"),),
            (care_plan_id, goal_id),
        );

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
        provider_id.require_auth();

        if load_care_plan(&env, care_plan_id).is_none() {
            return Err(Error::CarePlanNotFound);
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
        patient_id.require_auth();

        let mut goal = load_goal(&env, goal_id).ok_or(Error::GoalNotFound)?;

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
    pub fn mark_goal_achieved(
        env: Env,
        goal_id: u64,
        provider_id: Address,
        achievement_date: u64,
        outcome_notes: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut goal = load_goal(&env, goal_id).ok_or(Error::GoalNotFound)?;

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
        reporter.require_auth();

        if load_care_plan(&env, care_plan_id).is_none() {
            return Err(Error::CarePlanNotFound);
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
    pub fn resolve_barrier(
        env: Env,
        barrier_id: u64,
        provider_id: Address,
        resolution: String,
        resolution_date: u64,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut barrier = load_barrier(&env, barrier_id).ok_or(Error::BarrierNotFound)?;

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
        provider_id.require_auth();

        if load_care_plan(&env, care_plan_id).is_none() {
            return Err(Error::CarePlanNotFound);
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
        provider_id.require_auth();

        let mut review = load_review(&env, review_id).ok_or(Error::ReviewNotFound)?;

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
        if let Some(mut plan) = load_care_plan(&env, review.care_plan_id) {
            plan.last_review_date = Some(conducted_at);
            plan.next_review_date =
                conducted_at + (plan.review_frequency_days as u64 * 86_400);

            if !continue_plan {
                plan.status = CarePlanStatus::Completed;
            }

            save_care_plan(&env, &plan);
        }

        save_review(&env, &review);

        env.events().publish(
            (Symbol::new(&env, "review_conducted"),),
            (review_id, provider_id, continue_plan),
        );

        Ok(())
    }

    /// Assign a care team member to a care plan.
    pub fn assign_care_team_member(
        env: Env,
        care_plan_id: u64,
        coordinating_provider: Address,
        team_member: Address,
        role: Symbol,
        responsibilities: Vec<String>,
    ) -> Result<(), Error> {
        coordinating_provider.require_auth();

        if load_care_plan(&env, care_plan_id).is_none() {
            return Err(Error::CarePlanNotFound);
        }

        let mut team = load_care_team(&env, care_plan_id);

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

    /// Get a summary of a care plan.
    pub fn get_care_plan_summary(
        env: Env,
        care_plan_id: u64,
        requester: Address,
    ) -> Result<CarePlanSummary, Error> {
        requester.require_auth();

        let plan = load_care_plan(&env, care_plan_id).ok_or(Error::CarePlanNotFound)?;

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