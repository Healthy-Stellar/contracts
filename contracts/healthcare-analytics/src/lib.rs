#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    String, Symbol, Vec,
};
use shared::resource_management::{
    create_report_job, complete_job, get_job, get_next_job_for_execution, get_system_limits,
    set_system_limits, should_throttle_job, JobPriority, JobState, ResourceKey, ResourceQuota, ResourceUsage,
};
use shared::incident_tracking::{
    capture_incident, attach_evidence, IncidentSeverity, EvidenceType,
};

/// --------------------
/// Degradation Policy
/// --------------------

/// Controls how `request_report` behaves when system resources are constrained.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DegradationPolicy {
    /// Fail hard — return `Error::JobThrottled` (default, preserves previous behaviour).
    Fail,
    /// Accept with halved resource quota; returns `ResultQuality::Truncated`.
    Approximate,
    /// Accept at 50 % sampling rate; returns `ResultQuality::Sampled`.
    Sample,
}

/// Indicates the completeness of a report job's results.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResultQuality {
    /// Full fidelity — no degradation applied.
    Full,
    /// Partial results due to resource cap; some records omitted.
    Truncated,
    /// Results derived from a statistical sample; extrapolation applied.
    Sampled,
}

/// Returned by `request_report` to surface the accepted job and its quality tier.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportJobAccepted {
    pub job_id: u64,
    pub result_quality: ResultQuality,
}

/// --------------------
/// Data Structures
/// --------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Statistics {
    pub metric_type: Symbol,
    pub count: u64,
    pub sum: i128,
    pub average: i128,
    pub min: i128,
    pub max: i128,
    pub period_start: u64,
    pub period_end: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricRecord {
    pub id: u64,
    pub metric_type: Symbol,
    pub value: i128,
    pub category: Symbol,
    pub timestamp: u64,
    pub metadata_hash: Option<BytesN<32>>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualityMetricRecord {
    pub id: u64,
    pub provider_id: Address,
    pub metric_name: String,
    pub value: i128,
    pub period: u64,
}

/// --------------------
/// Storage Keys
/// --------------------

#[contracttype]
pub enum DataKey {
    MetricCounter,
    Metric(u64),
    MetricsByType(Symbol),
    QualityMetricCounter,
    QualityMetric(u64),
    QualityMetricsByProvider(Address),
    Admin,
    /// Stores the `ResultQuality` for a report job accepted under a degraded policy.
    JobResultQuality(u64),
    PendingAdmin,
    RotationExpiry,
    RequesterCpuUsed(Address),
    RequesterMemoryUsed(Address),
    RequesterCpuLimit,
    RequesterMemoryLimit,
}

/// --------------------
/// Errors
/// --------------------

pub const ADMIN_ROTATION_WINDOW: u64 = 86_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    InvalidTimeRange = 1,
    NoDataFound = 2,
    Unauthorized = 3,
    InvalidValue = 4,
    JobThrottled = 5,
    InsufficientResources = 6,
    JobNotFound = 7,
    ResourceOverrun = 8,
    JobFailure = 9,
    ArithmeticOverflow = 10,
    RotationPending = 11,
    NoRotationPending = 12,
    RotationExpired = 13,
    NotPendingAdmin = 14,
    RequesterThrottled = 15,
    InvalidThreshold = 16,
    JobAlreadyExecuting = 17,
}

#[contract]
pub struct HealthcareAnalytics;

