use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::types::{VitalSigns, VitalType};

/// Per-vital running accumulator: sum of all values seen and how many
/// readings actually carried this vital. The average is derived on read
/// so partial readings (e.g. BP-only) never skew other vitals and no
/// per-step integer truncation drift accumulates.
#[contracttype]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VitalStat {
    pub sum: u64,
    pub n: u32,
}

impl VitalStat {
    pub fn add(&mut self, v: u32) {
        self.sum += v as u64;
        self.n += 1;
    }

    pub fn avg(&self) -> Option<u32> {
        if self.n == 0 {
            None
        } else {
            Some((self.sum / self.n as u64) as u32)
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DailyAggregate {
    pub heart_rate: VitalStat,
    pub systolic: VitalStat,
    pub diastolic: VitalStat,
    pub spo2: VitalStat,
    pub temperature: VitalStat,
    pub respiratory_rate: VitalStat,
    pub count: u32,
}

impl DailyAggregate {
    pub fn record(&mut self, v: &VitalSigns) {
        if let Some(x) = v.heart_rate {
            self.heart_rate.add(x);
        }
        if let Some(x) = v.systolic {
            self.systolic.add(x);
        }
        if let Some(x) = v.diastolic {
            self.diastolic.add(x);
        }
        if let Some(x) = v.spo2 {
            self.spo2.add(x);
        }
        if let Some(x) = v.temperature {
            self.temperature.add(x);
        }
        if let Some(x) = v.respiratory_rate {
            self.respiratory_rate.add(x);
        }
        self.count += 1;
    }

    pub fn avg_for(&self, vital: VitalType) -> Option<u32> {
        match vital {
            VitalType::HeartRate => self.heart_rate.avg(),
            VitalType::Systolic => self.systolic.avg(),
            VitalType::Diastolic => self.diastolic.avg(),
            VitalType::Spo2 => self.spo2.avg(),
            VitalType::Temperature => self.temperature.avg(),
            VitalType::RespiratoryRate => self.respiratory_rate.avg(),
        }
    }
}

pub fn update_aggregate(env: &Env, patient: &Address, day: u64, v: &VitalSigns) {
    let key = (patient.clone(), day);
    let mut agg: DailyAggregate = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_default();
    agg.record(v);
    env.storage().persistent().set(&key, &agg);
}

pub fn get_aggregate(env: &Env, patient: &Address, day: u64) -> Option<DailyAggregate> {
    env.storage().persistent().get(&(patient.clone(), day))
}

/// Index of device ids registered for a patient, so deregistration can
/// enumerate and clear every `DeviceReg` / `DeviceAddress` entry.
pub fn add_registered_device(env: &Env, patient: &Address, device_id: &Address) {
    let key = (patient.clone(),);
    let mut devices: Vec<Address> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    if !devices.contains(device_id) {
        devices.push_back(device_id.clone());
        env.storage().persistent().set(&key, &devices);
    }
}

pub fn remove_registered_device(env: &Env, patient: &Address, device_id: &Address) {
    let key = (patient.clone(),);
    if let Some(devices) = env.storage().persistent().get::<_, Vec<Address>>(&key) {
        let mut next: Vec<Address> = Vec::new(env);
        for d in devices.iter() {
            if &d != device_id {
                next.push_back(d);
            }
        }
        env.storage().persistent().set(&key, &next);
    }
}

pub fn get_registered_devices(env: &Env, patient: &Address) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&(patient.clone(),))
        .unwrap_or_else(|| Vec::new(env))
}

/// Index of vital types a patient has granted to providers, so
/// deregistration can enumerate and clear every `MonitoringParams` entry.
pub fn add_monitoring_vital(env: &Env, patient: &Address, vital: &VitalType) {
    let key = (patient.clone(),);
    let mut vitals: Vec<VitalType> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    if !vitals.contains(vital) {
        vitals.push_back(vital.clone());
        env.storage().persistent().set(&key, &vitals);
    }
}

pub fn remove_monitoring_vital(env: &Env, patient: &Address, vital: &VitalType) {
    let key = (patient.clone(),);
    if let Some(vitals) = env.storage().persistent().get::<_, Vec<VitalType>>(&key) {
        let mut next: Vec<VitalType> = Vec::new(env);
        for v in vitals.iter() {
            if &v != vital {
                next.push_back(v);
            }
        }
        env.storage().persistent().set(&key, &next);
    }
}

pub fn get_monitoring_vitals(env: &Env, patient: &Address) -> Vec<VitalType> {
    env.storage()
        .persistent()
        .get(&(patient.clone(),))
        .unwrap_or_else(|| Vec::new(env))
}

/// Bounded per-day page of window indices. Each page holds at most
/// `WINDOW_PAGE_CAP` entries, so a write only ever touches the current
/// day's page instead of the whole patient index. This keeps write cost
/// constant regardless of how many windows a patient has accumulated.
pub const WINDOW_PAGE_CAP: u32 = 64;

#[contracttype]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WindowPage {
    pub raw_indices: Vec<u32>,
    pub agg_indices: Vec<u32>,
}

