#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, String};

fn setup() -> (Env, HealthcareAnalyticsClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);
    (env, client)
}

// ========================
// record_metric tests
// ========================

#[test]
fn test_record_metric_basic() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );

    let stats = client.get_statistics(&symbol_short!("bp"), &1699999999, &1700000001, &None);
    assert_eq!(stats.count, 1);
    assert_eq!(stats.sum, 120);
    assert_eq!(stats.average, 120);
    assert_eq!(stats.min, 120);
    assert_eq!(stats.max, 120);
}

#[test]
fn test_record_metric_with_metadata_hash() {
    let (env, client) = setup();

    let hash: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);

    client.record_metric(
        &symbol_short!("bp"),
        &130,
        &symbol_short!("vitals"),
        &1700000000,
        &Some(hash),
    );

    let stats = client.get_statistics(&symbol_short!("bp"), &1699999999, &1700000001, &None);
    assert_eq!(stats.count, 1);
    assert_eq!(stats.sum, 130);
}

#[test]
fn test_record_multiple_metrics_same_type() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &130,
        &symbol_short!("vitals"),
        &1700001000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &110,
        &symbol_short!("vitals"),
        &1700002000,
        &None,
    );

    let stats = client.get_statistics(&symbol_short!("bp"), &1699999999, &1700002001, &None);
    assert_eq!(stats.count, 3);
    assert_eq!(stats.sum, 360);
    assert_eq!(stats.average, 120);
    assert_eq!(stats.min, 110);
    assert_eq!(stats.max, 130);
}

#[test]
fn test_statistics_sum_overflow_rejected() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &i128::MAX,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &1,
        &symbol_short!("vitals"),
        &1700000001,
        &None,
    );

    let result = client.try_get_statistics(
        &symbol_short!("bp"),
        &1699999999,
        &1700000002,
        &None,
    );

    assert_eq!(result, Err(Ok(Error::ArithmeticOverflow)));
}

#[test]
fn test_record_metrics_different_types() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("hr"),
        &72,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );

    let bp_stats = client.get_statistics(&symbol_short!("bp"), &1699999999, &1700000001, &None);
    assert_eq!(bp_stats.count, 1);
    assert_eq!(bp_stats.sum, 120);
    assert_eq!(bp_stats.metric_type, symbol_short!("bp"));

    let hr_stats = client.get_statistics(&symbol_short!("hr"), &1699999999, &1700000001, &None);
    assert_eq!(hr_stats.count, 1);
    assert_eq!(hr_stats.sum, 72);
    assert_eq!(hr_stats.metric_type, symbol_short!("hr"));
}

#[test]
fn test_record_metric_negative_values() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("temp"),
        &-5,
        &symbol_short!("lab"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("temp"),
        &10,
        &symbol_short!("lab"),
        &1700001000,
        &None,
    );

    let stats = client.get_statistics(&symbol_short!("temp"), &1699999999, &1700001001, &None);
    assert_eq!(stats.count, 2);
    assert_eq!(stats.sum, 5);
    assert_eq!(stats.average, 2);
    assert_eq!(stats.min, -5);
    assert_eq!(stats.max, 10);
}

// ========================
// get_statistics tests
// ========================

#[test]
fn test_get_statistics_time_range_filter() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &140,
        &symbol_short!("vitals"),
        &1700050000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &100,
        &symbol_short!("vitals"),
        &1700100000,
        &None,
    );

    // Only the first two should match
    let stats = client.get_statistics(&symbol_short!("bp"), &1699999999, &1700050001, &None);
    assert_eq!(stats.count, 2);
    assert_eq!(stats.sum, 260);
    assert_eq!(stats.min, 120);
    assert_eq!(stats.max, 140);
    assert_eq!(stats.period_start, 1699999999);
    assert_eq!(stats.period_end, 1700050001);
}

#[test]
fn test_get_statistics_category_filter() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &200,
        &symbol_short!("emer"),
        &1700001000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &130,
        &symbol_short!("vitals"),
        &1700002000,
        &None,
    );

    // Only "vitals" category
    let stats = client.get_statistics(
        &symbol_short!("bp"),
        &1699999999,
        &1700002001,
        &Some(symbol_short!("vitals")),
    );
    assert_eq!(stats.count, 2);
    assert_eq!(stats.sum, 250);
    assert_eq!(stats.min, 120);
    assert_eq!(stats.max, 130);
}

