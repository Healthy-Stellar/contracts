use crate::types::{
    AlertThresholds, DataKey, DeviceReading, DeviceRegistration, Error, MonitoringParameters,
    PagedAggResult, PagedRawResult, Range, Severity, VitalAlert, VitalReading, VitalSigns,
    VitalStatistics, VitalsAggregate, WindowIndex, AGG_WINDOW_SECONDS, ALERT_COOLDOWN_SECONDS,
    MAX_ALERTS_PER_VITAL, PAGE_SIZE, RAW_WINDOW_SECONDS,
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
        ttl_config::extend_instance_ttl(&env);
        recorder.require_auth();

        // Authorization check: recorder must be the patient, a provider holding
        // MonitoringParameters for this patient, or a registered monitoring device.
        let is_patient = recorder == patient_id;
        let is_provider = Self::is_authorized_provider(&env, &patient_id, &recorder);
        let is_device = env
            .storage()
            .persistent()
            .has(&DataKey::DeviceAddress(patient_id.clone(), recorder.clone()));

        if !is_patient && !is_provider && !is_device {
            return Err(Error::Unauthorized);
        }

        let reading_id = Self::record_vital_signs_internal(
            &env,
            &patient_id,
            &recorder,
            measurement_time,
            &vitals,
        );

        Ok(reading_id)
    }

    /// Internal function to write vital reading into windowed storage buckets and evaluate thresholds.
    fn record_vital_signs_internal(
        env: &Env,
        patient_id: &Address,
        recorder: &Address,
        measurement_time: u64,
        vitals: &VitalSigns,
    ) -> u64 {
        let reading = VitalReading {
            measurement_time,
            vitals: vitals.clone(),
            recorder: recorder.clone(),
        };

        // Raw windowed storage
        let raw_idx = measurement_time / RAW_WINDOW_SECONDS;
        let raw_key = DataKey::RawWindow(patient_id.clone(), raw_idx);
        let mut raw_bucket: Vec<VitalReading> = env
            .storage()
            .persistent()
            .get(&raw_key)
            .unwrap_or(Vec::new(env));
        raw_bucket.push_back(reading);
        env.storage().persistent().set(&raw_key, &raw_bucket);
        env.storage()
            .persistent()
            .set(&DataKey::LatestRawWindow(patient_id.clone()), &raw_idx);

        // Roll up into daily aggregate
        let agg_idx = measurement_time / AGG_WINDOW_SECONDS;
        Self::update_aggregate(env, patient_id, agg_idx, measurement_time, vitals);

        // Maintain per-patient window index for queries and deregistration
        let win_key = DataKey::PatientWindows(patient_id.clone());
        let mut win_index: WindowIndex = env
            .storage()
            .persistent()
            .get(&win_key)
            .unwrap_or(WindowIndex {
                raw_indices: Vec::new(env),
                agg_indices: Vec::new(env),
            });

        let mut raw_seen = false;
        for i in 0..win_index.raw_indices.len() {
            if win_index.raw_indices.get(i).unwrap_or(u64::MAX) == raw_idx {
                raw_seen = true;
                break;
            }
        }
        if !raw_seen {
            win_index.raw_indices.push_back(raw_idx);
        }

        let mut agg_seen = false;
        for i in 0..win_index.agg_indices.len() {
            if win_index.agg_indices.get(i).unwrap_or(u64::MAX) == agg_idx {
                agg_seen = true;
                break;
            }
        }
        if !agg_seen {
            win_index.agg_indices.push_back(agg_idx);
        }

        env.storage().persistent().set(&win_key, &win_index);

        // Deterministic stable reading ID per patient without unbounded VitalsHistory
        let counter_key = DataKey::ReadingCounter(patient_id.clone());
        let count: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0);
        let reading_id = count + 1;
        env.storage().persistent().set(&counter_key, &reading_id);

        env.events().publish(
            (symbol_short!("vital_rec"), patient_id.clone()),
            (measurement_time, raw_idx, agg_idx),
        );

        // Evaluate all configured thresholds and emit alerts if breached
        Self::evaluate_thresholds(env, patient_id, measurement_time, vitals);

        reading_id
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
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();
        // #728: a MonitoringParameters record grants `provider_id` PHI read access
        // to the patient (see is_authorized_provider). Without the patient's own
        // signature, any address could self-appoint as "provider" for any patient
        // by simply calling this function and authorizing as themselves. Require
        // the patient's consent for every such grant.
        patient_id.require_auth();

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
        device_address: Address,
        device_type: Symbol,
        serial_number: String,
        calibration_date: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        patient_id.require_auth();

        let key = DataKey::DeviceReg(patient_id.clone(), device_id);
        let reg = DeviceRegistration {
            device_type,
            serial_number,
            calibration_date,
        };
        env.storage().persistent().set(&key, &reg);

        // Record the reverse mapping so trigger_vital_alert can verify a device address.
        env.storage()
            .persistent()
            .set(&DataKey::DeviceAddress(patient_id, device_address), &true);

        Ok(())
    }

    pub fn submit_device_reading(
        env: Env,
        device_id: String,
        patient_id: Address,
        device_address: Address,
        _reading_time: u64,
        readings: Vec<DeviceReading>,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        // Verify device is registered
        let device_key = DataKey::DeviceReg(patient_id.clone(), device_id);
        if !env.storage().persistent().has(&device_key) {
            return Err(Error::NotFound); // Device not registered
        }

        // Authorization: require auth from either the device or the patient
        // Primary path: registered device submits on its own authority
        let is_device = env
            .storage()
            .persistent()
            .has(&DataKey::DeviceAddress(patient_id.clone(), device_address.clone()));

        if is_device {
            device_address.require_auth();
        } else {
            // Fallback: patient can submit readings on device's behalf
            patient_id.require_auth();
        }

        for reading in readings.iter() {
            Self::record_vital_signs_internal(
                &env,
                &patient_id,
                &device_address,
                reading.reading_time,
                &reading.values,
            );
        }

        Ok(())
    }

    /// Manually raise an alert for a patient's vital.
    ///
    /// #1121: the alert is stamped with the ledger time (callers can no longer
    /// back- or future-date it), `severity` is a [`Severity`] and `value` is
    /// numeric. The same per-(patient, vital_type) cooldown as the automatic
    /// threshold path applies (`AlertCooldown`), alerts are stored in a capped
    /// ring buffer, and a `man_alert` event (ids and severity only) is emitted.
    pub fn trigger_vital_alert(
        env: Env,
        caller: Address,
        patient_id: Address,
        vital_type: Symbol,
        value: u32,
        severity: Severity,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        // Require auth from whichever actor is asserting the alert.
        caller.require_auth();

        // Authorization check: caller must be one of –
        //   1. the patient themselves,
        //   2. an authorized provider (has MonitoringParameters for this vital type), or
        //   3. a registered monitoring device belonging to the patient
        //      (device IDs are Strings, but the device owner is the patient and the
        //       device address is the caller that registered via register_monitoring_device;
        //       we model this as: a DeviceReg entry exists for the (patient, device_id)
        //       where the device registered itself as caller).
        //
        // For devices: since DeviceReg is keyed by (patient_id, device_id: String) and
        // device_id is an opaque String, we check if a MonitoringParams entry exists for
        // the caller as provider (covers case 2) OR caller == patient (case 1).
        // Case 3 is covered by checking for a DeviceReg entry keyed to the caller's
        // address converted to a canonical String — devices register themselves under
        // their own address string via register_monitoring_device.
        let is_patient = caller == patient_id;
        let is_provider = env
            .storage()
            .persistent()
            .get::<_, MonitoringParameters>(&DataKey::MonitoringParams(
                patient_id.clone(),
                vital_type.clone(),
            ))
            .map(|p| p.provider_id == caller)
            .unwrap_or(false);

        // Check device registration: use the DeviceAddress reverse-mapping written by
        // register_monitoring_device to verify that caller is a registered device
        // for this patient.
        let is_device = env
            .storage()
            .persistent()
            .has(&DataKey::DeviceAddress(patient_id.clone(), caller.clone()));

        if !is_patient && !is_provider && !is_device {
            return Err(Error::Unauthorized);
        }

        // Only standard vital types are accepted, so every alert can be
        // enumerated (and purged) by deregister_patient.
        if !Self::standard_vital_types(&env).iter().any(|vt| vt == &vital_type) {
            return Err(Error::InvalidParameter);
        }

        let alert_time = env.ledger().timestamp();

        // Same cooldown as the automatic threshold path.
        let cooldown_key = DataKey::LastAlertTime(patient_id.clone(), vital_type.clone());
        if let Some(last_alert) = env.storage().persistent().get::<_, u64>(&cooldown_key) {
            if alert_time < last_alert.saturating_add(ALERT_COOLDOWN_SECONDS) {
                return Err(Error::AlertCooldown);
            }
        }

        let seq = Self::store_alert(&env, &patient_id, &vital_type, value, severity, alert_time);
        env.storage().persistent().set(&cooldown_key, &alert_time);

        env.events().publish(
            (symbol_short!("man_alert"), patient_id, vital_type),
            (seq, severity),
        );
        Ok(())
    }

    /// #1121: the vital types tracked by this contract.
    fn standard_vital_types(env: &Env) -> [Symbol; 8] {
        [
            Symbol::new(env, "heart_rate"),
            Symbol::new(env, "bp_systolic"),
            Symbol::new(env, "bp_diastolic"),
            Symbol::new(env, "temperature"),
            Symbol::new(env, "respiratory"),
            Symbol::new(env, "oxygen_sat"),
            Symbol::new(env, "blood_glucose"),
            Symbol::new(env, "weight"),
        ]
    }

    /// #1121: write one alert into the (patient, vital_type) ring buffer and
    /// return its sequence number. Constant cost regardless of history size.
    fn store_alert(
        env: &Env,
        patient_id: &Address,
        vital_type: &Symbol,
        value: u32,
        severity: Severity,
        alert_time: u64,
    ) -> u64 {
        let seq_key = DataKey::AlertSeq(patient_id.clone(), vital_type.clone());
        let seq: u64 = env.storage().persistent().get(&seq_key).unwrap_or(0);
        let slot = (seq % MAX_ALERTS_PER_VITAL as u64) as u32;
        env.storage().persistent().set(
            &DataKey::Alert(patient_id.clone(), vital_type.clone(), slot),
            &VitalAlert {
                seq,
                value,
                severity,
                alert_time,
            },
        );
        env.storage().persistent().set(&seq_key, &(seq + 1));
        seq
    }

    pub fn get_vital_trends(
        env: Env,
        requester: Address,
        patient_id: Address,
        _vital_type: Symbol, // In a real system, you'd filter by this
        start_date: u64,
        end_date: u64,
    ) -> Result<Vec<VitalReading>, Error> {
        requester.require_auth();

        // Authorization: requester must be the patient or an authorized provider
        // (i.e., holds a MonitoringParameters entry for any vital type of this patient).
        if requester != patient_id
            && !Self::is_authorized_provider(&env, &patient_id, &requester)
        {
            return Err(Error::Unauthorized);
        }

        let mut trends = Vec::new(&env);
        let win_key = DataKey::PatientWindows(patient_id.clone());
        if let Some(win_index) = env
            .storage()
            .persistent()
            .get::<_, WindowIndex>(&win_key)
        {
            for i in 0..win_index.raw_indices.len() {
                if let Some(idx) = win_index.raw_indices.get(i) {
                    let win_start = idx * RAW_WINDOW_SECONDS;
                    let win_end = (idx + 1) * RAW_WINDOW_SECONDS;
                    if win_start <= end_date && win_end >= start_date {
                        let raw_key = DataKey::RawWindow(patient_id.clone(), idx);
                        if let Some(bucket) = env
                            .storage()
                            .persistent()
                            .get::<_, Vec<VitalReading>>(&raw_key)
                        {
                            for record in bucket.iter() {
                                if record.measurement_time >= start_date && record.measurement_time <= end_date {
                                    trends.push_back(record);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let key = DataKey::VitalsHistory(patient_id.clone());
            if let Some(history) = env.storage().persistent().get::<_, Vec<VitalReading>>(&key) {
                for record in history.iter() {
                    if record.measurement_time >= start_date && record.measurement_time <= end_date {
                        trends.push_back(record);
                    }
                }
            }
        }

        Ok(trends)
    }

    pub fn calculate_vital_statistics(
        env: Env,
        requester: Address,
        patient_id: Address,
        vital_type: Symbol,
        period: u64,
    ) -> Result<VitalStatistics, Error> {
        ttl_config::extend_instance_ttl(&env);
        requester.require_auth();

        // Authorization: requester must be the patient or an authorized provider.
        if requester != patient_id
            && !Self::is_authorized_provider(&env, &patient_id, &requester)
        {
            return Err(Error::Unauthorized);
        }

        let mut valid_count = 0;
        let mut sum = 0;
        let mut max_val = 0;
        let mut min_val = u32::MAX;

        let win_key = DataKey::PatientWindows(patient_id.clone());
        if let Some(win_index) = env
            .storage()
            .persistent()
            .get::<_, WindowIndex>(&win_key)
        {
            for i in 0..win_index.raw_indices.len() {
                if let Some(idx) = win_index.raw_indices.get(i) {
                    let win_end = (idx + 1) * RAW_WINDOW_SECONDS;
                    if win_end >= period {
                        let raw_key = DataKey::RawWindow(patient_id.clone(), idx);
                        if let Some(bucket) = env
                            .storage()
                            .persistent()
                            .get::<_, Vec<VitalReading>>(&raw_key)
                        {
                            for record in bucket.iter() {
                                if record.measurement_time >= period {
                                    if let Some(val) = Self::extract_vital_value(&env, &record.vitals, &vital_type) {
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
                        }
                    }
                }
            }
        } else {
            let key = DataKey::VitalsHistory(patient_id.clone());
            if let Some(history) = env.storage().persistent().get::<_, Vec<VitalReading>>(&key) {
                for record in history.iter() {
                    if record.measurement_time >= period {
                        if let Some(val) = Self::extract_vital_value(&env, &record.vitals, &vital_type) {
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

    /// Check whether `provider` holds a `MonitoringParameters` entry for *any*
    /// vital type belonging to `patient_id`. This is the contract-local definition
    /// of "authorized provider": whoever a patient's provider set monitoring
    /// parameters for is considered authorized to read that patient's vitals.
    fn is_authorized_provider(env: &Env, patient_id: &Address, provider: &Address) -> bool {
        let vital_types = [
            Symbol::new(env, "heart_rate"),
            Symbol::new(env, "bp_systolic"),
            Symbol::new(env, "bp_diastolic"),
            Symbol::new(env, "temperature"),
            Symbol::new(env, "respiratory"),
            Symbol::new(env, "oxygen_sat"),
            Symbol::new(env, "blood_glucose"),
            Symbol::new(env, "weight"),
        ];
        for vt in vital_types.iter() {
            let key = DataKey::MonitoringParams(patient_id.clone(), vt.clone());
            if let Some(params) = env
                .storage()
                .persistent()
                .get::<_, MonitoringParameters>(&key)
            {
                if &params.provider_id == provider {
                    return true;
                }
            }
        }
        false
    }

    /// Evaluate all configured monitoring parameters for a patient against the
    /// supplied vitals. For each vital type that has a MonitoringParameters entry,
    /// check the AlertThresholds and emit a deterministic VitalAlert if a threshold
    /// is breached, subject to a per-(patient, vital_type) cooldown window.
    fn evaluate_thresholds(env: &Env, patient_id: &Address, measurement_time: u64, vitals: &VitalSigns) {
        let vital_types = Self::standard_vital_types(env);

        for vt in vital_types.iter() {
            let params_key = DataKey::MonitoringParams(patient_id.clone(), vt.clone());
            let params: Option<MonitoringParameters> =
                env.storage().persistent().get(&params_key);
            let params = match params {
                Some(p) => p,
                None => continue,
            };

            let value = match Self::extract_vital_value(env, vitals, &vt) {
                Some(v) => v,
                None => continue,
            };

            let severity = Self::classify_severity(value, &params.alert_thresholds);
            let severity = match severity {
                Some(s) => s,
                None => continue, // within normal range
            };

            // Cooldown check: suppress duplicate alerts within ALERT_COOLDOWN_SECONDS.
            let cooldown_key = DataKey::LastAlertTime(patient_id.clone(), vt.clone());
            let last_alert: u64 = env
                .storage()
                .persistent()
                .get(&cooldown_key)
                .unwrap_or(0);

            if measurement_time < last_alert + ALERT_COOLDOWN_SECONDS {
                continue; // still within cooldown window
            }

            // Record the alert in the capped ring buffer (#1121).
            Self::store_alert(env, patient_id, vt, value, severity, measurement_time);
            env.storage().persistent().set(&cooldown_key, &measurement_time);

            env.events().publish(
                (symbol_short!("vt_alert"), patient_id.clone(), vt.clone()),
                (value, severity, measurement_time),
            );
        }
    }

    /// Return the severity for a value against thresholds, or None if in range.
    fn classify_severity(value: u32, t: &AlertThresholds) -> Option<Severity> {
        if let Some(cl) = t.critical_low {
            if value <= cl {
                return Some(Severity::CriticalLow);
            }
        }
        if let Some(ch) = t.critical_high {
            if value >= ch {
                return Some(Severity::CriticalHigh);
            }
        }
        if let Some(l) = t.low {
            if value <= l {
                return Some(Severity::Low);
            }
        }
        if let Some(h) = t.high {
            if value >= h {
                return Some(Severity::High);
            }
        }
        None
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

    /// Retrieve the stored alerts (at most `MAX_ALERTS_PER_VITAL`, oldest
    /// first) for a patient's specific vital type.
    pub fn get_alerts(
        env: Env,
        requester: Address,
        patient_id: Address,
        vital_type: Symbol,
    ) -> Result<Vec<VitalAlert>, Error> {
        requester.require_auth();

        // Authorization: requester must be the patient or an authorized provider.
        if requester != patient_id
            && !Self::is_authorized_provider(&env, &patient_id, &requester)
        {
            return Err(Error::Unauthorized);
        }

        let seq: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::AlertSeq(patient_id.clone(), vital_type.clone()))
            .unwrap_or(0);
        let cap = MAX_ALERTS_PER_VITAL as u64;
        let start = seq.saturating_sub(cap);
        let mut alerts: Vec<VitalAlert> = Vec::new(&env);
        for s in start..seq {
            let slot = (s % cap) as u32;
            if let Some(a) = env.storage().persistent().get::<_, VitalAlert>(&DataKey::Alert(
                patient_id.clone(),
                vital_type.clone(),
                slot,
            )) {
                alerts.push_back(a);
            }
        }
        Ok(alerts)
    }

    /// Remove all vitals state for a deregistered patient.
    ///
    /// Clears:
    /// - `VitalsHistory`
    /// - `LatestRawWindow`
    /// - All `Alert` ring-buffer slots, `AlertSeq` and `LastAlertTime` entries
    ///   for standard vital types
    /// - All `RawWindow` and `AggWindow` bucket entries (enumerated via the
    ///   `PatientWindows` index maintained by `record_vital_signs`)
    /// - The `PatientWindows` index itself
    ///
    /// Callable by the patient themselves.
    pub fn deregister_patient(env: Env, patient_id: Address) {
        ttl_config::extend_instance_ttl(&env);
        patient_id.require_auth();

        env.storage()
            .persistent()
            .remove(&DataKey::VitalsHistory(patient_id.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::LatestRawWindow(patient_id.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::ReadingCounter(patient_id.clone()));

        // #638: Purge all RawWindow and AggWindow bucket entries using the
        // index that was built up incrementally by record_vital_signs.
        let win_key = DataKey::PatientWindows(patient_id.clone());
        if let Some(win_index) = env
            .storage()
            .persistent()
            .get::<_, WindowIndex>(&win_key)
        {
            for i in 0..win_index.raw_indices.len() {
                if let Some(idx) = win_index.raw_indices.get(i) {
                    env.storage()
                        .persistent()
                        .remove(&DataKey::RawWindow(patient_id.clone(), idx));
                }
            }
            for i in 0..win_index.agg_indices.len() {
                if let Some(idx) = win_index.agg_indices.get(i) {
                    env.storage()
                        .persistent()
                        .remove(&DataKey::AggWindow(patient_id.clone(), idx));
                }
            }
        }
        env.storage().persistent().remove(&win_key);

        // Clear alerts for all standard vital types
        let vital_types = Self::standard_vital_types(&env);
        for vt in vital_types.iter() {
            let seq_key = DataKey::AlertSeq(patient_id.clone(), vt.clone());
            let seq: u64 = env.storage().persistent().get(&seq_key).unwrap_or(0);
            let used_slots = seq.min(MAX_ALERTS_PER_VITAL as u64) as u32;
            for slot in 0..used_slots {
                env.storage()
                    .persistent()
                    .remove(&DataKey::Alert(patient_id.clone(), vt.clone(), slot));
            }
            env.storage().persistent().remove(&seq_key);
            env.storage()
                .persistent()
                .remove(&DataKey::LastAlertTime(patient_id.clone(), vt.clone()));
        }

        env.events().publish(
            (Symbol::new(&env, "pat_dreg"), patient_id),
            Symbol::new(&env, "pv_clean"),
        );
    }
}
