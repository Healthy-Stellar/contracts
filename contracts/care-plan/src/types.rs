#![no_std]

use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Symbol, Vec};

// -----------------------------------------------------------------------
// Error types
// -----------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    CarePlanNotFound = 2,
    GoalNotFound = 3,
    InterventionNotFound = 4,
    BarrierNotFound = 5,
    ReviewNotFound = 6,
    GoalAlreadyAchieved = 7,
    GoalDiscontinued = 8,
    BarrierAlreadyResolved = 9,
    ReviewAlreadyConducted = 10,
}

// -----------------------------------------------------------------------
// Enums
// -----------------------------------------------------------------------

/// Lifecycle status of a care goal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GoalStatus {
    /// Goal is active and being worked on.
    Active,
    /// Goal is progressing as expected.
    OnTrack,
    /// Goal is at risk of not being met.
    AtRisk,
    /// Goal has been successfully achieved.
    Achieved,
    /// Goal has been discontinued.
    Discontinued,
}

/// Lifecycle status of a care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CarePlanStatus {
    /// Care plan is active.
    Active,
    /// Care plan is under review.
    UnderReview,
    /// Care plan has been completed.
    Completed,
    /// Care plan has been discontinued.
    Discontinued,
}

// -----------------------------------------------------------------------
// Core structs
// -----------------------------------------------------------------------

/// A single progress entry logged against a care goal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressEntry {
    pub goal_id: u64,
    pub patient_id: Address,
    pub current_value: String,
    pub progress_note: String,
    pub recorded_date: u64,
}

/// A care goal associated with a care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CareGoal {
    pub goal_id: u64,
    pub care_plan_id: u64,
    pub description: String,
    pub target_value: Option<String>,
    pub target_date: u64,
    pub priority: Symbol,
    pub status: GoalStatus,
    pub progress_entries: Vec<ProgressEntry>,
    pub achievement_date: Option<u64>,
    pub outcome_notes: Option<String>,
    pub created_by: Address,
    pub created_at: u64,
}

/// An intervention associated with a care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Intervention {
    pub intervention_id: u64,
    pub care_plan_id: u64,
    pub intervention_type: Symbol,
    pub description: String,
    pub frequency: String,
    /// patient | provider | caregiver
    pub responsible_party: Symbol,
    pub assigned_by: Address,
    pub created_at: u64,
}

/// A barrier to care plan progress.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Barrier {
    pub barrier_id: u64,
    pub care_plan_id: u64,
    pub reporter: Address,
    pub barrier_type: Symbol,
    pub description: String,
    pub identified_date: u64,
    pub resolved: bool,
    pub resolution: Option<String>,
    pub resolution_date: Option<u64>,
    pub resolved_by: Option<Address>,
}

/// A scheduled review of a care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CareReview {
    pub review_id: u64,
    pub care_plan_id: u64,
    pub scheduled_by: Address,
    pub review_date: u64,
    pub review_type: Symbol,
    pub conducted: bool,
    pub review_notes_hash: Option<BytesN<32>>,
    pub plan_modifications: Vec<String>,
    pub continue_plan: bool,
    pub conducted_by: Option<Address>,
    pub conducted_at: Option<u64>,
}

/// A care team member assigned to a care plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CareTeamMember {
    pub care_plan_id: u64,
    pub team_member: Address,
    pub role: Symbol,
    pub responsibilities: Vec<String>,
    pub assigned_by: Address,
    pub assigned_at: u64,
}

/// The top-level care plan record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarePlan {
    pub care_plan_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    /// chronic_disease | post_op | preventive | palliative
    pub plan_type: Symbol,
    pub conditions: Vec<String>,
    pub goals: Vec<String>,
    pub start_date: u64,
    pub review_frequency_days: u32,
    pub status: CarePlanStatus,
    pub next_review_date: u64,
    pub last_review_date: Option<u64>,
    pub created_at: u64,
}

/// Summary returned by get_care_plan_summary.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarePlanSummary {
    pub care_plan_id: u64,
    pub patient_id: Address,
    pub plan_type: Symbol,
    pub active_goals: Vec<CareGoal>,
    pub interventions: Vec<Intervention>,
    pub care_team: Vec<CareTeamMember>,
    pub barriers: Vec<Barrier>,
    pub last_review_date: Option<u64>,
    pub next_review_date: u64,
}

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Auto-increment counter for care plans.
    CarePlanCounter,
    /// Auto-increment counter for goals.
    GoalCounter,
    /// Auto-increment counter for interventions.
    InterventionCounter,
    /// Auto-increment counter for barriers.
    BarrierCounter,
    /// Auto-increment counter for reviews.
    ReviewCounter,
    /// care_plan_id -> CarePlan
    CarePlan(u64),
    /// goal_id -> CareGoal
    Goal(u64),
    /// intervention_id -> Intervention
    Intervention(u64),
    /// barrier_id -> Barrier
    Barrier(u64),
    /// review_id -> CareReview
    Review(u64),
    /// care_plan_id -> Vec<u64> (goal ids)
    PlanGoals(u64),
    /// care_plan_id -> Vec<u64> (intervention ids)
    PlanInterventions(u64),
    /// care_plan_id -> Vec<u64> (barrier ids)
    PlanBarriers(u64),
    /// care_plan_id -> Vec<u64> (review ids)
    PlanReviews(u64),
    /// care_plan_id -> Vec<CareTeamMember>
    PlanCareTeam(u64),
    /// patient_id -> Vec<u64> (care plan ids)
    PatientPlans(Address),
}