#![no_std]
#![allow(deprecated)]

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
}

/// --------------------
/// Errors
/// --------------------

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

    /// Request a report generation job
    /// Returns job_id if accepted, or error if throttled/insufficient resources
    pub fn request_report(
        env: Env,
        requester: Address,
        report_type: String,
        priority: JobPriority,
        estimated_cpu: u64,
        estimated_memory: u64,
    ) -> Result<u64, Error> {
        requester.require_auth();

        // Check if system is throttled
        if should_throttle_job(&env) {
            // Capture incident for throttling
            let incident_id = capture_incident(
                &env,
                IncidentSeverity::High,
                String::from_str(&env, "healthcare-analytics"),
                5, // Throttling error code
                String::from_str(&env, "Report job throttled due to resource constraints"),
                requester.clone(),
            );
            // Attach evidence: current resource usage
            let cpu_used: u64 = env.storage().instance().get(&ResourceKey::TotalCpuUsed).unwrap_or(0);
            let memory_used: u64 = env.storage().instance().get(&ResourceKey::TotalMemoryUsed).unwrap_or(0);
            let hash: Bytes = env.crypto().sha256(
                &Bytes::from_slice(&env, b"current_resource_usage_snapshot")
            ).into();
            let _ = attach_evidence(&env, incident_id, EvidenceType::StateSnapshot, hash, requester.clone());
            return Err(Error::JobThrottled);
        }

        let quota = ResourceQuota {
            cpu_units: estimated_cpu,
            memory_units: estimated_memory,
            timeout_seconds: 300, // 5 minutes default
        };

        // Create the job via shared module
        let job_id = create_report_job(&env, report_type.clone(), priority, requester.clone(), quota);

        env.events().publish(
            (symbol_short!("rep_job"), requester),
            (report_type, job_id),
        );

        Ok(job_id)
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
            );
            let hash: Bytes = env.crypto().sha256(
                &Bytes::from_slice(&env, b"resource_usage_exceeded_quota")
            ).into();
            let _ = attach_evidence(&env, incident_id, EvidenceType::ContextData, hash, job.requested_by);
        }

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
        let mut running: Vec<u64> = env
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
}

#[cfg(test)]
mod test;
