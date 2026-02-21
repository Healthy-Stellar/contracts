#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Vec,
};

#[test]
fn test_record_anonymized_outcome() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    env.mock_all_auths();

    let outcome_type = symbol_short!("surgery");
    let condition = String::from_str(&env, "Appendicitis");
    let treatment = String::from_str(&env, "Appendectomy");
    let result = symbol_short!("success");
    let age_group = symbol_short!("age19_35");
    let gender = symbol_short!("female");
    let timestamp = 1640000000;

    client.record_anonymized_outcome(
        &outcome_type,
        &condition,
        &treatment,
        &result,
        &age_group,
        &gender,
        &timestamp,
    );

    let stats = client.get_population_statistics(&condition, &None, &0);
    assert_eq!(stats.total_cases, 1);
}

#[test]
fn test_record_quality_metric() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let metric_name = String::from_str(&env, "Infection Rate");
    let numerator = 5;
    let denominator = 100;
    let reporting_period = 202401;

    env.mock_all_auths();

    client.record_quality_metric(
        &provider_id,
        &metric_name,
        &numerator,
        &denominator,
        &reporting_period,
    );
}

#[test]
#[should_panic(expected = "Denominator cannot be zero")]
fn test_record_quality_metric_zero_denominator() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let metric_name = String::from_str(&env, "Test Metric");

    env.mock_all_auths();

    client.record_quality_metric(&provider_id, &metric_name, &10, &0, &202401);
}

#[test]
fn test_calculate_provider_scorecard() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let metric_name = String::from_str(&env, "Patient Safety");

    env.mock_all_auths();

    client.record_quality_metric(&provider_id, &metric_name, &90, &100, &202401);

    let mut metrics: Vec<String> = Vec::new(&env);
    metrics.push_back(metric_name.clone());

    let scorecard = client.calculate_provider_scorecard(&provider_id, &metrics, &202401, &202401);

    assert_eq!(scorecard.provider_id, provider_id);
    assert_eq!(scorecard.quality_metrics.len(), 1);
}

#[test]
fn test_get_population_statistics() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    env.mock_all_auths();

    let condition = String::from_str(&env, "Diabetes");

    client.record_anonymized_outcome(
        &symbol_short!("chronic"),
        &condition,
        &String::from_str(&env, "Insulin"),
        &symbol_short!("stable"),
        &symbol_short!("age51_65"),
        &symbol_short!("male"),
        &1640000000,
    );

    client.record_anonymized_outcome(
        &symbol_short!("chronic"),
        &condition,
        &String::from_str(&env, "Metformin"),
        &symbol_short!("stable"),
        &symbol_short!("age36_50"),
        &symbol_short!("female"),
        &1640100000,
    );

    let stats = client.get_population_statistics(&condition, &None, &0);

    assert_eq!(stats.total_cases, 2);
    assert_eq!(stats.condition, condition);
    assert_eq!(stats.gender_distribution.male, 1);
    assert_eq!(stats.gender_distribution.female, 1);
    assert!(stats.average_age > 0);
}

#[test]
fn test_get_population_statistics_with_age_filter() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    env.mock_all_auths();

    let condition = String::from_str(&env, "Hypertension");
    let age_filter = symbol_short!("age51_65");

    client.record_anonymized_outcome(
        &symbol_short!("chronic"),
        &condition,
        &String::from_str(&env, "ACE Inhibitor"),
        &symbol_short!("improved"),
        &age_filter,
        &symbol_short!("male"),
        &1640000000,
    );

    client.record_anonymized_outcome(
        &symbol_short!("chronic"),
        &condition,
        &String::from_str(&env, "Beta Blocker"),
        &symbol_short!("stable"),
        &symbol_short!("age19_35"),
        &symbol_short!("female"),
        &1640100000,
    );

    let stats = client.get_population_statistics(&condition, &Some(age_filter), &0);

    assert_eq!(stats.total_cases, 1);
    assert_eq!(stats.gender_distribution.male, 1);
    assert_eq!(stats.gender_distribution.female, 0);
}

#[test]
fn test_get_population_statistics_with_time_filter() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    env.mock_all_auths();

    let condition = String::from_str(&env, "COVID-19");

    env.ledger().with_mut(|li| {
        li.timestamp = 1640200000;
    });

    client.record_anonymized_outcome(
        &symbol_short!("acute"),
        &condition,
        &String::from_str(&env, "Antiviral"),
        &symbol_short!("recovered"),
        &symbol_short!("age36_50"),
        &symbol_short!("male"),
        &1640150000,
    );

    client.record_anonymized_outcome(
        &symbol_short!("acute"),
        &condition,
        &String::from_str(&env, "Supportive"),
        &symbol_short!("recovered"),
        &symbol_short!("age19_35"),
        &symbol_short!("female"),
        &1639000000,
    );

    let stats = client.get_population_statistics(&condition, &None, &100000);

    assert_eq!(stats.total_cases, 1);
}

