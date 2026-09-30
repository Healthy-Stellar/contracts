#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, String};

fn setup() -> (Env, HealthcareAnalyticsClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let recorder = Address::generate(&env);
    client.authorize_provider(&admin, &recorder);
    (env, client, recorder)
}

// ========================
// record_metric tests
// ========================

#[test]
fn test_record_metric_basic() {
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
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
    let (env, client, recorder) = setup();

    let hash: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);

    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &130,
        &symbol_short!("vitals"),
        &1700001000,
        &None,
    );
    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &i128::MAX,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
        &symbol_short!("temp"),
        &-5,
        &symbol_short!("lab"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
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

#[test]
fn test_record_metric_rejects_unregistered_provider() {
    let (env, client, _recorder) = setup();

    let unregistered = Address::generate(&env);

    let result = client.try_record_metric(
        &unregistered,
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );

    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_authorize_provider_rejects_non_admin() {
    let (env, client, admin) = setup_with_admin();

    let non_admin = Address::generate(&env);
    let provider = Address::generate(&env);

    let result = client.try_authorize_provider(&non_admin, &provider);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // The admin path still works, confirming the rejection above was auth-specific.
    client.authorize_provider(&admin, &provider);
    client.record_metric(
        &provider,
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
}

// ========================
// get_statistics tests
// ========================

#[test]
fn test_get_statistics_time_range_filter() {
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &140,
        &symbol_short!("vitals"),
        &1700050000,
        &None,
    );
    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &200,
        &symbol_short!("emer"),
        &1700001000,
        &None,
    );
    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &200,
        &symbol_short!("emer"),
        &1700001000,
        &None,
    );
    client.record_metric(
        &recorder,
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
    let (_env, client, _recorder) = setup();

    let result = client.try_get_statistics(&symbol_short!("bp"), &1700001000, &1700000000, &None);
    assert!(result.is_err());
}

#[test]
fn test_get_statistics_no_data() {
    let (_env, client, _recorder) = setup();

    let result = client.try_get_statistics(&symbol_short!("bp"), &1700000000, &1700001000, &None);
    assert!(result.is_err());
}

#[test]
fn test_get_statistics_no_data_in_range() {
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    client.record_metric(
        &recorder,
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
    let (env, client, _recorder) = setup();

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
    let (env, client, _recorder) = setup();

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
    let (env, client, _recorder) = setup();

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
    let (env, client, _recorder) = setup();

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
    let (env, client, _recorder) = setup();

    let provider = Address::generate(&env);

    let result = client.try_get_quality_metrics(&provider, &202401);
    assert!(result.is_err());
}

#[test]
fn test_get_quality_metrics_wrong_period() {
    let (env, client, _recorder) = setup();

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
    let (_env, client, recorder) = setup();

    // Record metrics from different categories (simulating different sources)
    // without any patient-identifying information
    for i in 0..10 {
        client.record_metric(
            &recorder,
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
    let (_env, client, recorder) = setup();

    // Blood pressure metrics
    client.record_metric(
        &recorder,
        &symbol_short!("bp_sys"),
        &120,
        &symbol_short!("cardio"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
        &symbol_short!("bp_sys"),
        &140,
        &symbol_short!("cardio"),
        &1700001000,
        &None,
    );

    // Heart rate metrics
    client.record_metric(
        &recorder,
        &symbol_short!("hr"),
        &72,
        &symbol_short!("cardio"),
        &1700000000,
        &None,
    );
    client.record_metric(
        &recorder,
        &symbol_short!("hr"),
        &80,
        &symbol_short!("cardio"),
        &1700001000,
        &None,
    );

    // Lab result metrics
    client.record_metric(
        &recorder,
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
    let (_env, client, recorder) = setup();

    // Record metrics across multiple time periods
    let base_time: u64 = 1700000000;
    let day: u64 = 86400;

    // Week 1 metrics
    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &120,
        &symbol_short!("vitals"),
        &base_time,
        &None,
    );
    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &125,
        &symbol_short!("vitals"),
        &(base_time + day),
        &None,
    );

    // Week 2 metrics
    client.record_metric(
        &recorder,
        &symbol_short!("bp"),
        &130,
        &symbol_short!("vitals"),
        &(base_time + 7 * day),
        &None,
    );
    client.record_metric(
        &recorder,
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
// Degradation policy tests
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
fn test_request_report_full_quality_when_not_throttled() {
    let (env, client, _admin) = setup_with_admin();
    let requester = Address::generate(&env);

    let accepted = client
        .request_report(
            &requester,
            &String::from_str(&env, "quality_metrics"),
            &shared::resource_management::JobPriority::Normal,
            &500_000,
            &50_000,
            &DegradationPolicy::Fail,
        );

    assert_eq!(accepted.result_quality, ResultQuality::Full);
    assert_eq!(
        client.get_job_result_quality(&accepted.job_id),
        ResultQuality::Full
    );
}

#[test]
fn test_request_report_fail_policy_when_throttled() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    // throttle_threshold = 0 means always throttled
    client
        .set_resource_limits(&admin, &1, &1, &1, &0);
    env.as_contract(&client.address, || {
        env.storage().instance().set(&shared::resource_management::ResourceKey::TotalCpuUsed, &1u64);
    });

    let result = client.try_request_report(
        &requester,
        &String::from_str(&env, "adverse_event"),
        &shared::resource_management::JobPriority::Normal,
        &1_000_000,
        &100_000,
        &DegradationPolicy::Fail,
    );

    assert_eq!(result, Err(Ok(Error::JobThrottled)));
}

#[test]
fn test_request_report_approximate_policy_when_throttled() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    client
        .set_resource_limits(&admin, &1, &1, &1, &0);
    env.as_contract(&client.address, || {
        env.storage().instance().set(&shared::resource_management::ResourceKey::TotalCpuUsed, &1u64);
    });

    let accepted = client
        .request_report(
            &requester,
            &String::from_str(&env, "quality_metrics"),
            &shared::resource_management::JobPriority::Normal,
            &1_000_000,
            &100_000,
            &DegradationPolicy::Approximate,
        );

    assert_eq!(accepted.result_quality, ResultQuality::Truncated);
    assert_eq!(
        client.get_job_result_quality(&accepted.job_id),
        ResultQuality::Truncated
    );
}

#[test]
fn test_request_report_sample_policy_when_throttled() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    client
        .set_resource_limits(&admin, &1, &1, &1, &0);
    env.as_contract(&client.address, || {
        env.storage().instance().set(&shared::resource_management::ResourceKey::TotalCpuUsed, &1u64);
    });

    let accepted = client
        .request_report(
            &requester,
            &String::from_str(&env, "quality_metrics"),
            &shared::resource_management::JobPriority::Normal,
            &1_000_000,
            &100_000,
            &DegradationPolicy::Sample,
        );

    assert_eq!(accepted.result_quality, ResultQuality::Sampled);
    assert_eq!(
        client.get_job_result_quality(&accepted.job_id),
        ResultQuality::Sampled
    );
}

#[test]
fn test_cpu_quota_accumulates_across_jobs() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    // Budget: 1_000 CPU, throttle at 80% → threshold is 800
    client.set_resource_limits(&admin, &1_000, &1_000_000, &5, &80);

    // First job: 400 CPU — below threshold, accepted
    let job1 = client.try_request_report(
        &requester,
        &String::from_str(&env, "report_a"),
        &JobPriority::Normal,
        &400,
        &50,
        &DegradationPolicy::Fail,
    ).unwrap().unwrap().job_id;
    client.execute_next_report(&admin).unwrap();
    client.complete_report(&job1, &400, &50);

    // Second job: another 400 CPU — still below threshold (total 800, not > 800)
    let job2 = client.try_request_report(
        &requester,
        &String::from_str(&env, "report_b"),
        &JobPriority::Normal,
        &400,
        &50,
        &DegradationPolicy::Fail,
    ).unwrap().unwrap().job_id;
    client.execute_next_report(&admin).unwrap();
    client.complete_report(&job2, &400, &50);

    // Now TotalCpuUsed = 800; 800*100/1000 = 80, which is NOT > 80, so one more is still ok.
    // Push it over: complete a tiny job that adds 1 more CPU unit.
    let job3 = client.try_request_report(
        &requester,
        &String::from_str(&env, "report_c"),
        &JobPriority::Normal,
        &1,
        &1,
        &DegradationPolicy::Fail,
    ).unwrap().unwrap().job_id;
    client.execute_next_report(&admin).unwrap();
    client.complete_report(&job3, &1, &1);

    // TotalCpuUsed = 801; 801*100/1000 = 80 (integer), still not > 80.
    // Add one more to make it 901 total.
    let job4 = client.try_request_report(
        &requester,
        &String::from_str(&env, "report_d"),
        &JobPriority::Normal,
        &100,
        &1,
        &DegradationPolicy::Fail,
    ).unwrap().unwrap().job_id;
    client.execute_next_report();
    client.complete_report(&job4, &100, &1, &50);

    // TotalCpuUsed = 901; 901*100/1000 = 90 > 80 → throttled
    let result = client.try_request_report(
        &requester,
        &String::from_str(&env, "report_e"),
        &JobPriority::Normal,
        &10,
        &10,
        &DegradationPolicy::Fail,
    );
    assert_eq!(result, Err(Ok(Error::JobThrottled)));
}

#[test]
fn test_cancel_report_queued() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    let accepted = client
        .request_report(
            &requester,
            &String::from_str(&env, "quality_metrics"),
            &shared::resource_management::JobPriority::Normal,
            &500_000,
            &50_000,
            &DegradationPolicy::Fail,
        );

    let job_id = accepted.job_id;

    // Only the original requester may cancel
    let non_requester = Address::generate(&env);
    let err_unauthorized = client.try_cancel_report(&non_requester, &job_id);
    assert_eq!(err_unauthorized, Err(Ok(Error::Unauthorized)));

    // Queued job can be cancelled by its requester
    client.cancel_report(&requester, &job_id);

    // Verify it cannot be cancelled again / is no longer in Queued state
    let err_not_found = client.try_cancel_report(&requester, &job_id);
    assert_eq!(err_not_found, Err(Ok(Error::JobNotFound)));

    // Cancelled job ID cannot be re-executed
    let next_job = client.execute_next_report(&admin);
    assert_eq!(next_job, Ok(None));
}

#[test]
fn test_cancel_report_executing() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    let accepted = client
        .request_report(
            &requester,
            &String::from_str(&env, "quality_metrics"),
            &shared::resource_management::JobPriority::Normal,
            &500_000,
            &50_000,
            &DegradationPolicy::Fail,
        );

    let job_id = accepted.job_id;

    // Start executing the job
    let executed_id = client.execute_next_report(&admin).unwrap().unwrap();
    assert_eq!(executed_id, job_id);

    // Executing job cancellation returns Error::JobAlreadyExecuting
    let err_executing = client.try_cancel_report(&requester, &job_id);
    assert_eq!(err_executing, Err(Ok(Error::JobAlreadyExecuting)));
}

