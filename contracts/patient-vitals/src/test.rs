#![cfg(test)]
#![allow(deprecated)]

use crate::contract::{PatientVitalsContract, PatientVitalsContractClient};
use crate::types::{AlertThresholds, DeviceReading, Range, VitalSigns};
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
    let device_signer = Address::generate(&env);
    let device_id = String::from_str(&env, "DEVICE_123");
    let calibration_expiry: u64 = 1672531200 + 3600;

    client.register_monitoring_device(
        &patient_id,
        &device_id,
        &Symbol::new(&env, "watch"),
        &String::from_str(&env, "SN-456"),
        &1670000000,
        &device_signer,
        &calibration_expiry,
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

    let trends =
        client.get_vital_trends(&patient_id, &Symbol::new(&env, "heart_rate"), &0, &u64::MAX);
    assert_eq!(trends.len(), 1);
    assert_eq!(trends.get(0).unwrap().vitals.heart_rate, Some(75));
}

#[test]
fn test_spoofed_device_rejected() {
    let env = Env::default();

    let contract_id = env.register_contract(None, PatientVitalsContract);
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let real_signer = Address::generate(&env);
    let spoofed_signer = Address::generate(&env);
    let device_id = String::from_str(&env, "DEVICE_REAL");
    let calibration_expiry: u64 = 9_999_999_999;

    // Register with real_signer — patient authorises this
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &patient_id,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "register_monitoring_device",
            args: (
                patient_id.clone(),
                device_id.clone(),
                Symbol::new(&env, "watch"),
                String::from_str(&env, "SN-001"),
                1670000000_u64,
                real_signer.clone(),
                calibration_expiry,
            )
                .into_val(&env),
            sub_invokes: &[],
        },
    }]);
    client.register_monitoring_device(
        &patient_id,
        &device_id,
        &Symbol::new(&env, "watch"),
        &String::from_str(&env, "SN-001"),
        &1670000000,
        &real_signer,
        &calibration_expiry,
    );

    // Attempt to submit signed by spoofed_signer — must fail auth
    let mut readings = Vec::new(&env);
    readings.push_back(DeviceReading {
        reading_time: 1_000_000,
        values: VitalSigns {
            blood_pressure_systolic: Some(200),
            blood_pressure_diastolic: None,
            heart_rate: None,
            temperature: None,
            respiratory_rate: None,
            oxygen_saturation: None,
            blood_glucose: None,
            weight: None,
        },
    });

    // Only authorise spoofed_signer — real_signer is NOT authorised
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &spoofed_signer,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "submit_device_reading",
            args: (
                device_id.clone(),
                patient_id.clone(),
                1_000_000_u64,
                readings.clone(),
            )
                .into_val(&env),
            sub_invokes: &[],
        },
    }]);

    let result =
        client.try_submit_device_reading(&device_id, &patient_id, &1_000_000, &readings);
    // Auth check for real_signer fails because only spoofed_signer was mocked
    assert!(result.is_err());
}

#[test]
fn test_expired_calibration_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PatientVitalsContract);
    let client = PatientVitalsContractClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let device_signer = Address::generate(&env);
    let device_id = String::from_str(&env, "DEVICE_EXP");
    let calibration_expiry: u64 = 1_000_000; // expires at t=1_000_000

    client.register_monitoring_device(
        &patient_id,
        &device_id,
        &Symbol::new(&env, "sensor"),
        &String::from_str(&env, "SN-EXP"),
        &900_000,
        &device_signer,
        &calibration_expiry,
    );

    let mut readings = Vec::new(&env);
    readings.push_back(DeviceReading {
        reading_time: 1_000_001, // one second past expiry
        values: VitalSigns {
            blood_pressure_systolic: None,
            blood_pressure_diastolic: None,
            heart_rate: Some(80),
            temperature: None,
            respiratory_rate: None,
            oxygen_saturation: None,
            blood_glucose: None,
            weight: None,
        },
    });

    let result =
        client.try_submit_device_reading(&device_id, &patient_id, &1_000_001, &readings);
    assert_eq!(result, Err(Ok(crate::types::Error::CalibrationExpired)));
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
