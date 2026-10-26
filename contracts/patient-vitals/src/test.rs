#![cfg(test)]
#![allow(deprecated)]

use crate::contract::{PatientVitalsContract, PatientVitalsContractClient};
use crate::types::{AlertThresholds, DeviceReading, Range, VitalSigns, ALERT_COOLDOWN_SECONDS};
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    Address, Env, IntoVal, String, Symbol, Vec,
};

#[test]
fn test_record_vital_signs() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let vitals = VitalSigns {
        blood_pressure_systolic: Some(120),
        blood_pressure_diastolic: Some(80),
        heart_rate: Some(72),
        temperature: Some(366), // 36.6 C
        respiratory_rate: Some(16),
        oxygen_saturation: Some(98),
        blood_glucose: None,
        weight: Some(70000), // 70 kg
    };

    // Provider must be authorized for the patient to read trends.
    // Set monitoring params so provider_id is recognized as authorized.
    client.set_monitoring_parameters(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "heart_rate"),
        &Range { min: 60, max: 100 },
        &AlertThresholds { critical_low: None, low: None, high: None, critical_high: None },
        &3600,
    );

    let result = client.record_vital_signs(&patient_id, &provider_id, &1672531200, &vitals);
    assert_eq!(result, 1);

    // Patient reads their own trends.
    let trends = client.get_vital_trends(
        &patient_id,
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
        &1672531100,
        &1672531300,
    );
    assert_eq!(trends.len(), 1);
    assert_eq!(trends.get(0).unwrap().vitals.heart_rate, Some(72));

    // Authorized provider can also read.
    let trends_provider = client.get_vital_trends(
        &provider_id,
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
        &1672531100,
        &1672531300,
    );
    assert_eq!(trends_provider.len(), 1);
}

#[test]
fn test_set_monitoring_parameters() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let target_range = Range { min: 60, max: 100 };
    let alert_thresholds = AlertThresholds {
        critical_low: Some(40),
        low: Some(50),
        high: Some(110),
        critical_high: Some(130),
    };

    client.set_monitoring_parameters(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "heart_rate"),
        &target_range,
        &alert_thresholds,
        &3600,
    );
}

#[test]
fn test_device_registration_and_reading() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let device_id = String::from_str(&env, "DEVICE_123");
    let device_address = Address::generate(&env);

    client.register_monitoring_device(
        &patient_id,
        &device_id,
        &device_address,
        &Symbol::new(&env, "watch"),
        &String::from_str(&env, "SN-456"),
        &1670000000,
    );

    let mut readings = Vec::new(&env);
    readings.push_back(DeviceReading {
        reading_time: 1672531200,
        values: VitalSigns {
            blood_pressure_systolic: None,
            blood_pressure_diastolic: None,
            heart_rate: Some(75),
            temperature: None,
            respiratory_rate: None,
            oxygen_saturation: None,
            blood_glucose: None,
            weight: None,
        },
    });

    client.submit_device_reading(&device_id, &patient_id, &device_address, &1672531200, &readings);

    // Verify trends to see the reading was added
    let trends =
        client.get_vital_trends(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"), &0, &u64::MAX);
    assert_eq!(trends.len(), 1);
    assert_eq!(trends.get(0).unwrap().vitals.heart_rate, Some(75));
}

#[test]
fn test_submit_device_reading_device_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let device_id = String::from_str(&env, "DEVICE_123");
    let device_address = Address::generate(&env);

    // Register the device
    client.register_monitoring_device(
        &patient_id,
        &device_id,
        &device_address,
        &Symbol::new(&env, "watch"),
        &String::from_str(&env, "SN-456"),
        &1670000000,
    );

    let mut readings = Vec::new(&env);
    readings.push_back(DeviceReading {
        reading_time: 1672531200,
        values: VitalSigns {
            blood_pressure_systolic: None,
            blood_pressure_diastolic: None,
            heart_rate: Some(75),
            temperature: None,
            respiratory_rate: None,
            oxygen_saturation: None,
            blood_glucose: None,
            weight: None,
        },
    });

    // Device submits via its own auth (not patient auth)
    client.submit_device_reading(&device_id, &patient_id, &device_address, &1672531200, &readings);

    // Verify the reading was added
    let trends =
        client.get_vital_trends(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"), &0, &u64::MAX);
    assert_eq!(trends.len(), 1);
    assert_eq!(trends.get(0).unwrap().vitals.heart_rate, Some(75));
    // Recorder should be the device address
    assert_eq!(trends.get(0).unwrap().recorder, device_address);
}

