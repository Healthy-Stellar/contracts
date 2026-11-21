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