/// Append a raw window index to the patient's current day page, spilling
/// into a new page when the current one is full. Only the current page is
/// read and written, so cost does not grow with total window count.
pub fn append_raw_window(env: &Env, patient: &Address, day: u64, idx: u32) {
    let mut page_no = current_page_no(env, patient, day);
    loop {
        let key = (patient.clone(), day, page_no);
        let mut page: WindowPage = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_default();
        if page.raw_indices.len() < WINDOW_PAGE_CAP {
            page.raw_indices.push_back(idx);
            env.storage().persistent().set(&key, &page);
            return;
        }
        page_no += 1;
        env.storage()
            .persistent()
            .set(&(patient.clone(), day), &page_no);
    }
}

/// Append an aggregate window index to the patient's current day page.
pub fn append_agg_window(env: &Env, patient: &Address, day: u64, idx: u32) {
    let mut page_no = current_page_no(env, patient, day);
    loop {
        let key = (patient.clone(), day, page_no);
        let mut page: WindowPage = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_default();
        if page.agg_indices.len() < WINDOW_PAGE_CAP {
            page.agg_indices.push_back(idx);
            env.storage().persistent().set(&key, &page);
            return;
        }
        page_no += 1;
        env.storage()
            .persistent()
            .set(&(patient.clone(), day), &page_no);
    }
}

fn current_page_no(env: &Env, patient: &Address, day: u64) -> u32 {
    env.storage()
        .persistent()
        .get(&(patient.clone(), day))
        .unwrap_or(0u32)
}

/// Read the raw window indices for a single day page.
pub fn get_raw_page(env: &Env, patient: &Address, day: u64, page_no: u32) -> Vec<u32> {
    env.storage()
        .persistent()
        .get::<_, WindowPage>(&(patient.clone(), day, page_no))
        .map(|p| p.raw_indices)
        .unwrap_or_else(|| Vec::new(env))
}

/// Read the aggregate window indices for a single day page.
pub fn get_agg_page(env: &Env, patient: &Address, day: u64, page_no: u32) -> Vec<u32> {
    env.storage()
        .persistent()
        .get::<_, WindowPage>(&(patient.clone(), day, page_no))
        .map(|p| p.agg_indices)
        .unwrap_or_else(|| Vec::new(env))
}

/// Remove up to `max_windows` window entries for a patient, starting from
/// `day`/`page_no`. Returns the next `(day, page_no)` cursor, or `None`
/// when the index is fully drained. Callers can invoke this repeatedly to
/// complete erasure without exceeding per-transaction limits.
pub fn drain_window_page(
    env: &Env,
    patient: &Address,
    day: u64,
    page_no: u32,
    max_windows: u32,
) -> Option<(u64, u32)> {
    let key = (patient.clone(), day, page_no);
    let page: WindowPage = match env.storage().persistent().get(&key) {
        Some(p) => p,
        None => return None,
    };
    let mut removed = 0u32;
    for idx in page.raw_indices.iter() {
        if removed >= max_windows {
            return Some((day, page_no));
        }
        env.storage()
            .persistent()
            .remove(&(patient.clone(), idx));
        removed += 1;
    }
    for idx in page.agg_indices.iter() {
        if removed >= max_windows {
            return Some((day, page_no));
        }
        env.storage()
            .persistent()
            .remove(&(patient.clone(), idx));
        removed += 1;
    }
    env.storage().persistent().remove(&key);
    Some((day, page_no + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(hr: Option<u32>, sys: Option<u32>, dia: Option<u32>) -> VitalSigns {
        VitalSigns {
            heart_rate: hr,
            systolic: sys,
            diastolic: dia,
            spo2: None,
            temperature: None,
            respiratory_rate: None,
        }
    }

    #[test]
    fn interleaved_partial_readings_average_per_vital() {
        let mut agg = DailyAggregate::default();
        // reading 1: heart_rate only
        agg.record(&reading(Some(60), None, None));
        // reading 2: BP only (no heart_rate)
        agg.record(&reading(None, Some(120), Some(80)));
        // reading 3: heart_rate only
        agg.record(&reading(Some(90), None, None));

        assert_eq!(agg.count, 3);
        // heart_rate avg over the 2 readings that carried it: (60 + 90) / 2 = 75
        assert_eq!(agg.avg_for(VitalType::HeartRate), Some(75));
        // BP avg over the single reading that carried it
        assert_eq!(agg.avg_for(VitalType::Systolic), Some(120));
        assert_eq!(agg.avg_for(VitalType::Diastolic), Some(80));
        // vitals never seen have no average
        assert_eq!(agg.avg_for(VitalType::Spo2), None);
    }

    #[test]
    fn no_truncation_drift_over_long_series() {
        let mut agg = DailyAggregate::default();
        // 100 readings of 1 and 100 readings of 2 -> exact avg 1 (integer)
        for _ in 0..100 {
            agg.record(&reading(Some(1), None, None));
        }
        for _ in 0..100 {
            agg.record(&reading(Some(2), None, None));
        }
        // sum = 300, n = 200 -> 1 (integer division of exact sum, no per-step drift)
        assert_eq!(agg.avg_for(VitalType::HeartRate), Some(1));
    }
}
