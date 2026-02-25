#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
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
}

#[contract]
pub struct HealthcareAnalytics;

#[contractimpl]
impl HealthcareAnalytics {
    /// Record an anonymized metric for population health analytics.
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

        for i in 0..ids.len() {
            let id = ids.get(i).unwrap();
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
                sum += record.value;
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
        env.storage()
            .persistent()
            .set(&DataKey::QualityMetricsByProvider(provider_id.clone()), &ids);

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

        for i in 0..ids.len() {
            let id = ids.get(i).unwrap();
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
