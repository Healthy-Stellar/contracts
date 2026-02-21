#![cfg(test)]

use super::*;
use crate::contract::{PatientVitalsContract, PatientVitalsContractClient};
use crate::types::{AlertThresholds, DeviceReading, Range, VitalSigns, VitalStatistics};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Symbol, Vec};

#[test]
fn test_record_vital_signs() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PatientVitalsContract);
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

    let result = client.record_vital_signs(&patient_id, &provider_id, &1672531200, &vitals);
    assert_eq!(result, 1);

    // Test get trends
    let trends = client.get_vital_trends(
        &patient_id,
        &Symbol::new(&env, "heart_rate"),
        &1672531100,
        &1672531300,
    );
    assert_eq!(trends.len(), 1);
    assert_eq!(trends.get(0).unwrap().vitals.heart_rate, Some(72));
}

#[test]
fn test_set_monitoring_parameters() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PatientVitalsContract);
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

    let contract_id = env.register_contract(None, PatientVitalsContract);
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let device_id = String::from_str(&env, "DEVICE_123");

    client.register_monitoring_device(
        &patient_id,
        &device_id,
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

    client.submit_device_reading(&device_id, &patient_id, &1672531200, &readings);

    // Verify trends to see the reading was added
    let trends =
        client.get_vital_trends(&patient_id, &Symbol::new(&env, "heart_rate"), &0, &u64::MAX);
    assert_eq!(trends.len(), 1);
    assert_eq!(trends.get(0).unwrap().vitals.heart_rate, Some(75));
}

#[test]
fn test_trigger_vital_alert() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PatientVitalsContract);
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);

    client.trigger_vital_alert(
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

    let contract_id = env.register_contract(None, PatientVitalsContract);
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
    client.record_vital_signs(&patient_id, &provider_id, &1000, &vitals);

    vitals.heart_rate = Some(80);
    client.record_vital_signs(&patient_id, &provider_id, &2000, &vitals);

    vitals.heart_rate = Some(90);
    client.record_vital_signs(&patient_id, &provider_id, &3000, &vitals);

    // Test stats calculating heart rate from time 1500
    let stats =
        client.calculate_vital_statistics(&patient_id, &Symbol::new(&env, "heart_rate"), &1500);
    assert_eq!(stats.count, 2);
    assert_eq!(stats.min_value, 80);
    assert_eq!(stats.max_value, 90);
    assert_eq!(stats.average_value, 85);
}