// ========================
// execute_next_report auth tests
// ========================

#[test]
fn test_execute_next_report_requires_admin_auth() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    // Queue a report
    client.request_report(
        &requester,
        &String::from_str(&env, "quality_metrics"),
        &shared::resource_management::JobPriority::Normal,
        &500_000,
        &50_000,
        &DegradationPolicy::Fail,
    );

    // Non-admin call should be rejected
    let non_admin = Address::generate(&env);
    let result = client.try_execute_next_report(&non_admin);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_execute_next_report_admin_succeeds() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    // Queue a report
    let accepted = client.request_report(
        &requester,
        &String::from_str(&env, "quality_metrics"),
        &shared::resource_management::JobPriority::Normal,
        &500_000,
        &50_000,
        &DegradationPolicy::Fail,
    );

    // Admin call should succeed and return the job_id
    let result = client.execute_next_report(&admin);
    assert_eq!(result, Ok(Some(accepted.job_id)));
}

// ========================
// set_resource_limits tests
// ========================

#[test]
fn test_set_resource_limits_rejects_zero_cpu_budget() {
    let (env, client, admin) = setup_with_admin();

    let result = client.try_set_resource_limits(&admin, &0, &1_000_000, &5, &80);
    assert_eq!(result, Err(Ok(Error::InvalidValue)));
}