#[test]
fn test_get_statistics_time_and_category_filter() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &200,
        &symbol_short!("emer"),
        &1700001000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &130,
        &symbol_short!("vitals"),
        &1700050000,
        &None,
    );

    // Vitals category + only first time window
    let stats = client.get_statistics(
        &symbol_short!("bp"),
        &1699999999,
        &1700001001,
        &Some(symbol_short!("vitals")),
    );
    assert_eq!(stats.count, 1);
    assert_eq!(stats.sum, 120);
}

#[test]
fn test_get_statistics_invalid_time_range() {
    let (_env, client) = setup();

    let result = client.try_get_statistics(&symbol_short!("bp"), &1700001000, &1700000000, &None);
    assert!(result.is_err());
}

#[test]
fn test_get_statistics_no_data() {
    let (_env, client) = setup();

    let result = client.try_get_statistics(&symbol_short!("bp"), &1700000000, &1700001000, &None);
    assert!(result.is_err());
}

#[test]
fn test_get_statistics_no_data_in_range() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );

    // Query a time range that doesn't include the recorded metric
    let result = client.try_get_statistics(&symbol_short!("bp"), &1700100000, &1700200000, &None);
    assert!(result.is_err());
}

#[test]
fn test_get_statistics_single_record() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("hr"),
        &72,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );

    let stats = client.get_statistics(&symbol_short!("hr"), &1699999999, &1700000001, &None);
    assert_eq!(stats.count, 1);
    assert_eq!(stats.sum, 72);
    assert_eq!(stats.average, 72);
    assert_eq!(stats.min, 72);
    assert_eq!(stats.max, 72);
}

#[test]
fn test_get_statistics_exact_boundary_timestamps() {
    let (_env, client) = setup();

    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );

    // Query where start_time == timestamp == end_time
    let stats = client.get_statistics(&symbol_short!("bp"), &1700000000, &1700000000, &None);
    assert_eq!(stats.count, 1);
    assert_eq!(stats.sum, 120);
}

// ========================
// record_quality_metric tests
// ========================

#[test]
fn test_record_quality_metric_basic() {
    let (env, client) = setup();

    let provider = Address::generate(&env);

    client.record_quality_metric(
        &provider,
        &String::from_str(&env, "Infection Rate"),
        &500,
        &202401,
    );

    let metrics = client.get_quality_metrics(&provider, &202401);
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics.get(0).unwrap().value, 500);
    assert_eq!(
        metrics.get(0).unwrap().metric_name,
        String::from_str(&env, "Infection Rate")
    );
}

#[test]
fn test_record_multiple_quality_metrics_same_provider() {
    let (env, client) = setup();

    let provider = Address::generate(&env);

    client.record_quality_metric(
        &provider,
        &String::from_str(&env, "Infection Rate"),
        &500,
        &202401,
    );
    client.record_quality_metric(
        &provider,
        &String::from_str(&env, "Readmission Rate"),
        &300,
        &202401,
    );
    client.record_quality_metric(
        &provider,
        &String::from_str(&env, "Mortality Rate"),
        &100,
        &202401,
    );

    let metrics = client.get_quality_metrics(&provider, &202401);
    assert_eq!(metrics.len(), 3);
}

#[test]
fn test_record_quality_metric_different_periods() {
    let (env, client) = setup();

    let provider = Address::generate(&env);

    client.record_quality_metric(
        &provider,
        &String::from_str(&env, "Infection Rate"),
        &500,
        &202401,
    );
    client.record_quality_metric(
        &provider,
        &String::from_str(&env, "Infection Rate"),
        &450,
        &202402,
    );

    let jan_metrics = client.get_quality_metrics(&provider, &202401);
    assert_eq!(jan_metrics.len(), 1);
    assert_eq!(jan_metrics.get(0).unwrap().value, 500);

    let feb_metrics = client.get_quality_metrics(&provider, &202402);
    assert_eq!(feb_metrics.len(), 1);
    assert_eq!(feb_metrics.get(0).unwrap().value, 450);
}