#[test]
fn test_submit_device_reading_patient_fallback() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let device_id = String::from_str(&env, "DEVICE_123");
    let device_address = Address::generate(&env);

    // Register the device
    client.register_monitoring_device(
        &patient_id,
        &device_id,
        &device_address,
        &Symbol::new(&env, "watch"),
        &String::from_str(&env, "SN-456"),
        &1670000000,
    );

    let mut readings = Vec::new(&env);
    readings.push_back(DeviceReading {
        reading_time: 1672531200,
        values: VitalSigns {
            blood_pressure_systolic: None,
            blood_pressure_diastolic: None,
            heart_rate: Some(75),
            temperature: None,
            respiratory_rate: None,
            oxygen_saturation: None,
            blood_glucose: None,
            weight: None,
        },
    });

    // Patient can submit on device's behalf via fallback auth
    client.submit_device_reading(&device_id, &patient_id, &device_address, &1672531200, &readings);

    // Verify the reading was added
    let trends =
        client.get_vital_trends(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"), &0, &u64::MAX);
    assert_eq!(trends.len(), 1);
}

#[test]
fn test_submit_device_reading_unregistered_device_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let device_id = String::from_str(&env, "DEVICE_123");
    let unregistered_device = Address::generate(&env);

    let mut readings = Vec::new(&env);
    readings.push_back(DeviceReading {
        reading_time: 1672531200,
        values: VitalSigns {
            blood_pressure_systolic: None,
            blood_pressure_diastolic: None,
            heart_rate: Some(75),
            temperature: None,
            respiratory_rate: None,
            oxygen_saturation: None,
            blood_glucose: None,
            weight: None,
        },
    });

    // Attempt to submit with an unregistered device should fail
    let result = client.try_submit_device_reading(&device_id, &patient_id, &unregistered_device, &1672531200, &readings);
    assert_eq!(result, Err(Ok(crate::types::Error::NotFound)));
}

#[test]
fn test_trigger_vital_alert() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);

    // Patient triggers their own alert.
    client.trigger_vital_alert(
        &patient_id,
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
        &String::from_str(&env, "135"),
        &Symbol::new(&env, "critical_hi"),
        &1672531200,
    );
}

#[test]
fn test_calculate_vital_statistics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    // Insert multiple readings
    let mut vitals = VitalSigns {
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        heart_rate: Some(70),
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    client.record_vital_signs(&patient_id, &patient_id, &1000, &vitals);

    vitals.heart_rate = Some(80);
    client.record_vital_signs(&patient_id, &patient_id, &2000, &vitals);

    vitals.heart_rate = Some(90);
    client.record_vital_signs(&patient_id, &patient_id, &3000, &vitals);

    // Test stats calculating heart rate from time 1500
    let stats =
        client.calculate_vital_statistics(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"), &1500);
    assert_eq!(stats.count, 2);
    assert_eq!(stats.min_value, 80);
    assert_eq!(stats.max_value, 90);
    assert_eq!(stats.average_value, 85);
}

// ── Threshold evaluation & alert deduplication ───────────────────────────────

fn setup_heart_rate_thresholds(client: &PatientVitalsContractClient, patient_id: &Address, env: &Env) {
    let provider_id = Address::generate(env);
    client.set_monitoring_parameters(
        patient_id,
        &provider_id,
        &Symbol::new(env, "heart_rate"),
        &Range { min: 60, max: 100 },
        &AlertThresholds {
            critical_low: Some(40),
            low: Some(50),
            high: Some(110),
            critical_high: Some(130),
        },
        &3600,
    );
}

/// A reading that breaches the high threshold must create exactly one alert
/// with the correct severity.
#[test]
fn test_threshold_breach_creates_alert() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);
    let patient_id = Address::generate(&env);

    setup_heart_rate_thresholds(&client, &patient_id, &env);

    let vitals = VitalSigns {
        heart_rate: Some(120), // above high=110, below critical_high=130 → "high"
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    client.record_vital_signs(&patient_id, &patient_id, &1_000_000, &vitals);

    let alerts = client.get_alerts(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"));
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts.get(0).unwrap().severity, Symbol::new(&env, "high"));
    assert_eq!(alerts.get(0).unwrap().alert_time, 1_000_000);
}