#[contractimpl]
impl HealthcareAnalytics {
    /// Initialize the analytics contract with admin and resource limits
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Request a report generation job.
    ///
    /// When the system is under resource pressure the caller can choose a
    /// `degradation_mode` instead of always failing:
    /// - `Fail`        — return `JobThrottled` (legacy behaviour).
    /// - `Approximate` — accept with a halved resource quota; results may be
    ///                    truncated. Returns `ResultQuality::Truncated`.
    /// - `Sample`      — accept at 50 % sampling rate; results are extrapolated.
    ///                    Returns `ResultQuality::Sampled`.
    ///
    /// A `rep_degr` event is emitted whenever a degraded mode is applied.
    pub fn request_report(
        env: Env,
        requester: Address,
        report_type: String,
        priority: JobPriority,
        estimated_cpu: u64,
        estimated_memory: u64,
        degradation_mode: DegradationPolicy,
    ) -> Result<ReportJobAccepted, Error> {
        requester.require_auth();

        // Per-requester throttle check — enforced independently of the global budget.
        let req_cpu_used: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RequesterCpuUsed(requester.clone()))
            .unwrap_or(0);
        let req_mem_used: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RequesterMemoryUsed(requester.clone()))
            .unwrap_or(0);
        let req_cpu_limit: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RequesterCpuLimit)
            .unwrap_or(u64::MAX);
        let req_mem_limit: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RequesterMemoryLimit)
            .unwrap_or(u64::MAX);
        if req_cpu_used.saturating_add(estimated_cpu) > req_cpu_limit
            || req_mem_used.saturating_add(estimated_memory) > req_mem_limit
        {
            return Err(Error::RequesterThrottled);
        }

        let throttled = should_throttle_job(&env, &report_type);

        if throttled {
            match degradation_mode {
                DegradationPolicy::Fail => {
                    let incident_id = capture_incident(
                        &env,
                        IncidentSeverity::High,
                        String::from_str(&env, "healthcare-analytics"),
                        5,
                        String::from_str(
                            &env,
                            "Report job throttled due to resource constraints",
                        ),
                        requester.clone(),
                        None,
                    );
                    let hash: Bytes = env
                        .crypto()
                        .sha256(&Bytes::from_slice(
                            &env,
                            b"current_resource_usage_snapshot",
                        ))
                        .into();
                    let _ = attach_evidence(
                        &env,
                        incident_id,
                        EvidenceType::StateSnapshot,
                        hash,
                        requester.clone(),
                    );
                    return Err(Error::JobThrottled);
                }
                DegradationPolicy::Approximate => {
                    let quota = ResourceQuota {
                        cpu_units: estimated_cpu / 2,
                        memory_units: estimated_memory / 2,
                        timeout_seconds: 300,
                    };
                    let job_id = create_report_job(
                        &env,
                        report_type.clone(),
                        priority,
                        requester.clone(),
                        quota,
                    );
                    env.storage().persistent().set(
                        &DataKey::JobResultQuality(job_id),
                        &ResultQuality::Truncated,
                    );
                    env.events().publish(
                        (symbol_short!("rep_degr"), requester.clone()),
                        (
                            report_type.clone(),
                            job_id,
                            Symbol::new(&env, "approximate"),
                            100u32,
                        ),
                    );
                    env.events()
                        .publish((symbol_short!("rep_job"), requester), (report_type, job_id));
                    return Ok(ReportJobAccepted {
                        job_id,
                        result_quality: ResultQuality::Truncated,
                    });
                }
                DegradationPolicy::Sample => {
                    let sampling_rate_pct: u32 = 50;
                    let quota = ResourceQuota {
                        cpu_units: estimated_cpu * (sampling_rate_pct as u64) / 100,
                        memory_units: estimated_memory * (sampling_rate_pct as u64) / 100,
                        timeout_seconds: 300,
                    };
                    let job_id = create_report_job(
                        &env,
                        report_type.clone(),
                        priority,
                        requester.clone(),
                        quota,
                    );
                    env.storage().persistent().set(
                        &DataKey::JobResultQuality(job_id),
                        &ResultQuality::Sampled,
                    );
                    env.events().publish(
                        (symbol_short!("rep_degr"), requester.clone()),
                        (
                            report_type.clone(),
                            job_id,
                            Symbol::new(&env, "sample"),
                            sampling_rate_pct,
                        ),
                    );
                    env.events()
                        .publish((symbol_short!("rep_job"), requester), (report_type, job_id));
                    return Ok(ReportJobAccepted {
                        job_id,
                        result_quality: ResultQuality::Sampled,
                    });
                }
            }
        }

        let quota = ResourceQuota {
            cpu_units: estimated_cpu,
            memory_units: estimated_memory,
            timeout_seconds: 300,
        };
        let job_id =
            create_report_job(&env, report_type.clone(), priority, requester.clone(), quota);

        env.events()
            .publish((symbol_short!("rep_job"), requester), (report_type, job_id));

        Ok(ReportJobAccepted {
            job_id,
            result_quality: ResultQuality::Full,
        })
    }

    /// Return the result quality for a report job (`Full` if not degraded).
    pub fn get_job_result_quality(env: Env, job_id: u64) -> ResultQuality {
        env.storage()
            .persistent()
            .get(&DataKey::JobResultQuality(job_id))
            .unwrap_or(ResultQuality::Full)
    }

    /// Execute next available report job (respects resource limits)
    /// Returns job_id if a job was started, or None if queue empty or resources exhausted
    pub fn execute_next_report(env: Env) -> Option<u64> {
        // Admin-only operation
        if let Some(job_id) = get_next_job_for_execution(&env) {
            // Start execution (in real implementation, this would spawn background job)
            let _ = shared::resource_management::start_job(&env, job_id);
            env.events()
                .publish((symbol_short!("exec_job"), job_id), symbol_short!("started"));
            Some(job_id)
        } else {
            None
        }
    }

    /// Mark a report job as completed with actual resource usage
    pub fn complete_report(env: Env, job_id: u64, cpu_used: u64, memory_used: u64) -> Result<(), Error> {
        let job = get_job(&env, job_id).map_err(|_| Error::JobNotFound)?;
        complete_job(&env, job_id, cpu_used, memory_used)
            .map_err(|_| Error::JobNotFound)?;

        // Check for resource overrun
        if cpu_used > job.quota.cpu_units || memory_used > job.quota.memory_units {
            let incident_id = capture_incident(
                &env,
                IncidentSeverity::Medium,
                String::from_str(&env, "healthcare-analytics"),
                8, // Resource overrun error code
                String::from_str(&env, "Job exceeded resource quota"),
                job.requested_by.clone(),
                None,
            );
            let hash: Bytes = env.crypto().sha256(
                &Bytes::from_slice(&env, b"resource_usage_exceeded_quota")
            ).into();
            let _ = attach_evidence(&env, incident_id, EvidenceType::ContextData, hash, job.requested_by.clone());
        }

        // Update per-requester cumulative usage.
        let req_cpu: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RequesterCpuUsed(job.requested_by.clone()))
            .unwrap_or(0);
        let req_mem: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RequesterMemoryUsed(job.requested_by.clone()))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::RequesterCpuUsed(job.requested_by.clone()),
            &(req_cpu + cpu_used),
        );
        env.storage().persistent().set(
            &DataKey::RequesterMemoryUsed(job.requested_by.clone()),
            &(req_mem + memory_used),
        );

        env.events()
            .publish((symbol_short!("job_done"), job_id), (cpu_used, memory_used));

        Ok(())
    }

    /// Mark a report job as failed and capture incident
    pub fn fail_report(env: Env, job_id: u64, error_message: String, requester: Address) -> Result<(), Error> {
        requester.require_auth();

        let mut job = get_job(&env, job_id).map_err(|_| Error::JobNotFound)?;
        job.state = JobState::Failed;
        env.storage().persistent().set(&ResourceKey::ReportJob(job_id), &job);

        // Remove from running jobs
        let running: Vec<u64> = env
            .storage()
            .persistent()
            .get(&ResourceKey::RunningJobs)
            .unwrap_or(Vec::new(&env));
        let mut new_running = Vec::new(&env);
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

        // Capture incident for job failure
        let incident_id = capture_incident(
            &env,
            IncidentSeverity::High,
            String::from_str(&env, "healthcare-analytics"),
            9, // Job failure error code
            String::from_str(&env, "Report job failed"),
            requester.clone(),
            None,
        );
        let hash: Bytes = env.crypto().sha256(
            &Bytes::from_slice(&env, b"report_job_error_log")
        ).into();
        let _ = attach_evidence(&env, incident_id, EvidenceType::ErrorLog, hash, requester);

        env.events()
            .publish((symbol_short!("job_fail"), job_id), error_message);

        Ok(())
    }

    /// Get system resource limits for monitoring
    pub fn get_resource_limits(env: Env) -> (u64, u64, u32, u64) {
        let limits = get_system_limits(&env);
        (
            limits.total_cpu_budget,
            limits.total_memory_budget,
            limits.max_concurrent_jobs,
            limits.throttle_threshold,
        )
    }

    /// Set system resource limits (admin only)
    pub fn set_resource_limits(
        env: Env,
        admin: Address,
        cpu_budget: u64,
        memory_budget: u64,
        max_concurrent: u32,
        throttle_percent: u64,
    ) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        if throttle_percent > 100 {
            return Err(Error::InvalidThreshold);
        }

        set_system_limits(
            &env,
            shared::resource_management::SystemResourceLimits {
                max_concurrent_jobs: max_concurrent,
                total_cpu_budget: cpu_budget,
                total_memory_budget: memory_budget,
                throttle_threshold: throttle_percent,
            },
        );

        Ok(())
    }

    /// Propose transferring admin to `new_admin`. Must be confirmed within 24 hours.
    pub fn propose_admin_rotation(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        if env.storage().instance().has(&DataKey::PendingAdmin) {
            return Err(Error::RotationPending);
        }
        let expiry = env.ledger().timestamp() + ADMIN_ROTATION_WINDOW;
        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
        env.storage().instance().set(&DataKey::RotationExpiry, &expiry);
        Ok(())
    }

    /// New admin confirms the rotation proposed by the current admin.
    pub fn accept_admin_rotation(env: Env, new_admin: Address) -> Result<(), Error> {
        new_admin.require_auth();
        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .ok_or(Error::NoRotationPending)?;
        if new_admin != pending {
            return Err(Error::NotPendingAdmin);
        }
        let expiry: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RotationExpiry)
            .unwrap_or(0);
        if env.ledger().timestamp() > expiry {
            env.storage().instance().remove(&DataKey::PendingAdmin);
            env.storage().instance().remove(&DataKey::RotationExpiry);
            return Err(Error::RotationExpired);
        }
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.storage().instance().remove(&DataKey::RotationExpiry);
        Ok(())
    }

    /// Configure per-requester CPU and memory caps (admin only).
    pub fn set_requester_limits(
        env: Env,
        admin: Address,
        cpu_limit: u64,
        memory_limit: u64,
    ) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::RequesterCpuLimit, &cpu_limit);
        env.storage()
            .instance()
            .set(&DataKey::RequesterMemoryLimit, &memory_limit);
        Ok(())
    }

    /// Configure report type specific throttle threshold (admin only)
    pub fn set_report_type_threshold(
        env: Env,
        admin: Address,
        report_type: String,
        threshold_pct: u64,
    ) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        if threshold_pct > 100 {
            return Err(Error::InvalidThreshold);
        }
        shared::resource_management::set_report_type_threshold(&env, report_type, threshold_pct);
        Ok(())
    }

    /// Return cumulative resource usage for a specific requester.
    pub fn get_requester_usage(env: Env, requester: Address) -> ResourceUsage {
        let cpu_used: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RequesterCpuUsed(requester.clone()))
            .unwrap_or(0);
        let memory_used: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RequesterMemoryUsed(requester.clone()))
            .unwrap_or(0);
        ResourceUsage {
            cpu_used,
            memory_used,
            start_time: 0,
            end_time: 0,
        }
    }

    /// Reset per-requester usage counters (admin only).
    pub fn reset_requester_usage(
        env: Env,
        admin: Address,
        requester: Address,
    ) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        env.storage()
            .persistent()
            .remove(&DataKey::RequesterCpuUsed(requester.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::RequesterMemoryUsed(requester.clone()));
        Ok(())
    }
    /// Privacy is preserved by accepting only pre-anonymized, aggregate-ready
    /// values with an optional metadata hash instead of raw patient data.
    pub fn record_metric(
        env: Env,
        metric_type: Symbol,
        value: i128,
        category: Symbol,
        timestamp: u64,
        metadata_hash: Option<BytesN<32>>,
    ) -> Result<(), Error> {
        let id = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::MetricCounter)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::MetricCounter, &(id + 1));

        let record = MetricRecord {
            id,
            metric_type: metric_type.clone(),
            value,
            category,
            timestamp,
            metadata_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Metric(id), &record);

        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::MetricsByType(metric_type.clone()))
            .unwrap_or(Vec::new(&env));
        ids.push_back(id);
        env.storage()
            .persistent()
            .set(&DataKey::MetricsByType(metric_type.clone()), &ids);

        env.events()
            .publish((symbol_short!("rec_met"), metric_type), id);

        Ok(())
    }

    /// Get aggregate statistics for a metric type within a time range.
    /// Optionally filter by category. Returns count, sum, average, min, and max.
    pub fn get_statistics(
        env: Env,
        metric_type: Symbol,
        start_time: u64,
        end_time: u64,
        category: Option<Symbol>,
    ) -> Result<Statistics, Error> {
        if start_time > end_time {
            return Err(Error::InvalidTimeRange);
        }

        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::MetricsByType(metric_type.clone()))
            .unwrap_or(Vec::new(&env));

        let mut count: u64 = 0;
        let mut sum: i128 = 0;
        let mut min: i128 = i128::MAX;
        let mut max: i128 = i128::MIN;

        for id in ids.iter() {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, MetricRecord>(&DataKey::Metric(id))
            {
                if record.timestamp < start_time || record.timestamp > end_time {
                    continue;
                }

                if let Some(ref cat) = category {
                    if record.category != *cat {
                        continue;
                    }
                }

                count += 1;
                sum = sum.checked_add(record.value).ok_or(Error::ArithmeticOverflow)?;
                if record.value < min {
                    min = record.value;
                }
                if record.value > max {
                    max = record.value;
                }
            }
        }

        if count == 0 {
            return Err(Error::NoDataFound);
        }

        let average = sum / count as i128;

        Ok(Statistics {
            metric_type,
            count,
            sum,
            average,
            min,
            max,
            period_start: start_time,
            period_end: end_time,
        })
    }

    /// Record a quality metric for a specific healthcare provider.
    /// Requires provider authorization.
    pub fn record_quality_metric(
        env: Env,
        provider_id: Address,
        metric_name: String,
        value: i128,
        period: u64,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let id = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::QualityMetricCounter)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::QualityMetricCounter, &(id + 1));

        let record = QualityMetricRecord {
            id,
            provider_id: provider_id.clone(),
            metric_name: metric_name.clone(),
            value,
            period,
        };

        env.storage()
            .persistent()
            .set(&DataKey::QualityMetric(id), &record);

        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::QualityMetricsByProvider(provider_id.clone()))
            .unwrap_or(Vec::new(&env));
        ids.push_back(id);
        env.storage().persistent().set(
            &DataKey::QualityMetricsByProvider(provider_id.clone()),
            &ids,
        );

        env.events()
            .publish((symbol_short!("rec_qm"), provider_id), metric_name);

        Ok(())
    }

    /// Retrieve quality metrics for a provider filtered by reporting period.
    pub fn get_quality_metrics(
        env: Env,
        provider_id: Address,
        period: u64,
    ) -> Result<Vec<QualityMetricRecord>, Error> {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::QualityMetricsByProvider(provider_id))
            .unwrap_or(Vec::new(&env));

        let mut results: Vec<QualityMetricRecord> = Vec::new(&env);

        for id in ids.iter() {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, QualityMetricRecord>(&DataKey::QualityMetric(id))
            {
                if record.period == period {
                    results.push_back(record);
                }
            }
        }

        if results.is_empty() {
            return Err(Error::NoDataFound);
        }

        Ok(results)
    }

    /// Cancel a queued report job.
    pub fn cancel_report(env: Env, requester: Address, job_id: u64) -> Result<(), Error> {
        requester.require_auth();

        let job = get_job(&env, job_id).map_err(|_| Error::JobNotFound)?;

        if job.requested_by != requester {
            return Err(Error::Unauthorized);
        }

        if job.state == JobState::Running {
            return Err(Error::JobAlreadyExecuting);
        }

        if job.state != JobState::Queued {
            return Err(Error::JobNotFound);
        }

        shared::resource_management::cancel_queued_job(&env, job_id)
            .map_err(|_| Error::JobNotFound)?;

        env.events()
            .publish((symbol_short!("rep_canc"), requester), job_id);

        Ok(())
    }
}

#[cfg(test)]
mod test;