#[test]
fn test_set_resource_limits_rejects_zero_memory_budget() {
    let (env, client, admin) = setup_with_admin();

    let result = client.try_set_resource_limits(&admin, &1_000_000, &0, &5, &80);
    assert_eq!(result, Err(Ok(Error::InvalidValue)));
}

#[test]
fn test_set_resource_limits_rejects_both_zero() {
    let (env, client, admin) = setup_with_admin();

    let result = client.try_set_resource_limits(&admin, &0, &0, &5, &80);
    assert_eq!(result, Err(Ok(Error::InvalidValue)));
}

#[test]
fn test_set_resource_limits_accepts_nonzero_budgets() {
    let (env, client, admin) = setup_with_admin();

    let result = client.try_set_resource_limits(&admin, &1_000_000, &100_000, &5, &80);
    assert_eq!(result, Ok(Ok(())));
}

#[test]
fn test_request_report_throttle_check_no_panic_with_proper_budgets() {
    let (env, client, admin) = setup_with_admin();
    let requester = Address::generate(&env);

    // Set valid non-zero budgets
    client.set_resource_limits(&admin, &10_000, &10_000, &5, &50);

    // should_throttle_job is called internally; it should not panic
    // because the budgets are guaranteed to be non-zero
    let result = client.try_request_report(
        &requester,
        &String::from_str(&env, "quality_metrics"),
        &JobPriority::Normal,
        &100,
        &100,
        &DegradationPolicy::Fail,
    );

    // Should succeed, proving should_throttle_job didn't panic
    assert!(result.is_ok());
}