/// A reading that breaches the critical_high threshold must produce a
/// "critical_hi" severity alert.
#[test]
fn test_critical_threshold_severity() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);
    let patient_id = Address::generate(&env);

    setup_heart_rate_thresholds(&client, &patient_id, &env);

    let vitals = VitalSigns {
        heart_rate: Some(135), // >= critical_high=130
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    client.record_vital_signs(&patient_id, &patient_id, &2_000_000, &vitals);

    let alerts = client.get_alerts(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"));
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts.get(0).unwrap().severity, Symbol::new(&env, "critical_hi"));
}

/// A normal reading (within range) must not create any alert.
#[test]
fn test_normal_reading_no_alert() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);
    let patient_id = Address::generate(&env);

    setup_heart_rate_thresholds(&client, &patient_id, &env);

    let vitals = VitalSigns {
        heart_rate: Some(75), // within [60, 100]
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    client.record_vital_signs(&patient_id, &patient_id, &3_000_000, &vitals);

    let alerts = client.get_alerts(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"));
    assert_eq!(alerts.len(), 0);
}

/// Two consecutive breaching readings within the cooldown window must produce
/// only one alert (deduplication).
#[test]
fn test_cooldown_suppresses_duplicate_alert() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);
    let patient_id = Address::generate(&env);

    setup_heart_rate_thresholds(&client, &patient_id, &env);

    let vitals = VitalSigns {
        heart_rate: Some(120),
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    let t0: u64 = 5_000_000;
    // First breach — alert created.
    client.record_vital_signs(&patient_id, &patient_id, &t0, &vitals);
    // Second breach within cooldown window — must be suppressed.
    let t1 = t0 + ALERT_COOLDOWN_SECONDS - 1;
    client.record_vital_signs(&patient_id, &patient_id, &t1, &vitals);

    let alerts = client.get_alerts(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"));
    assert_eq!(alerts.len(), 1, "duplicate alert within cooldown must be suppressed");
}

/// A breach after the cooldown window has elapsed must produce a second alert.
#[test]
fn test_alert_after_cooldown_expires() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);
    let patient_id = Address::generate(&env);

    setup_heart_rate_thresholds(&client, &patient_id, &env);

    let vitals = VitalSigns {
        heart_rate: Some(120),
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    let t0: u64 = 6_000_000;
    client.record_vital_signs(&patient_id, &patient_id, &t0, &vitals);
    // After cooldown expires a new alert must be created.
    let t1 = t0 + ALERT_COOLDOWN_SECONDS;
    client.record_vital_signs(&patient_id, &patient_id, &t1, &vitals);

    let alerts = client.get_alerts(&patient_id, &patient_id, &Symbol::new(&env, "heart_rate"));
    assert_eq!(alerts.len(), 2, "new alert must be created after cooldown expires");
    assert_eq!(alerts.get(1).unwrap().alert_time, t1);
}

// ── #638 regression: deregister purges all window buckets ────────────────────