#[test]
fn test_record_quality_metric_different_providers() {
    let (env, client) = setup();

    let provider_a = Address::generate(&env);
    let provider_b = Address::generate(&env);

    client.record_quality_metric(
        &provider_a,
        &String::from_str(&env, "Safety Score"),
        &900,
        &202401,
    );
    client.record_quality_metric(
        &provider_b,
        &String::from_str(&env, "Safety Score"),
        &850,
        &202401,
    );

    let metrics_a = client.get_quality_metrics(&provider_a, &202401);
    assert_eq!(metrics_a.len(), 1);
    assert_eq!(metrics_a.get(0).unwrap().value, 900);

    let metrics_b = client.get_quality_metrics(&provider_b, &202401);
    assert_eq!(metrics_b.len(), 1);
    assert_eq!(metrics_b.get(0).unwrap().value, 850);
}

// ========================
// get_quality_metrics tests
// ========================

#[test]
fn test_get_quality_metrics_no_data() {
    let (env, client) = setup();

    let provider = Address::generate(&env);

    let result = client.try_get_quality_metrics(&provider, &202401);
    assert!(result.is_err());
}

#[test]
fn test_get_quality_metrics_wrong_period() {
    let (env, client) = setup();

    let provider = Address::generate(&env);

    client.record_quality_metric(
        &provider,
        &String::from_str(&env, "Infection Rate"),
        &500,
        &202401,
    );

    let result = client.try_get_quality_metrics(&provider, &202412);
    assert!(result.is_err());
}

// ========================
// Privacy and aggregation tests
// ========================

#[test]
fn test_privacy_preserving_aggregation() {
    let (_env, client) = setup();

    // Record metrics from different categories (simulating different sources)
    // without any patient-identifying information
    for i in 0..10 {
        client.record_metric(
            &symbol_short!("bmi"),
            &(20 + i as i128),
            &symbol_short!("pop"),
            &(1700000000 + i * 1000),
            &None,
        );
    }

    let stats = client.get_statistics(&symbol_short!("bmi"), &1699999999, &1700010000, &None);

    // Verify aggregation works without individual identification
    assert_eq!(stats.count, 10);
    assert_eq!(stats.min, 20);
    assert_eq!(stats.max, 29);
    assert_eq!(stats.sum, 245);
    assert_eq!(stats.average, 24);
}