// ========================
// Admin rotation tests
// ========================

#[test]
fn test_propose_admin_rotation_rejects_non_admin() {
    let (env, client, _admin) = setup_with_admin();

    // A random address that was never set as admin.
    let impostor = Address::generate(&env);
    let new_admin = Address::generate(&env);

    let result = client.try_propose_admin_rotation(&impostor, &new_admin);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_propose_admin_rotation_succeeds_for_admin() {
    let (env, client, admin) = setup_with_admin();
    let new_admin = Address::generate(&env);

    // Current admin should be able to propose a rotation without error.
    client.propose_admin_rotation(&admin, &new_admin);
}

#[test]
fn test_propose_admin_rotation_rejects_duplicate_pending() {
    let (env, client, admin) = setup_with_admin();
    let new_admin_a = Address::generate(&env);
    let new_admin_b = Address::generate(&env);

    client.propose_admin_rotation(&admin, &new_admin_a);

    // A second proposal before the first is accepted or expires must fail.
    let result = client.try_propose_admin_rotation(&admin, &new_admin_b);
    assert_eq!(result, Err(Ok(Error::RotationPending)));
}

#[test]
fn test_expired_rotation_can_be_replaced_without_accept_attempt() {
    let (env, client, admin) = setup_with_admin();
    let typo = Address::generate(&env);
    let replacement = Address::generate(&env);

    client.propose_admin_rotation(&admin, &typo);
    env.ledger().set_timestamp(env.ledger().timestamp() + ADMIN_ROTATION_WINDOW + 1);

    client.propose_admin_rotation(&admin, &replacement);
    client.accept_admin_rotation(&replacement);
}

#[test]
fn test_admin_can_cancel_rotation() {
    let (env, client, admin) = setup_with_admin();
    let pending = Address::generate(&env);
    let replacement = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);
    client.cancel_admin_rotation(&admin);
    client.propose_admin_rotation(&admin, &replacement);
    client.accept_admin_rotation(&replacement);
}

#[test]
fn test_non_admin_cannot_cancel_rotation() {
    let (env, client, admin) = setup_with_admin();
    let pending = Address::generate(&env);
    let impostor = Address::generate(&env);

    client.propose_admin_rotation(&admin, &pending);
    let result = client.try_cancel_admin_rotation(&impostor);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    client.accept_admin_rotation(&pending);
}

#[test]
fn test_accept_admin_rotation_rejects_wrong_pending_admin() {
    let (env, client, admin) = setup_with_admin();
    let new_admin = Address::generate(&env);
    let other = Address::generate(&env);

    client.propose_admin_rotation(&admin, &new_admin);

    // An address that is not the pending admin cannot accept.
    let result = client.try_accept_admin_rotation(&other);
    assert_eq!(result, Err(Ok(Error::NotPendingAdmin)));
}