/// Record vitals spanning multiple raw windows and agg windows, deregister,
/// then verify that get_raw_window_page and get_aggregate_page return empty
/// for all previously-written indices.
#[test]
fn test_deregister_purges_raw_and_agg_windows() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PatientVitalsContract);
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let recorder = patient_id.clone();

    let vitals = VitalSigns {
        blood_pressure_systolic: Some(120),
        blood_pressure_diastolic: Some(80),
        heart_rate: Some(72),
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: Some(98),
        blood_glucose: None,
        weight: None,
    };

    // RAW_WINDOW_SECONDS = 3600 (1 h), AGG_WINDOW_SECONDS = 86400 (24 h).
    // Write into two distinct raw windows: t=0 (raw_idx=0) and t=7200 (raw_idx=2).
    let t0: u64 = 0;
    let t1: u64 = 7_200;   // 2 h later → different raw window
    // Write into two distinct agg windows: t=0 (agg_idx=0) and t=90000 (agg_idx=1).
    let t2: u64 = 90_000;  // ~25 h → second agg window

    client.record_vital_signs(&patient_id, &recorder, &t0, &vitals);
    client.record_vital_signs(&patient_id, &recorder, &t1, &vitals);
    client.record_vital_signs(&patient_id, &recorder, &t2, &vitals);

    // Verify data is present before deregistration.
    let raw0 = client.get_raw_window_page(&patient_id, &0u64, &0u32);
    assert!(!raw0.readings.is_empty(), "raw window 0 should have readings before deregister");
    let raw2 = client.get_raw_window_page(&patient_id, &2u64, &0u32);
    assert!(!raw2.readings.is_empty(), "raw window 2 should have readings before deregister");
    let agg0 = client.get_aggregate_page(&patient_id, &0u64, &0u64, &0u32);
    assert!(!agg0.aggregates.is_empty(), "agg window 0 should have an entry before deregister");
    let agg1 = client.get_aggregate_page(&patient_id, &1u64, &1u64, &0u32);
    assert!(!agg1.aggregates.is_empty(), "agg window 1 should have an entry before deregister");

    // Deregister.
    client.deregister_patient(&patient_id);

    // All raw windows must now be empty.
    let raw0_after = client.get_raw_window_page(&patient_id, &0u64, &0u32);
    assert!(
        raw0_after.readings.is_empty(),
        "raw window 0 must be empty after deregister"
    );
    let raw2_after = client.get_raw_window_page(&patient_id, &2u64, &0u32);
    assert!(
        raw2_after.readings.is_empty(),
        "raw window 2 must be empty after deregister"
    );

    // All agg windows must now be empty.
    let agg0_after = client.get_aggregate_page(&patient_id, &0u64, &0u64, &0u32);
    assert!(
        agg0_after.aggregates.is_empty(),
        "agg window 0 must be empty after deregister"
    );
    let agg1_after = client.get_aggregate_page(&patient_id, &1u64, &1u64, &0u32);
    assert!(
        agg1_after.aggregates.is_empty(),
        "agg window 1 must be empty after deregister"
    );
}

/// Deregistering a patient that has never recorded any vitals must succeed
/// without panicking (no PatientWindows entry to remove).
#[test]
fn test_deregister_patient_with_no_vitals() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PatientVitalsContract);
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    // Should not panic even though no data was recorded.
    client.deregister_patient(&patient_id);
}


#[test]
fn test_deregister_patient_clears_vitals_history() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let recorder = patient_id.clone();

    let vitals = VitalSigns {
        blood_pressure_systolic: Some(120),
        blood_pressure_diastolic: Some(80),
        heart_rate: Some(72),
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };

    client.record_vital_signs(&patient_id, &recorder, &1_000_000u64, &vitals);

    // History exists before deregistration
    let trends = client.get_vital_trends(
        &patient_id,
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
        &0u64,
        &u64::MAX,
    );
    assert_eq!(trends.len(), 1);

    client.deregister_patient(&patient_id);

    // History cleared after deregistration
    let trends_after = client.get_vital_trends(
        &patient_id,
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
        &0u64,
        &u64::MAX,
    );
    assert_eq!(trends_after.len(), 0);
}