#[test]
fn test_multiple_metric_types_aggregation() {
    let (_env, client) = setup();

    // Blood pressure metrics
    client.record_metric(
        &symbol_short!("bp_sys"),
        &120,
        &symbol_short!("cardio"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp_sys"),
        &140,
        &symbol_short!("cardio"),
        &1700001000,
        &None,
    );

    // Heart rate metrics
    client.record_metric(
        &symbol_short!("hr"),
        &72,
        &symbol_short!("cardio"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &symbol_short!("hr"),
        &80,
        &symbol_short!("cardio"),
        &1700001000,
        &None,
    );

    // Lab result metrics
    client.record_metric(
        &symbol_short!("glucose"),
        &95,
        &symbol_short!("lab"),
        &1700000000,
        &None,
    );

    let bp_stats = client.get_statistics(&symbol_short!("bp_sys"), &1699999999, &1700001001, &None);
    assert_eq!(bp_stats.count, 2);
    assert_eq!(bp_stats.average, 130);

    let hr_stats = client.get_statistics(&symbol_short!("hr"), &1699999999, &1700001001, &None);
    assert_eq!(hr_stats.count, 2);
    assert_eq!(hr_stats.average, 76);

    let glucose_stats =
        client.get_statistics(&symbol_short!("glucose"), &1699999999, &1700001001, &None);
    assert_eq!(glucose_stats.count, 1);
    assert_eq!(glucose_stats.average, 95);
}

#[test]
fn test_time_series_support() {
    let (_env, client) = setup();

    // Record metrics across multiple time periods
    let base_time: u64 = 1700000000;
    let day: u64 = 86400;

    // Week 1 metrics
    client.record_metric(
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &base_time,
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &125,
        &symbol_short!("vitals"),
        &(base_time + day),
        &None,
    );

    // Week 2 metrics
    client.record_metric(
        &symbol_short!("bp"),
        &130,
        &symbol_short!("vitals"),
        &(base_time + 7 * day),
        &None,
    );
    client.record_metric(
        &symbol_short!("bp"),
        &135,
        &symbol_short!("vitals"),
        &(base_time + 8 * day),
        &None,
    );

    // Query week 1 only
    let week1 = client.get_statistics(
        &symbol_short!("bp"),
        &base_time,
        &(base_time + 2 * day),
        &None,
    );
    assert_eq!(week1.count, 2);
    assert_eq!(week1.average, 122);

    // Query week 2 only
    let week2 = client.get_statistics(
        &symbol_short!("bp"),
        &(base_time + 7 * day),
        &(base_time + 9 * day),
        &None,
    );
    assert_eq!(week2.count, 2);
    assert_eq!(week2.average, 132);

    // Query all
    let all = client.get_statistics(
        &symbol_short!("bp"),
        &base_time,
        &(base_time + 9 * day),
        &None,
    );
    assert_eq!(all.count, 4);
    assert_eq!(all.average, 127);
}

// ========================
// Resource quota enforcement tests
// ========================

fn setup_with_admin() -> (Env, HealthcareAnalyticsClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn test_request_report_within_quota_accepted() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    // Set a modest budget: 1_000 CPU, 1_000 memory, throttle at 80%
    client.set_resource_limits(&admin, &1_000, &1_000, &5, &80);

    // Request a job well within budget (100 CPU, 100 memory)
    let job_id = client.request_report(
        &requester,
        &String::from_str(&env, "quality_metrics"),
        &JobPriority::Normal,
        &100,
        &100,
    );
    assert!(job_id > 0);
}

#[test]
fn test_request_report_throttled_when_cpu_budget_exceeded() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    // Budget: 1_000 CPU, throttle at 80% → throttles when TotalCpuUsed > 800
    client.set_resource_limits(&admin, &1_000, &1_000_000, &5, &80);

    // Submit and complete a job that consumes 900 CPU (above the 80% threshold)
    let job_id = client.request_report(
        &requester,
        &String::from_str(&env, "adverse_event_report"),
        &JobPriority::High,
        &900,
        &100,
    );
    client.execute_next_report();
    client.complete_report(&job_id, &900, &100);

    // Next request must be throttled: TotalCpuUsed (900) > 80% of 1_000 (800)
    let result = client.try_request_report(
        &requester,
        &String::from_str(&env, "quality_metrics"),
        &JobPriority::Normal,
        &10,
        &10,
    );
    assert_eq!(result, Err(Ok(Error::JobThrottled)));
}

#[test]
fn test_cpu_quota_accumulates_across_jobs() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    // Budget: 1_000 CPU, throttle at 80% → threshold is 800
    client.set_resource_limits(&admin, &1_000, &1_000_000, &5, &80);

    // First job: 400 CPU — below threshold, accepted
    let job1 = client.request_report(
        &requester,
        &String::from_str(&env, "report_a"),
        &JobPriority::Normal,
        &400,
        &50,
    );
    client.execute_next_report();
    client.complete_report(&job1, &400, &50);

    // Second job: another 400 CPU — still below threshold (total 800, not > 800)
    let job2 = client.request_report(
        &requester,
        &String::from_str(&env, "report_b"),
        &JobPriority::Normal,
        &400,
        &50,
    );
    client.execute_next_report();
    client.complete_report(&job2, &400, &50);

    // Now TotalCpuUsed = 800; 800*100/1000 = 80, which is NOT > 80, so one more is still ok.
    // Push it over: complete a tiny job that adds 1 more CPU unit.
    let job3 = client.request_report(
        &requester,
        &String::from_str(&env, "report_c"),
        &JobPriority::Normal,
        &1,
        &1,
    );
    client.execute_next_report();
    client.complete_report(&job3, &1, &1);

    // TotalCpuUsed = 801; 801*100/1000 = 80 (integer), still not > 80.
    // Add one more to make it 901 total.
    let job4 = client.request_report(
        &requester,
        &String::from_str(&env, "report_d"),
        &JobPriority::Normal,
        &100,
        &1,
    );
    client.execute_next_report();
    client.complete_report(&job4, &100, &1);

    // TotalCpuUsed = 901; 901*100/1000 = 90 > 80 → throttled
    let result = client.try_request_report(
        &requester,
        &String::from_str(&env, "report_e"),
        &JobPriority::Normal,
        &10,
        &10,
    );
    assert_eq!(result, Err(Ok(Error::JobThrottled)));
}
