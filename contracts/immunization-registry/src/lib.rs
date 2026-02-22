#![no_std]

mod types;
mod test;

use soroban_sdk::{contract, contractimpl, Env, Address, String, Symbol, Vec, BytesN};
use types::{DataKey, Error, VaccineRecord, AdverseEvent, VaccineSeries};

#[contract]
pub struct ImmunizationRegistry;

#[contractimpl]
impl ImmunizationRegistry {
    pub fn record_immunization(
        env: Env,
        record: VaccineRecord,
    ) -> Result<u64, Error> {
        record.provider_id.require_auth();

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ImmunizationCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage().instance().set(&DataKey::ImmunizationCounter, &new_id);

        env.storage().persistent().set(&DataKey::ImmunizationRecord(new_id), &record);

        let mut patient_records: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientImmunizations(record.patient_id.clone()))
            .unwrap_or(Vec::new(&env));
        patient_records.push_back(new_id);
        env.storage()
            .persistent()
            .set(&DataKey::PatientImmunizations(record.patient_id.clone()), &patient_records);

        Ok(new_id)
    }

    pub fn record_adverse_event(
        env: Env,
        immunization_id: u64,
        reporter: Address,
        event_description: String,
        severity: Symbol,
        onset_date: u64,
    ) -> Result<(), Error> {
        reporter.require_auth();

        if !env.storage().persistent().has(&DataKey::ImmunizationRecord(immunization_id)) {
            return Err(Error::RecordNotFound);
        }

        let event = AdverseEvent {
            reporter,
            event_description,
            severity,
            onset_date,
        };

        let mut events: Vec<AdverseEvent> = env
            .storage()
            .persistent()
            .get(&DataKey::AdverseEvents(immunization_id))
            .unwrap_or(Vec::new(&env));
        events.push_back(event);
        env.storage()
            .persistent()
            .set(&DataKey::AdverseEvents(immunization_id), &events);

        Ok(())
    }

    pub fn get_immunization_history(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<Vec<VaccineRecord>, Error> {
        requester.require_auth();

        let record_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientImmunizations(patient_id))
            .unwrap_or(Vec::new(&env));

        let mut history: Vec<VaccineRecord> = Vec::new(&env);
        for id in record_ids {
            if let Some(record) = env.storage().persistent().get(&DataKey::ImmunizationRecord(id)) {
                history.push_back(record);
            }
        }

        Ok(history)
    }

    pub fn register_vaccine_series(
        env: Env,
        patient_id: Address,
        series_name: String,
        doses_required: u32,
        schedule_hash: BytesN<32>,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let series = VaccineSeries {
            series_name,
            doses_required,
            schedule_hash,
        };

        let mut series_list: Vec<VaccineSeries> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientVaccineSeries(patient_id.clone()))
            .unwrap_or(Vec::new(&env));
        series_list.push_back(series);
        env.storage()
            .persistent()
            .set(&DataKey::PatientVaccineSeries(patient_id), &series_list);

        Ok(())
    }

    pub fn check_due_vaccines(
        env: Env,
        patient_id: Address,
        _current_date: u64,
    ) -> Result<Vec<VaccineSeries>, Error> {
        // For the sake of this functionality without complex date logic in the smart contract,
        // we determine if a series is due by counting the number of records a patient has
        // for that series (matched by a heuristic, like cvx_code or sequence counting).
        // A simple approach is returning series that have doses_required > currently administered doses.

        let series_list: Vec<VaccineSeries> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientVaccineSeries(patient_id.clone()))
            .unwrap_or(Vec::new(&env));

        let record_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientImmunizations(patient_id.clone()))
            .unwrap_or(Vec::new(&env));

        let mut due_series: Vec<VaccineSeries> = Vec::new(&env);

        for series in series_list {
            // Count how many records exist for this user that might match this series.
            // In a real medical system, we would match by CVX code exactly to the series definition.
            // Since we don't have CVX to Series mapping in the simplified schema, we'll
            // just count matching records based on name heuristics or assume each record
            // is a dose for a generic tracking purpose, or we just trust the system.

            // To adhere precisely to check_due_vaccines using the standard logic:
            // We check if the patient has received 'doses_required' for vaccines corresponding to this series.
            // Let's assume series_name matches vaccine_name for this heuristic:
            let mut administered_doses = 0;
            for id in record_ids.clone() {
                if let Some(record) = env.storage().persistent().get::<DataKey, VaccineRecord>(&DataKey::ImmunizationRecord(id)) {
                    if record.vaccine_name == series.series_name {
                        administered_doses += 1;
                    }
                }
            }

            if administered_doses < series.doses_required {
                due_series.push_back(series);
            }
        }

        Ok(due_series)
    }
}