#[test]
fn test_deregister_patient_clears_alerts() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let vital_type = Symbol::new(&env, "heart_rate");

    client.trigger_vital_alert(
        &patient_id,
        &patient_id,
        &vital_type,
        &String::from_str(&env, "150"),
        &Symbol::new(&env, "high"),
        &1_000_000u64,
    );

    assert_eq!(client.get_alerts(&patient_id, &patient_id, &vital_type).len(), 1);

    client.deregister_patient(&patient_id);

    assert_eq!(client.get_alerts(&patient_id, &patient_id, &vital_type).len(), 0);
}

// ── #614: access control for read-only PHI functions ─────────────────────────

/// An unrelated address must not be able to read another patient's vital trends.
#[test]
fn test_unauthorized_address_cannot_read_vital_trends() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let stranger = Address::generate(&env);
    let provider_id = Address::generate(&env);

    // Record a vital so there is data to attempt to read.
    let vitals = VitalSigns {
        heart_rate: Some(80),
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    client.record_vital_signs(&patient_id, &patient_id, &1_000_000, &vitals);

    // Stranger is not the patient and has no MonitoringParameters entry → Unauthorized.
    let result = client.try_get_vital_trends(
        &stranger,
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
        &0,
        &u64::MAX,
    );
    assert!(result.is_err(), "stranger must not read another patient's vital trends");
}

/// An unrelated address must not be able to read another patient's vital statistics.
#[test]
fn test_unauthorized_address_cannot_read_vital_statistics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let stranger = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let vitals = VitalSigns {
        heart_rate: Some(80),
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    client.record_vital_signs(&patient_id, &patient_id, &1_000_000, &vitals);

    let result = client.try_calculate_vital_statistics(
        &stranger,
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
        &0,
    );
    assert!(result.is_err(), "stranger must not read another patient's vital statistics");
}

/// An unrelated address must not be able to read another patient's alerts.
#[test]
fn test_unauthorized_address_cannot_read_alerts() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let stranger = Address::generate(&env);

    // Give patient a threshold breach so alerts exist.
    setup_heart_rate_thresholds(&client, &patient_id, &env);
    let vitals = VitalSigns {
        heart_rate: Some(120),
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    client.record_vital_signs(&patient_id, &patient_id, &1_000_000, &vitals);

    let result = client.try_get_alerts(
        &stranger,
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
    );
    assert!(result.is_err(), "stranger must not read another patient's alerts");
}

/// An authorized provider (has MonitoringParameters for the patient) CAN read alerts.
#[test]
fn test_authorized_provider_can_read_alerts() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    // Register provider by setting monitoring params.
    client.set_monitoring_parameters(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "heart_rate"),
        &Range { min: 60, max: 100 },
        &AlertThresholds {
            critical_low: Some(40),
            low: Some(50),
            high: Some(110),
            critical_high: Some(130),
        },
        &3600,
    );

    // Breach the threshold.
    let vitals = VitalSigns {
        heart_rate: Some(120),
        blood_pressure_systolic: None,
        blood_pressure_diastolic: None,
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };
    client.record_vital_signs(&patient_id, &provider_id, &1_000_000, &vitals);

    // Provider can read alerts.
    let alerts = client.get_alerts(&provider_id, &patient_id, &Symbol::new(&env, "heart_rate"));
    assert_eq!(alerts.len(), 1, "authorized provider must be able to read patient alerts");
}

// ── #615: trigger_vital_alert authorization ───────────────────────────────────

/// A provider with MonitoringParameters for the patient can trigger an alert
/// on that patient's behalf.
#[test]
fn test_provider_can_trigger_vital_alert() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let vital_type = Symbol::new(&env, "heart_rate");

    // Authorize provider by recording monitoring params.
    client.set_monitoring_parameters(
        &patient_id,
        &provider_id,
        &vital_type,
        &Range { min: 60, max: 100 },
        &AlertThresholds { critical_low: None, low: None, high: None, critical_high: None },
        &3600,
    );

    // Provider triggers an emergency alert on behalf of the patient.
    client.trigger_vital_alert(
        &provider_id,
        &patient_id,
        &vital_type,
        &String::from_str(&env, "200"),
        &Symbol::new(&env, "critical_hi"),
        &2_000_000,
    );

    let alerts = client.get_alerts(&provider_id, &patient_id, &vital_type);
    assert_eq!(alerts.len(), 1, "provider-triggered alert must be recorded");
    assert_eq!(alerts.get(0).unwrap().severity, Symbol::new(&env, "critical_hi"));
}

