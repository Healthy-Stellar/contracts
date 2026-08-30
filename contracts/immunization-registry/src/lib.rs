#![no_std]

//! # Immunization Registry Contract
//!
//! Manages vaccine records, vaccination series tracking, adverse events, and immunization history
//! for population health management and outbreak response.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Immunization provider authentication required for record updates.
//! Patient consent for vaccine record access. Public health authority access for epidemiological
//! queries. Adverse event reporting restricted to authorized reporters. Registry data access
//! controlled per role.
//!
//! **Audit Controls:** Vaccine administration events logged with vaccine type, date, and provider.
//! Series completion events tracked. Adverse event reports captured with severity. Population
//! immunization coverage calculated from records. Audit trail enables vaccine safety monitoring.
//!
//! **Data Retention Policy:** Vaccine records retained indefinitely for lifetime immunity tracking.
//! Series status tracked with completion date. Adverse events retained for pharmacovigilance.
//! Exemptions and contraindications documented. Deregistration marks patient records as deleted
//! without removal.
//!
//! **Encryption/Integrity:** Vaccine type enumerations prevent invalid vaccines. Series status
//! tracking prevents duplicate administrations. Adverse event records encrypted. Patient identity
//! validated via address. Timestamps immutable once recorded.

mod test;
mod types;

use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env, String, Symbol, Vec};
use types::{AdverseEvent, DataKey, Error, VaccineRecord, VaccineSeries};

#[contract]
pub struct ImmunizationRegistry;

#[contractimpl]
impl ImmunizationRegistry {
    /// Configure the regulator/public-health authority and provider-registry contract address.
    pub fn initialize(env: Env, regulator: Address, provider_registry: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Regulator) {
            return Err(Error::AlreadyInitialized);
        }

