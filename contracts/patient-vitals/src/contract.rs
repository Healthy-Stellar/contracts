use crate::types::{
    AlertThresholds, DataKey, DeviceReading, DeviceRegistration, Error, MonitoringParameters,
    PagedAggResult, PagedRawResult, Range, VitalAlert, VitalReading, VitalSigns, VitalStatistics,
    VitalsAggregate, AGG_WINDOW_SECONDS, PAGE_SIZE, RAW_WINDOW_SECONDS,
};
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, String, Symbol, Vec};

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
        recorder: Address,
        measurement_time: u64,
        vitals: VitalSigns,
    ) -> Result<u64, Error> {
        recorder.require_auth();

        let reading = VitalReading {
            measurement_time,
            vitals: vitals.clone(),
            recorder,
        };

        // --- raw windowed storage ---
        let raw_idx = measurement_time / RAW_WINDOW_SECONDS;
        let raw_key = DataKey::RawWindow(patient_id.clone(), raw_idx);
        let mut raw_bucket: Vec<VitalReading> = env
            .storage()
            .persistent()
            .get(&raw_key)
            .unwrap_or(Vec::new(&env));
        raw_bucket.push_back(reading.clone());
        env.storage().persistent().set(&raw_key, &raw_bucket);
        env.storage()
            .persistent()
            .set(&DataKey::LatestRawWindow(patient_id.clone()), &raw_idx);

        // --- roll up into daily aggregate ---
        let agg_idx = measurement_time / AGG_WINDOW_SECONDS;
        Self::update_aggregate(&env, &patient_id, agg_idx, measurement_time, &vitals);

        // --- legacy flat history (kept for backward compat) ---
        let key = DataKey::VitalsHistory(patient_id.clone());
        let mut history: Vec<VitalReading> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        history.push_back(reading);
        env.storage().persistent().set(&key, &history);

        env.events().publish(
            (symbol_short!("vital_rec"), patient_id),
            (measurement_time, raw_idx, agg_idx),
        );

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
        device_signer: Address,
        calibration_expiry: u64,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        // Prevent a device signer from being bound to more than one patient
        let bound_key = DataKey::DeviceSignerBound(device_signer.clone());
        if env.storage().persistent().has(&bound_key) {
            return Err(Error::Unauthorized);
        }
        env.storage().persistent().set(&bound_key, &patient_id);

        let key = DataKey::DeviceReg(patient_id, device_id);
        let reg = DeviceRegistration {
            device_type,
            serial_number,
            calibration_date,
            device_signer,
            calibration_expiry,
        };

        env.storage().persistent().set(&key, &reg);
        Ok(())
    }

    pub fn submit_device_reading(
        env: Env,
        device_id: String,
        patient_id: Address,
        reading_time: u64,
        readings: Vec<DeviceReading>,
    ) -> Result<(), Error> {
        // Load the device registration — rejects unregistered devices
        let device_key = DataKey::DeviceReg(patient_id.clone(), device_id);
        let reg: DeviceRegistration = env
            .storage()
            .persistent()
            .get(&device_key)
            .ok_or(Error::NotFound)?;

        // Only the registered device signer may submit readings
        reg.device_signer.require_auth();

        // Reject expired calibration
        if reading_time > reg.calibration_expiry {
            return Err(Error::CalibrationExpired);
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
                recorder: reg.device_signer.clone(),
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

    // ── Retention / archival helpers ─────────────────────────────────────────

    fn update_aggregate(env: &Env, patient_id: &Address, agg_idx: u64, ts: u64, v: &VitalSigns) {
        let key = DataKey::AggWindow(patient_id.clone(), agg_idx);
        let mut agg: VitalsAggregate = env.storage().persistent().get(&key).unwrap_or(
            VitalsAggregate {
                window_start: agg_idx * AGG_WINDOW_SECONDS,
                window_end: (agg_idx + 1) * AGG_WINDOW_SECONDS - 1,
                count: 0,
                min_heart_rate: None,
                max_heart_rate: None,
                avg_heart_rate: None,
                min_systolic: None,
                max_systolic: None,
                avg_systolic: None,
                min_oxygen_sat: None,
                max_oxygen_sat: None,
                avg_oxygen_sat: None,
            },
        );
        let _ = ts; // window bounds already encode time
        agg.count += 1;
        Self::merge_u32_stat(&mut agg.min_heart_rate, &mut agg.max_heart_rate, &mut agg.avg_heart_rate, v.heart_rate, agg.count);
        Self::merge_u32_stat(&mut agg.min_systolic, &mut agg.max_systolic, &mut agg.avg_systolic, v.blood_pressure_systolic, agg.count);
        Self::merge_u32_stat(&mut agg.min_oxygen_sat, &mut agg.max_oxygen_sat, &mut agg.avg_oxygen_sat, v.oxygen_saturation, agg.count);
        env.storage().persistent().set(&key, &agg);
    }

    fn merge_u32_stat(min: &mut Option<u32>, max: &mut Option<u32>, avg: &mut Option<u32>, val: Option<u32>, count: u32) {
        if let Some(v) = val {
            *min = Some(min.map_or(v, |m| if v < m { v } else { m }));
            *max = Some(max.map_or(v, |m| if v > m { v } else { m }));
            let prev_avg = avg.unwrap_or(v);
            *avg = Some((prev_avg * (count - 1) + v) / count);
        }
    }

    /// Paged retrieval of raw readings from a specific hourly window.
    /// `page` is 0-based; returns up to PAGE_SIZE readings.
    pub fn get_raw_window_page(
        env: Env,
        patient_id: Address,
        window_idx: u64,
        page: u32,
    ) -> Result<PagedRawResult, Error> {
        let key = DataKey::RawWindow(patient_id, window_idx);
        let all: Vec<VitalReading> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        let start = (page * PAGE_SIZE) as u32;
        if start > 0 && start >= all.len() {
            return Err(Error::InvalidPage);
        }
        let mut readings = Vec::new(&env);
        let end = (start + PAGE_SIZE).min(all.len());
        for i in start..end {
            if let Some(reading) = all.get(i) {
                readings.push_back(reading);
            }
        }
        let next_page = if end < all.len() { Some(page + 1) } else { None };
        Ok(PagedRawResult { readings, next_page })
    }

    /// Paged retrieval of daily aggregates.
    /// `from_agg_idx` and `to_agg_idx` are inclusive day-window indices.
    pub fn get_aggregate_page(
        env: Env,
        patient_id: Address,
        from_agg_idx: u64,
        to_agg_idx: u64,
        page: u32,
    ) -> Result<PagedAggResult, Error> {
        let total_windows = if to_agg_idx >= from_agg_idx {
            (to_agg_idx - from_agg_idx + 1) as u32
        } else {
            return Err(Error::InvalidParameter);
        };
        let start = page * PAGE_SIZE;
        if start > 0 && start >= total_windows {
            return Err(Error::InvalidPage);
        }
        let end = (start + PAGE_SIZE).min(total_windows);
        let mut aggregates = Vec::new(&env);
        for i in start..end {
            let idx = from_agg_idx + i as u64;
            let key = DataKey::AggWindow(patient_id.clone(), idx);
            if let Some(agg) = env.storage().persistent().get::<_, VitalsAggregate>(&key) {
                aggregates.push_back(agg);
            }
        }
        let next_page = if end < total_windows { Some(page + 1) } else { None };
        Ok(PagedAggResult { aggregates, next_page })
    }
}