/// An unrelated third party must not be able to trigger an alert for a patient.
#[test]
fn test_third_party_cannot_trigger_vital_alert() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let stranger = Address::generate(&env);
    let vital_type = Symbol::new(&env, "heart_rate");

    // No MonitoringParameters set for stranger → not an authorized provider.
    let result = client.try_trigger_vital_alert(
        &stranger,
        &patient_id,
        &vital_type,
        &String::from_str(&env, "200"),
        &Symbol::new(&env, "critical_hi"),
        &2_000_000,
    );
    assert!(result.is_err(), "third party must not trigger a vital alert");
}

/// A registered monitoring device can trigger an emergency alert on a patient's behalf.
#[test]
fn test_device_can_trigger_vital_alert() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let device_address = Address::generate(&env);
    let vital_type = Symbol::new(&env, "heart_rate");

    // Register the device for this patient.
    client.register_monitoring_device(
        &patient_id,
        &String::from_str(&env, "WATCH_001"),
        &device_address,
        &Symbol::new(&env, "watch"),
        &String::from_str(&env, "SN-001"),
        &1_000_000,
    );

    // Device triggers an emergency alert.
    client.trigger_vital_alert(
        &device_address,
        &patient_id,
        &vital_type,
        &String::from_str(&env, "185"),
        &Symbol::new(&env, "critical_hi"),
        &4_000_000,
    );

    let alerts = client.get_alerts(&patient_id, &patient_id, &vital_type);
    assert_eq!(alerts.len(), 1, "device-triggered alert must be recorded");
    assert_eq!(alerts.get(0).unwrap().severity, Symbol::new(&env, "critical_hi"));
}

// ── #728: provider self-appointment requires patient consent ─────────────────

/// An attacker must not be able to self-appoint as a patient's provider by
/// calling set_monitoring_parameters(patient_id=victim, provider_id=attacker)
/// and only authorizing as themselves. Without the patient's own signature the
/// call must fail, and the attacker must gain no MonitoringParameters entry
/// (and therefore no PHI read access via get_vital_trends / get_alerts).
#[test]
fn test_attacker_cannot_self_appoint_as_provider() {
    let env = Env::default();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let victim = Address::generate(&env);
    let attacker = Address::generate(&env);
    let vital_type = Symbol::new(&env, "heart_rate");
    let target_range = Range { min: 60, max: 100 };
    let alert_thresholds = AlertThresholds {
        critical_low: Some(40),
        low: Some(50),
        high: Some(110),
        critical_high: Some(130),
    };

    // Attacker signs only for themselves (as provider_id) — the victim never
    // authorizes this call. Real Soroban auth would never satisfy
    // victim.require_auth() here, so the call must be rejected.
    let result = client
        .mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "set_monitoring_parameters",
                args: (
                    &victim,
                    &attacker,
                    &vital_type,
                    &target_range,
                    &alert_thresholds,
                    &3600u32,
                )
                    .into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_set_monitoring_parameters(
            &victim,
            &attacker,
            &vital_type,
            &target_range,
            &alert_thresholds,
            &3600,
        );
    assert!(
        result.is_err(),
        "attacker must not be able to self-appoint as provider without the patient's auth"
    );

    // No MonitoringParameters entry was created, so the attacker gained no PHI
    // read access — get_vital_trends and get_alerts must still reject them.
    // (mock_all_auths is used only to satisfy the requester's own require_auth();
    // the Unauthorized rejection below comes from the application-level
    // is_authorized_provider check, which the failed call above never satisfied.)
    env.mock_all_auths();
    let trends_result = client.try_get_vital_trends(
        &attacker,
        &victim,
        &vital_type,
        &0u64,
        &u64::MAX,
    );
    assert!(
        trends_result.is_err(),
        "attacker must remain unable to read the victim's vital trends"
    );

    let alerts_result = client.try_get_alerts(&attacker, &victim, &vital_type);
    assert!(
        alerts_result.is_err(),
        "attacker must remain unable to read the victim's alerts"
    );
}

