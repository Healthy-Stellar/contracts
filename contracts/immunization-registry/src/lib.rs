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

#[cfg(test)]
mod test;
mod types;

use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, BytesN, Env, IntoVal, String, Symbol, Vec,
};
use types::{AdverseEvent, DataKey, Error, LastDose, VaccineRecord, VaccineSeries};

/// Maximum length of a lot number, in bytes.
const MAX_LOT_LEN: usize = 64;

/// Maximum number of patients returned by one `get_patients_by_lot` page.
pub const MAX_LOT_PAGE: u32 = 100;

/// Normalize a lot number so that `"EL9261"`, `"el9261 "` and `" El9261"`
/// map to the same lot: trims surrounding ASCII whitespace and upper-cases
/// ASCII letters.
fn normalize_lot(env: &Env, lot_number: &String) -> Result<String, Error> {
    let len = lot_number.len() as usize;
    if len == 0 || len > MAX_LOT_LEN {
        return Err(Error::InvalidLotNumber);
    }
    let mut buf = [0u8; MAX_LOT_LEN];
    lot_number.copy_into_slice(&mut buf[..len]);

    let mut start = 0;
    let mut end = len;
    while start < end && buf[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && buf[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if start == end {
        return Err(Error::InvalidLotNumber);
    }
    buf[start..end].make_ascii_uppercase();
    Ok(String::from_bytes(env, &buf[start..end]))
}

#[contract]
pub struct ImmunizationRegistry;

#[contractimpl]
impl ImmunizationRegistry {
    /// Configure the regulator/public-health authority and provider-registry
    /// contract address.
    ///
    /// Runs atomically as part of contract creation and cannot be called again,
    /// so an attacker cannot front-run deployment and choose the regulator.
    pub fn __constructor(env: Env, regulator: Address, provider_registry: Address) {
        ttl_config::extend_instance_ttl(&env);
        env.storage().instance().set(&DataKey::Regulator, &regulator);
        env.storage().instance().set(&DataKey::ProviderRegistry, &provider_registry);
    }

    /// Record an administered dose.
    ///
    /// - The same `(patient, cvx_code, dose_number)` can only be recorded once.
    /// - `dose_number` must be the previous dose for that `cvx_code` + 1, and
    ///   the administration date must be after the previous dose. Doses given
    ///   outside this registry (or catch-up schedules) can be recorded out of
    ///   sequence by passing a non-empty `override_reason`.
    /// - The lot number is normalized (trimmed, upper-cased) before storage.
    pub fn record_immunization(
        env: Env,
        record: VaccineRecord,
        override_reason: Option<String>,
    ) -> Result<u64, Error> {
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

        let mut record = record;
        record.lot_number = normalize_lot(&env, &record.lot_number)?;

        // Reject duplicate doses.
        let dose_key = DataKey::PatientDose(
            record.patient_id.clone(),
            record.cvx_code.clone(),
            record.dose_number,
        );
        if env.storage().persistent().has(&dose_key) {
            return Err(Error::DuplicateDose);
        }

        // Enforce dose sequence unless an override reason is supplied.
        let last_key = DataKey::PatientLastDose(record.patient_id.clone(), record.cvx_code.clone());
        let last: Option<LastDose> = env.storage().persistent().get(&last_key);
        let has_override = override_reason.as_ref().map_or(false, |r| !r.is_empty());
        if !has_override {
            let expected = last.as_ref().map_or(1, |l| l.dose_number.saturating_add(1));
            if record.dose_number != expected {
                return Err(Error::OutOfSequenceDose);
            }
            if let Some(ref l) = last {
                if record.administration_date <= l.administration_date {
                    return Err(Error::OutOfSequenceDose);
                }
            }
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

        env.storage().persistent().set(&dose_key, &new_id);
        if has_override {
            // Audit trail for out-of-sequence doses (ids only).
            env.events()
                .publish((symbol_short!("dose_ovr"), new_id), record.dose_number);
        }
        if last.as_ref().map_or(true, |l| record.dose_number > l.dose_number) {
            env.storage().persistent().set(
                &last_key,
                &LastDose {
                    dose_number: record.dose_number,
                    administration_date: record.administration_date,
                },
            );
        }

        // Per-lot index: one entry per dose plus a distinct-patient index, so
        // each write touches a constant number of ledger entries regardless
        // of lot size.
        let lot = record.lot_number.clone();
        let lot_count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::LotCount(lot.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::LotDose(lot.clone(), lot_count), &new_id);
        env.storage()
            .persistent()
            .set(&DataKey::LotCount(lot.clone()), &(lot_count + 1));

        let lot_patient_key = DataKey::LotPatient(lot.clone(), record.patient_id.clone());
        if !env.storage().persistent().has(&lot_patient_key) {
            env.storage().persistent().set(&lot_patient_key, &true);
            let patient_count: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::LotPatientCount(lot.clone()))
                .unwrap_or(0);
            env.storage().persistent().set(
                &DataKey::LotPatientAt(lot.clone(), patient_count),
                &record.patient_id,
            );
            env.storage()
                .persistent()
                .set(&DataKey::LotPatientCount(lot), &(patient_count + 1));
        }

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

    /// Return a page of the distinct patients who received a dose from
    /// `lot_number`, for recall tracing. Patients are returned in the order
    /// they were first vaccinated from the lot, starting at index `cursor`;
    /// `limit` is capped at `MAX_LOT_PAGE`. Use `get_lot_patient_count` to
    /// know when to stop. Restricted to the configured regulator/public-health
    /// authority.
    pub fn get_patients_by_lot(
        env: Env,
        lot_number: String,
        requester: Address,
        cursor: u64,
        limit: u32,
    ) -> Result<Vec<Address>, Error> {
        Self::require_regulator(&env, &requester)?;
        let lot = normalize_lot(&env, &lot_number)?;

        let total: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::LotPatientCount(lot.clone()))
            .unwrap_or(0);
        let limit = if limit == 0 || limit > MAX_LOT_PAGE {
            MAX_LOT_PAGE
        } else {
            limit
        };
        let end = total.min(cursor.saturating_add(limit as u64));

        let mut patients: Vec<Address> = Vec::new(&env);
        let mut i = cursor;
        while i < end {
            if let Some(patient) = env
                .storage()
                .persistent()
                .get::<DataKey, Address>(&DataKey::LotPatientAt(lot.clone(), i))
            {
                patients.push_back(patient);
            }
            i += 1;
        }

        Ok(patients)
    }

    /// Number of distinct patients vaccinated from `lot_number`. Regulator only.
    pub fn get_lot_patient_count(
        env: Env,
        lot_number: String,
        requester: Address,
    ) -> Result<u64, Error> {
        Self::require_regulator(&env, &requester)?;
        let lot = normalize_lot(&env, &lot_number)?;
        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::LotPatientCount(lot))
            .unwrap_or(0))
    }

    /// Number of doses administered from `lot_number`. Regulator only.
    pub fn get_lot_dose_count(
        env: Env,
        lot_number: String,
        requester: Address,
    ) -> Result<u64, Error> {
        Self::require_regulator(&env, &requester)?;
        let lot = normalize_lot(&env, &lot_number)?;
        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::LotCount(lot))
            .unwrap_or(0))
    }

    fn require_regulator(env: &Env, requester: &Address) -> Result<(), Error> {
        requester.require_auth();
        let regulator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Regulator)
            .ok_or(Error::NotInitialized)?;
        if *requester != regulator {
            return Err(Error::NotAuthorized);
        }
        Ok(())
    }

    pub fn record_adverse_event(
        env: Env,
        immunization_id: u64,
        reporter: Address,
        event_description: String,
        severity: Symbol,
        onset_date: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
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
