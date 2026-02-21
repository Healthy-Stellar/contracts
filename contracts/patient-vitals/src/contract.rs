use crate::types::{
    AlertThresholds, DataKey, DeviceReading, DeviceRegistration, Error, MonitoringParameters,
    Range, VitalAlert, VitalReading, VitalSigns, VitalStatistics,
};
use soroban_sdk::{contract, contractimpl, Address, Env, String, Symbol, Vec};

// Error codes
// 1 = Unauthorized
// 2 = Not Found
// 3 = Invalid Parameter

#[contract]
pub struct PatientVitalsContract;

#[contractimpl]
impl PatientVitalsContract {
    pub fn record_vital_signs(
        env: Env,
        patient_id: Address,
        recorder: Address, // patient, provider, or device
        measurement_time: u64,
        vitals: VitalSigns,
    ) -> Result<u64, Error> {
        recorder.require_auth();

        // Load existing history or create new
        let key = DataKey::VitalsHistory(patient_id.clone());
        let mut history: Vec<VitalReading> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));

        history.push_back(VitalReading {
            measurement_time,
            vitals,
            recorder,
        });

        env.storage().persistent().set(&key, &history);

        // Returning the inserted index / record id
        Ok(history.len() as u64)
    }

    pub fn set_monitoring_parameters(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        vital_type: Symbol,
        target_range: Range,
        alert_thresholds: AlertThresholds,
        monitoring_frequency: u32,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let key = DataKey::MonitoringParams(patient_id, vital_type);
        let params = MonitoringParameters {
            provider_id,
            target_range,
            alert_thresholds,
            monitoring_frequency,
        };

        env.storage().persistent().set(&key, &params);
        Ok(())
    }

    pub fn register_monitoring_device(
        env: Env,
        patient_id: Address,
        device_id: String,
        device_type: Symbol,
        serial_number: String,
        calibration_date: u64,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let key = DataKey::DeviceReg(patient_id, device_id);
        let reg = DeviceRegistration {
            device_type,
            serial_number,
            calibration_date,
        };

        env.storage().persistent().set(&key, &reg);
        Ok(())
    }

    pub fn submit_device_reading(
        env: Env,
        device_id: String,
        patient_id: Address,
        _reading_time: u64,
        readings: Vec<DeviceReading>,
    ) -> Result<(), Error> {
        // Assume device or patient has permission to submit
        patient_id.require_auth();

        // Verify device is registered
        let device_key = DataKey::DeviceReg(patient_id.clone(), device_id);
        if !env.storage().persistent().has(&device_key) {
            return Err(Error::NotFound); // Device not registered
        }

        let key = DataKey::VitalsHistory(patient_id.clone());
        let mut history: Vec<VitalReading> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));

        for reading in readings.iter() {
            history.push_back(VitalReading {
                measurement_time: reading.reading_time,
                vitals: reading.values,
                recorder: patient_id.clone(), // or device address
            });
        }

        env.storage().persistent().set(&key, &history);
        Ok(())
    }

    pub fn trigger_vital_alert(
        env: Env,
        patient_id: Address,
        vital_type: Symbol,
        value: String,
        severity: Symbol,
        alert_time: u64,
    ) -> Result<(), Error> {
        // Can be called by a monitoring service or device with auth
        // To simplify, we ensure patient_id gives auth or anyone can if it's an emergency alert
        // Let's require patient auth or some configured admin
        patient_id.require_auth();

        let key = DataKey::VitalsAlerts(patient_id.clone(), vital_type);
        let mut alerts: Vec<VitalAlert> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));

        alerts.push_back(VitalAlert {
            value,
            severity,
            alert_time,
        });

        env.storage().persistent().set(&key, &alerts);
        Ok(())
    }

    pub fn get_vital_trends(
        env: Env,
        patient_id: Address,
        _vital_type: Symbol, // In a real system, you'd filter by this
        start_date: u64,
        end_date: u64,
    ) -> Result<Vec<VitalReading>, Error> {
        // No auth strict needed if public view, but let's assume public getter
        // that's protected by off-chain or wrapper contract
        let key = DataKey::VitalsHistory(patient_id);
        let history: Vec<VitalReading> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));

        let mut trends = Vec::new(&env);
        for record in history.iter() {
            if record.measurement_time >= start_date && record.measurement_time <= end_date {
                trends.push_back(record.clone());
            }
        }

        Ok(trends)
    }

    pub fn calculate_vital_statistics(
        env: Env,
        patient_id: Address,
        vital_type: Symbol,
        period: u64,
    ) -> Result<VitalStatistics, Error> {
        // Filter by period (e.g. recent `period` seconds from now)
        // Since we don't know "now", we assume `period` is the start_date for statistics calculation.
        let key = DataKey::VitalsHistory(patient_id);
        let history: Vec<VitalReading> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));

        let mut valid_count = 0;
        let mut sum = 0;
        let mut max_val = 0;
        let mut min_val = u32::MAX;

        for record in history.iter() {
            if record.measurement_time >= period {
                let val_opt = Self::extract_vital_value(&env, &record.vitals, &vital_type);

                if let Some(val) = val_opt {
                    valid_count += 1;
                    sum += val;
                    if val > max_val {
                        max_val = val;
                    }
                    if val < min_val {
                        min_val = val;
                    }
                }
            }
        }

        if valid_count == 0 {
            return Ok(VitalStatistics {
                min_value: 0,
                max_value: 0,
                average_value: 0,
                count: 0,
            });
        }

        Ok(VitalStatistics {
            min_value: min_val,
            max_value: max_val,
            average_value: sum / valid_count,
            count: valid_count,
        })
    }

    fn extract_vital_value(env: &Env, vitals: &VitalSigns, vital_type: &Symbol) -> Option<u32> {
        if vital_type == &Symbol::new(env, "heart_rate") {
            return vitals.heart_rate;
        }
        if vital_type == &Symbol::new(env, "bp_systolic") {
            return vitals.blood_pressure_systolic;
        }
        if vital_type == &Symbol::new(env, "bp_diastolic") {
            return vitals.blood_pressure_diastolic;
        }
        if vital_type == &Symbol::new(env, "temperature") {
            return vitals.temperature;
        }
        if vital_type == &Symbol::new(env, "respiratory") {
            return vitals.respiratory_rate;
        }
        if vital_type == &Symbol::new(env, "oxygen_sat") {
            return vitals.oxygen_saturation;
        }
        if vital_type == &Symbol::new(env, "blood_glucose") {
            return vitals.blood_glucose;
        }
        if vital_type == &Symbol::new(env, "weight") {
            return vitals.weight;
        }

        None
    }
}