/// A patient who consents (both patient and provider authorize) can grant a
/// provider monitoring access — the legitimate counterpart to the attack above.
#[test]
fn test_patient_consent_allows_provider_appointment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let vital_type = Symbol::new(&env, "heart_rate");

    client.set_monitoring_parameters(
        &patient_id,
        &provider_id,
        &vital_type,
        &Range { min: 60, max: 100 },
        &AlertThresholds { critical_low: None, low: None, high: None, critical_high: None },
        &3600,
    );

    // Provider is now authorized to read the patient's alerts/trends.
    let alerts = client.get_alerts(&provider_id, &patient_id, &vital_type);
    assert_eq!(alerts.len(), 0);
}

/// The patient can still trigger their own alert (backward-compatible).
#[test]
fn test_patient_can_trigger_own_vital_alert() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let vital_type = Symbol::new(&env, "heart_rate");

    client.trigger_vital_alert(
        &patient_id,
        &patient_id,
        &vital_type,
        &String::from_str(&env, "150"),
        &Symbol::new(&env, "high"),
        &3_000_000,
    );

    let alerts = client.get_alerts(&patient_id, &patient_id, &vital_type);
    assert_eq!(alerts.len(), 1, "patient must be able to trigger their own alert");
}

#[test]
fn test_issue_1005_record_vital_signs_authorization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PatientVitalsContract, ());
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let stranger = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let device_address = Address::generate(&env);

    let vitals = VitalSigns {
        blood_pressure_systolic: Some(120),
        blood_pressure_diastolic: Some(80),
        heart_rate: Some(72),
        temperature: None,
        respiratory_rate: None,
        oxygen_saturation: None,
        blood_glucose: None,
        weight: None,
    };

    // 1. Unregistered stranger must fail with Unauthorized
    let stranger_res = client.try_record_vital_signs(&patient_id, &stranger, &1_000_000, &vitals);
    assert!(stranger_res.is_err(), "unauthorized recorder must be rejected");

    // 2. Patient recording their own vitals succeeds
    let id1 = client.record_vital_signs(&patient_id, &patient_id, &1_000_000, &vitals);
    assert_eq!(id1, 1, "first reading id must be 1");

    // 3. Provider with MonitoringParameters succeeds
    client.set_monitoring_parameters(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "heart_rate"),
        &Range { min: 60, max: 100 },
        &AlertThresholds { critical_low: None, low: None, high: None, critical_high: None },
        &3600,
    );
    let id2 = client.record_vital_signs(&patient_id, &provider_id, &1_001_000, &vitals);
    assert_eq!(id2, 2, "second reading id must be 2");

    // 4. Registered monitoring device succeeds
    client.register_monitoring_device(
        &patient_id,
        &String::from_str(&env, "PULSE_01"),
        &device_address,
        &Symbol::new(&env, "pulse_ox"),
        &String::from_str(&env, "SN-POX-01"),
        &1_000_000,
    );
    let id3 = client.record_vital_signs(&patient_id, &device_address, &1_002_000, &vitals);
    assert_eq!(id3, 3, "third reading id must be 3");

    // 5. submit_device_reading populates windowed buckets
    let mut dev_readings = Vec::new(&env);
    dev_readings.push_back(DeviceReading {
        reading_time: 1_003_000,
        values: vitals.clone(),
    });
    client.submit_device_reading(
        &String::from_str(&env, "PULSE_01"),
        &patient_id,
        &device_address,
        &1_003_000,
        &dev_readings,
    );

    // Reading appears in raw window page (1_003_000 / 3600 = window index 278)
    let raw_window = client.get_raw_window_page(&patient_id, &(1_003_000 / 3600), &0u32);
    assert!(!raw_window.readings.is_empty(), "device reading must appear in raw window page");
}