#[test]
fn test_track_readmission_rate() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let facility_id = Address::generate(&env);
    let condition = String::from_str(&env, "Heart Failure");
    let days = 30;
    let reporting_period = 202401;

    env.mock_all_auths();

    client.update_readmission_data(
        &facility_id,
        &condition,
        &100,
        &15,
        &days,
        &reporting_period,
    );

    let stats = client.track_readmission_rate(&facility_id, &condition, &days, &reporting_period);

    assert_eq!(stats.facility_id, facility_id);
    assert_eq!(stats.total_admissions, 100);
    assert_eq!(stats.readmissions, 15);
    assert_eq!(stats.readmission_rate, 1500);
    assert_eq!(stats.days, 30);
}

#[test]
fn test_track_readmission_rate_no_data() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let facility_id = Address::generate(&env);
    let condition = String::from_str(&env, "Pneumonia");

    env.mock_all_auths();

    let stats = client.track_readmission_rate(&facility_id, &condition, &30, &202401);

    assert_eq!(stats.total_admissions, 0);
    assert_eq!(stats.readmissions, 0);
    assert_eq!(stats.readmission_rate, 0);
}

#[test]
fn test_record_patient_satisfaction() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let visit_id = 12345;
    let satisfaction_score = 85;

    env.mock_all_auths();

    client.record_patient_satisfaction(&visit_id, &patient_id, &satisfaction_score, &None);
}

#[test]
fn test_record_patient_satisfaction_with_feedback() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let visit_id = 12346;
    let satisfaction_score = 95;
    let feedback_hash: BytesN<32> = BytesN::from_array(&env, &[1; 32]);

    env.mock_all_auths();

    client.record_patient_satisfaction(
        &visit_id,
        &patient_id,
        &satisfaction_score,
        &Some(feedback_hash),
    );
}

#[test]
#[should_panic(expected = "Satisfaction score must be 0-100")]
fn test_record_patient_satisfaction_invalid_score() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);

    env.mock_all_auths();

    client.record_patient_satisfaction(&12347, &patient_id, &150, &None);
}

#[test]
fn test_generate_compliance_report() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let compliance_type = symbol_short!("HIPAA");
    let period = 202401;

    env.mock_all_auths();

    client.update_compliance_data(&provider_id, &compliance_type, &period, &95, &100);

    let report = client.generate_compliance_report(&provider_id, &compliance_type, &period);

    assert_eq!(report.provider_id, provider_id);
    assert_eq!(report.compliance_type, compliance_type);
    assert_eq!(report.compliant_cases, 95);
    assert_eq!(report.total_cases, 100);
    assert_eq!(report.compliance_rate, 9500);
}

#[test]
fn test_generate_compliance_report_no_data() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let compliance_type = symbol_short!("quality");

    env.mock_all_auths();

    let report = client.generate_compliance_report(&provider_id, &compliance_type, &202401);

    assert_eq!(report.compliant_cases, 0);
    assert_eq!(report.total_cases, 0);
    assert_eq!(report.compliance_rate, 0);
}

#[test]
fn test_benchmark_performance() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let metric = String::from_str(&env, "Patient Safety");
    let peer_group = symbol_short!("hospital");

    env.mock_all_auths();

    let period = env.ledger().timestamp() / 86400;
    client.record_quality_metric(&provider_id, &metric, &95, &100, &period);

    client.update_benchmark_data(&peer_group, &metric, &8000, &8200);

    let benchmark = client.benchmark_performance(&provider_id, &metric, &peer_group);

    assert_eq!(benchmark.provider_id, provider_id);
    assert_eq!(benchmark.peer_group, peer_group);
    assert_eq!(benchmark.peer_average, 8000);
    assert_eq!(benchmark.peer_median, 8200);
    assert!(benchmark.percentile > 0);
}

#[test]
fn test_benchmark_performance_no_provider_data() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let metric = String::from_str(&env, "Quality Measure");
    let peer_group = symbol_short!("clinic");

    env.mock_all_auths();

    let benchmark = client.benchmark_performance(&provider_id, &metric, &peer_group);

    assert_eq!(benchmark.provider_value, 0);
}

#[test]
fn test_link_satisfaction_to_provider() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let visit_id = 99999;
    let satisfaction_score = 90;

    env.mock_all_auths();

    client.record_patient_satisfaction(&visit_id, &patient_id, &satisfaction_score, &None);
    client.link_satisfaction_to_provider(&provider_id, &visit_id);
}

#[test]
#[should_panic(expected = "Satisfaction record not found")]
fn test_link_satisfaction_to_provider_no_record() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);

    env.mock_all_auths();

    client.link_satisfaction_to_provider(&provider_id, &99998);
}

