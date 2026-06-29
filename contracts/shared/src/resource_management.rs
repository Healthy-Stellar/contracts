#![no_std]

use soroban_sdk::{contracttype, Address, Env, String, Vec};

/// Job priority levels
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Resource quota for a job
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceQuota {
    pub cpu_units: u64,       // Estimated CPU cost
    pub memory_units: u64,    // Estimated memory units
    pub timeout_seconds: u64, // Max execution time
}

/// Resource consumption tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceUsage {
    pub cpu_used: u64,
    pub memory_used: u64,
    pub start_time: u64,
    pub end_time: u64,
}

/// Job state
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
    Throttled,
    Cancelled,
}

/// Report job descriptor
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportJob {
    pub job_id: u64,
    pub job_type: String, // e.g., "adverse_event_report", "quality_metrics"
    pub priority: JobPriority,
    pub requested_by: Address,
    pub quota: ResourceQuota,
    pub usage: ResourceUsage,
    pub has_usage: bool,
    pub state: JobState,
    pub created_at: u64,
}

/// System-wide resource limits
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemResourceLimits {
    pub max_concurrent_jobs: u32,
    pub total_cpu_budget: u64,    // Per ledger
    pub total_memory_budget: u64, // Per ledger
    pub throttle_threshold: u64,  // % of budget at which to throttle
}

/// Storage keys for resource management
#[contracttype]
pub enum ResourceKey {
    Admin,
    JobCounter,
    ReportJob(u64),
    QueuedJobs,  // Vec<u64> - IDs of queued jobs
    RunningJobs, // Vec<u64> - IDs of running jobs
    SystemLimits,
    TotalCpuUsed,     // u64 - cumulative CPU usage this period
    TotalMemoryUsed,  // u64 - cumulative memory usage this period
    JobPriority(u64), // Quick lookup for priority
}

/// Default resource quotas by job type
pub const DEFAULT_CPU_QUOTA: u64 = 1_000_000; // CPU units
pub const DEFAULT_MEMORY_QUOTA: u64 = 100_000; // Memory units
pub const DEFAULT_TIMEOUT: u64 = 300; // 5 minutes

/// Default system limits
pub const DEFAULT_MAX_CONCURRENT: u32 = 5;
pub const DEFAULT_CPU_BUDGET: u64 = 10_000_000;
pub const DEFAULT_MEMORY_BUDGET: u64 = 1_000_000;
pub const DEFAULT_THROTTLE_THRESHOLD: u64 = 80; // 80%

/// Get or initialize system resource limits
pub fn get_system_limits(env: &Env) -> SystemResourceLimits {
    env.storage()
        .instance()
        .get(&ResourceKey::SystemLimits)
        .unwrap_or(SystemResourceLimits {
            max_concurrent_jobs: DEFAULT_MAX_CONCURRENT,
            total_cpu_budget: DEFAULT_CPU_BUDGET,
            total_memory_budget: DEFAULT_MEMORY_BUDGET,
            throttle_threshold: DEFAULT_THROTTLE_THRESHOLD,
        })
}

/// Update system resource limits
pub fn set_system_limits(env: &Env, limits: SystemResourceLimits) {
    env.storage()
        .instance()
        .set(&ResourceKey::SystemLimits, &limits);
}

/// Check if a job should be throttled due to resource constraints
pub fn should_throttle_job(env: &Env) -> bool {
    let limits = get_system_limits(env);
    let cpu_used: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::TotalCpuUsed)
        .unwrap_or(0u64);
    let memory_used: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::TotalMemoryUsed)
        .unwrap_or(0u64);

    let cpu_percent = (cpu_used * 100) / limits.total_cpu_budget;
    let memory_percent = (memory_used * 100) / limits.total_memory_budget;

    cpu_percent > limits.throttle_threshold || memory_percent > limits.throttle_threshold
}

/// Check if current system can accept a new job
pub fn can_accept_job(env: &Env, requested_quota: &ResourceQuota) -> bool {
    let limits = get_system_limits(env);

    // Check concurrent job limit
    let running: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ResourceKey::RunningJobs)
        .unwrap_or(Vec::new(env));
    if running.len() >= limits.max_concurrent_jobs {
        return false;
    }

    // Check remaining budget
    let cpu_used: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::TotalCpuUsed)
        .unwrap_or(0u64);
    let memory_used: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::TotalMemoryUsed)
        .unwrap_or(0u64);

    let cpu_available = limits.total_cpu_budget.saturating_sub(cpu_used);
    let memory_available = limits.total_memory_budget.saturating_sub(memory_used);

    requested_quota.cpu_units <= cpu_available && requested_quota.memory_units <= memory_available
}

/// Create a new report job
pub fn create_report_job(
    env: &Env,
    job_type: String,
    priority: JobPriority,
    requester: Address,
    quota: ResourceQuota,
) -> u64 {
    let job_id: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::JobCounter)
        .unwrap_or(0u64)
        + 1;
    env.storage()
        .instance()
        .set(&ResourceKey::JobCounter, &job_id);

    let job = ReportJob {
        job_id,
        job_type,
        priority: priority.clone(),
        requested_by: requester,
        quota,
        usage: ResourceUsage {
            cpu_used: 0,
            memory_used: 0,
            start_time: 0,
            end_time: 0,
        },
        has_usage: false,
        state: JobState::Queued,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&ResourceKey::ReportJob(job_id), &job);

    env.storage()
        .persistent()
        .set(&ResourceKey::JobPriority(job_id), &priority);

    // Add to queued jobs
    let mut queued: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ResourceKey::QueuedJobs)
        .unwrap_or(Vec::new(env));
    queued.push_back(job_id);
    env.storage()
        .persistent()
        .set(&ResourceKey::QueuedJobs, &queued);

    job_id
}