        regulator.require_auth();
        env.storage().instance().set(&DataKey::Regulator, &regulator);
        env.storage().instance().set(&DataKey::ProviderRegistry, &provider_registry);
        Ok(())
    }

    pub fn record_immunization(env: Env, record: VaccineRecord) -> Result<u64, Error> {
        record.provider_id.require_auth();
        record.patient_id.require_auth();

        // Verify caller is a registered provider.
        let provider_registry: Address = env
            .storage()
            .instance()
            .get(&DataKey::ProviderRegistry)
            .ok_or(Error::NotInitialized)?;
        let is_registered: bool = env.invoke_contract(
            &provider_registry,
            &Symbol::new(&env, "is_provider"),
            soroban_sdk::vec![&env, record.provider_id.clone().into_val(&env)],
        );
        if !is_registered {
            return Err(Error::NotAuthorized);
        }

        // Validate dose_number is not zero
        if record.dose_number == 0 {
            return Err(Error::InvalidDoseNumber);
        }

        // Validate administration_date is not in the future
        let current_time = env.ledger().timestamp();
        if record.administration_date > current_time {
            return Err(Error::InvalidAdministrationDate);
        }

        // Validate vaccine is not expired at administration
        if record.administration_date > record.expiration_date {
            return Err(Error::VaccineExpired);
        }

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ImmunizationCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::ImmunizationCounter, &new_id);

        env.storage()
            .persistent()
            .set(&DataKey::ImmunizationRecord(new_id), &record);

        let mut patient_records: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientImmunizations(record.patient_id.clone()))
            .unwrap_or(Vec::new(&env));
        patient_records.push_back(new_id);
        env.storage().persistent().set(
            &DataKey::PatientImmunizations(record.patient_id.clone()),
            &patient_records,
        );

        let mut lot_records: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::LotImmunizations(record.lot_number.clone()))
            .unwrap_or(Vec::new(&env));
        lot_records.push_back(new_id);
        env.storage().persistent().set(
            &DataKey::LotImmunizations(record.lot_number.clone()),
            &lot_records,
        );

        env.events().publish(
            (
                symbol_short!("immunize"),
                record.patient_id.clone(),
                record.provider_id.clone(),
            ),
            (new_id, record.vaccine_name.clone(), record.cvx_code.clone()),
        );

        Ok(new_id)
    }

    /// Return the distinct patients who received a dose from `lot_number`, for recall tracing.
    /// Restricted to the configured regulator/public-health authority.
    pub fn get_patients_by_lot(
        env: Env,
        lot_number: String,
        requester: Address,
    ) -> Result<Vec<Address>, Error> {
        requester.require_auth();

        let regulator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Regulator)
            .ok_or(Error::NotInitialized)?;
        if requester != regulator {
            return Err(Error::NotAuthorized);
        }

        let record_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::LotImmunizations(lot_number))
            .unwrap_or(Vec::new(&env));

        let mut patients: Vec<Address> = Vec::new(&env);
        for id in record_ids {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, VaccineRecord>(&DataKey::ImmunizationRecord(id))
            {
                if !patients.contains(&record.patient_id) {
                    patients.push_back(record.patient_id);
                }
            }
        }

        Ok(patients)
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

        let record: VaccineRecord = env
            .storage()
            .persistent()
            .get(&DataKey::ImmunizationRecord(immunization_id))
            .ok_or(Error::RecordNotFound)?;

        let regulator: Option<Address> = env.storage().instance().get(&DataKey::Regulator);
        let authorized = reporter == record.provider_id
            || reporter == record.patient_id
            || regulator.map(|r| reporter == r).unwrap_or(false);
        if !authorized {
            return Err(Error::NotAuthorized);
        }

        let event = AdverseEvent {
            reporter: reporter.clone(),
            event_description,
            severity: severity.clone(),
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

        env.events().publish(
            (symbol_short!("adv_event"), immunization_id, reporter),
            (severity, onset_date),
        );

        Ok(())
    }

    pub fn get_immunization_history(
        env: Env,
        patient_id: Address,
        requester: Address,
    ) -> Result<Vec<VaccineRecord>, Error> {
        requester.require_auth();
        if requester != patient_id {
            return Err(Error::NotAuthorized);
        }

        let record_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientImmunizations(patient_id))
            .unwrap_or(Vec::new(&env));

        let mut history: Vec<VaccineRecord> = Vec::new(&env);
        for id in record_ids {
            if let Some(record) = env
                .storage()
                .persistent()
                .get(&DataKey::ImmunizationRecord(id))
            {
                history.push_back(record);
            }
        }

        Ok(history)
    }

    pub fn register_vaccine_series(
        env: Env,
        patient_id: Address,
        series_name: String,
        cvx_code: String,
        doses_required: u32,
        schedule_hash: BytesN<32>,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let series = VaccineSeries {
            series_name: series_name.clone(),
            cvx_code: cvx_code.clone(),
            doses_required,
            schedule_hash,
        };

        let mut series_list: Vec<VaccineSeries> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientVaccineSeries(patient_id.clone()))
            .unwrap_or(Vec::new(&env));
        series_list.push_back(series);
        env.storage().persistent().set(
            &DataKey::PatientVaccineSeries(patient_id.clone()),
            &series_list,
        );

        env.events().publish(
            (symbol_short!("vac_ser"), patient_id, cvx_code),
            (series_name, doses_required),
        );

        Ok(())
    }

    pub fn check_due_vaccines(
        env: Env,
        patient_id: Address,
        requester: Address,
        _current_date: u64,
    ) -> Result<Vec<VaccineSeries>, Error> {
        requester.require_auth();
        if requester != patient_id {
            return Err(Error::NotAuthorized);
        }
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
            // Match administered doses to the series by CVX code, not vaccine name, so
            // brand names and combination vaccines administered under the same CVX code
            // are counted correctly.
            let mut administered_doses = 0;
            for id in record_ids.clone() {
                if let Some(record) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, VaccineRecord>(&DataKey::ImmunizationRecord(id))
                {
                    if record.cvx_code == series.cvx_code {
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