#[test]
fn test_calculate_provider_scorecard_with_satisfaction() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let metric_name = String::from_str(&env, "Care Quality");

    env.mock_all_auths();

    client.record_quality_metric(&provider_id, &metric_name, &85, &100, &202401);

    client.record_patient_satisfaction(&10001, &patient_id, &90, &None);
    client.link_satisfaction_to_provider(&provider_id, &10001);

    client.record_patient_satisfaction(&10002, &patient_id, &80, &None);
    client.link_satisfaction_to_provider(&provider_id, &10002);

    let mut metrics: Vec<String> = Vec::new(&env);
    metrics.push_back(metric_name);

    let scorecard = client.calculate_provider_scorecard(&provider_id, &metrics, &202401, &202401);

    assert_eq!(scorecard.provider_id, provider_id);
    assert_eq!(scorecard.patient_satisfaction, 85);
}

#[test]
fn test_multiple_anonymized_outcomes() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    env.mock_all_auths();

    let condition = String::from_str(&env, "Stroke");

    for i in 0..5 {
        client.record_anonymized_outcome(
            &symbol_short!("acute"),
            &condition,
            &String::from_str(&env, "Thrombolysis"),
            &symbol_short!("improved"),
            &symbol_short!("age51_65"),
            &symbol_short!("male"),
            &(1640000000 + i * 1000),
        );
    }

    let stats = client.get_population_statistics(&condition, &None, &0);
    assert_eq!(stats.total_cases, 5);
}

#[test]
fn test_multiple_quality_metrics_for_provider() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);

    env.mock_all_auths();

    let mut metric_vec: Vec<String> = Vec::new(&env);
    metric_vec.push_back(String::from_str(&env, "Mortality Rate"));
    metric_vec.push_back(String::from_str(&env, "Complication Rate"));
    metric_vec.push_back(String::from_str(&env, "Patient Safety"));

    for i in 0..metric_vec.len() {
        let metric = metric_vec.get(i).unwrap();
        client.record_quality_metric(&provider_id, &metric, &92, &100, &202401);
    }

    let scorecard =
        client.calculate_provider_scorecard(&provider_id, &metric_vec, &202401, &202401);
    assert_eq!(scorecard.quality_metrics.len(), 3);
}

#[test]
fn test_outcome_distribution_tracking() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    env.mock_all_auths();

    let condition = String::from_str(&env, "Surgery");

    client.record_anonymized_outcome(
        &symbol_short!("elective"),
        &condition,
        &String::from_str(&env, "Procedure A"),
        &symbol_short!("success"),
        &symbol_short!("age36_50"),
        &symbol_short!("female"),
        &1640000000,
    );

    client.record_anonymized_outcome(
        &symbol_short!("elective"),
        &condition,
        &String::from_str(&env, "Procedure A"),
        &symbol_short!("success"),
        &symbol_short!("age36_50"),
        &symbol_short!("male"),
        &1640001000,
    );

    client.record_anonymized_outcome(
        &symbol_short!("elective"),
        &condition,
        &String::from_str(&env, "Procedure A"),
        &symbol_short!("complica"),
        &symbol_short!("age51_65"),
        &symbol_short!("male"),
        &1640002000,
    );

    let stats = client.get_population_statistics(&condition, &None, &0);

    assert_eq!(stats.total_cases, 3);
    assert_eq!(stats.outcome_distribution.len(), 2);
}

#[test]
fn test_treatment_tracking() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    env.mock_all_auths();

    let condition = String::from_str(&env, "Infection");

    let mut treatments: Vec<String> = Vec::new(&env);
    treatments.push_back(String::from_str(&env, "Antibiotic A"));
    treatments.push_back(String::from_str(&env, "Antibiotic B"));
    treatments.push_back(String::from_str(&env, "Antibiotic A"));
    treatments.push_back(String::from_str(&env, "Antibiotic C"));

    for i in 0..treatments.len() {
        let treatment = treatments.get(i).unwrap();
        client.record_anonymized_outcome(
            &symbol_short!("acute"),
            &condition,
            &treatment,
            &symbol_short!("cured"),
            &symbol_short!("age19_35"),
            &symbol_short!("female"),
            &(1640000000 + i as u64 * 1000),
        );
    }

    let stats = client.get_population_statistics(&condition, &None, &0);

    assert_eq!(stats.total_cases, 4);
    assert_eq!(stats.common_treatments.len(), 3);
}

#[test]
fn test_empty_population_statistics() {
    let env = Env::default();
    let contract_id = env.register(HealthcareAnalytics, ());
    let client = HealthcareAnalyticsClient::new(&env, &contract_id);

    let condition = String::from_str(&env, "Rare Disease");

    let stats = client.get_population_statistics(&condition, &None, &0);

    assert_eq!(stats.total_cases, 0);
    assert_eq!(stats.average_age, 0);
    assert_eq!(stats.gender_distribution.male, 0);
    assert_eq!(stats.gender_distribution.female, 0);
    assert_eq!(stats.gender_distribution.other, 0);
}