/// Start executing a job
pub fn start_job(env: &Env, job_id: u64) -> Result<(), ()> {
    let mut job: ReportJob = env
        .storage()
        .persistent()
        .get(&ResourceKey::ReportJob(job_id))
        .ok_or(())?;

    job.state = JobState::Running;
    job.usage = ResourceUsage {
        cpu_used: 0,
        memory_used: 0,
        start_time: env.ledger().timestamp(),
        end_time: 0,
    };
    job.has_usage = true;

    env.storage()
        .persistent()
        .set(&ResourceKey::ReportJob(job_id), &job);

    // Move from queued to running
    let mut queued: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ResourceKey::QueuedJobs)
        .unwrap_or(Vec::new(env));
    let mut new_queued = Vec::new(env);
    for i in 0..queued.len() {
        if let Some(id) = queued.get(i) {
            if id != job_id {
                new_queued.push_back(id);
            }
        }
    }
    env.storage()
        .persistent()
        .set(&ResourceKey::QueuedJobs, &new_queued);

    let mut running: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ResourceKey::RunningJobs)
        .unwrap_or(Vec::new(env));
    running.push_back(job_id);
    env.storage()
        .persistent()
        .set(&ResourceKey::RunningJobs, &running);

    Ok(())
}

/// Complete job execution and record resource usage
pub fn complete_job(env: &Env, job_id: u64, cpu_used: u64, memory_used: u64) -> Result<(), ()> {
    let mut job: ReportJob = env
        .storage()
        .persistent()
        .get(&ResourceKey::ReportJob(job_id))
        .ok_or(())?;

    job.state = JobState::Completed;
    job.usage.cpu_used = cpu_used;
    job.usage.memory_used = memory_used;
    job.usage.end_time = env.ledger().timestamp();
    job.has_usage = true;

    env.storage()
        .persistent()
        .set(&ResourceKey::ReportJob(job_id), &job);

    // Update system totals
    let cpu_total: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::TotalCpuUsed)
        .unwrap_or(0u64);
    let memory_total: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::TotalMemoryUsed)
        .unwrap_or(0u64);

    env.storage()
        .instance()
        .set(&ResourceKey::TotalCpuUsed, &(cpu_total + cpu_used));
    env.storage()
        .instance()
        .set(&ResourceKey::TotalMemoryUsed, &(memory_total + memory_used));

    // Remove from running
    let mut running: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ResourceKey::RunningJobs)
        .unwrap_or(Vec::new(env));
    let mut new_running = Vec::new(env);
    for i in 0..running.len() {
        if let Some(id) = running.get(i) {
            if id != job_id {
                new_running.push_back(id);
            }
        }
    }
    env.storage()
        .persistent()
        .set(&ResourceKey::RunningJobs, &new_running);

    Ok(())
}

/// Get job details
pub fn get_job(env: &Env, job_id: u64) -> Result<ReportJob, ()> {
    env.storage()
        .persistent()
        .get(&ResourceKey::ReportJob(job_id))
        .ok_or(())
}

/// Get next high-priority job from queue (respects resource limits)
pub fn get_next_job_for_execution(env: &Env) -> Option<u64> {
    let queued: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ResourceKey::QueuedJobs)
        .unwrap_or(Vec::new(env));

    // Find highest priority queued job that fits within resource limits
    let mut best_job: Option<(u64, JobPriority)> = None;

    for i in 0..queued.len() {
        if let Some(job_id) = queued.get(i) {
            if let Ok(job) = get_job(env, job_id) {
                if can_accept_job(env, &job.quota) {
                    match &best_job {
                        None => best_job = Some((job_id, job.priority)),
                        Some((_, best_priority)) => {
                            if job.priority > *best_priority {
                                best_job = Some((job_id, job.priority));
                            }
                        }
                    }
                }
            }
        }
    }

    best_job.map(|(job_id, _)| job_id)
}

/// Cancel a queued job, transitioning its state to Cancelled and removing it from the queue
pub fn cancel_queued_job(env: &Env, job_id: u64) -> Result<(), ()> {
    let mut job: ReportJob = env
        .storage()
        .persistent()
        .get(&ResourceKey::ReportJob(job_id))
        .ok_or(())?;

    if job.state != JobState::Queued {
        return Err(());
    }

    job.state = JobState::Cancelled;
    env.storage()
        .persistent()
        .set(&ResourceKey::ReportJob(job_id), &job);

    // Remove from QueuedJobs
    let queued: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ResourceKey::QueuedJobs)
        .unwrap_or(Vec::new(env));
    let mut new_queued = Vec::new(env);
    for i in 0..queued.len() {
        if let Some(id) = queued.get(i) {
            if id != job_id {
                new_queued.push_back(id);
            }
        }
    }
    env.storage()
        .persistent()
        .set(&ResourceKey::QueuedJobs, &new_queued);

    Ok(())
}
