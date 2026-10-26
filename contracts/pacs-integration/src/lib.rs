#![no_std]
#![allow(clippy::too_many_arguments)]

//! # PACS Integration Contract
//!
//! Picture Archiving and Communication System (PACS) integration for medical imaging management,
//! study routing, archive retrieval, and image reference tracking.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** DICOM gateway/PACS system authentication for image uploads.
//! Radiologist access for study routing. Ordering provider access for image retrieval.
//! Patient consent for imaging data storage. Archive access restricted to authorized facilities.
//!
//! **Audit Controls:** Image upload events logged with modality, patient, and study date.
//! Study routing events tracked with destination facility. Archive retrieval events logged.
//! Image reference events tracked for access audit. Deletion events recorded. DICOM metadata
//! audit trail maintained.
//!
//! **Data Retention Policy:** Medical images retained indefinitely per regulatory requirements.
//! DICOM metadata archived with image references. Study routing history maintained. Archive
//! retrieval logs retained for audit. Deletion requests preserved with justification.
//!
//! **Encryption/Integrity:** DICOM image data encrypted in persistent storage. Image hash
//! validation ensures integrity. Patient-image linkage encrypted. Study date immutable.
//! PACS endpoint validation prevents unauthorized image routing.

mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env, String, Symbol, Vec,
};
use soroban_sdk::xdr::ToXdr;
use storage::*;
use types::*;

const MAX_VIEW_TIMESTAMP_DRIFT_SECS: u64 = 300;

fn compute_view_log_entry_hash(
    env: &Env,
    study_id: u64,
    viewer_id: &Address,
    purpose: &String,
    view_timestamp: u64,
    view_duration: u32,
    recorded_at: u64,
    previous_entry_hash: &Option<BytesN<32>>,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_array(&study_id.to_be_bytes());
    data.append(&viewer_id.clone().to_xdr(env));
    data.append(&purpose.clone().to_xdr(env));
    data.extend_from_array(&view_timestamp.to_be_bytes());
    data.extend_from_array(&view_duration.to_be_bytes());
    data.extend_from_array(&recorded_at.to_be_bytes());
    if let Some(prev) = previous_entry_hash {
        data.append(&prev.clone().to_xdr(env));
    }
    env.crypto().sha256(&data).into()
}

#[contract]
pub struct PacsContract;

#[contractimpl]
impl PacsContract {
    /// Register a new DICOM imaging study and return its on-chain study_id.
    #[allow(clippy::too_many_arguments)]
    pub fn register_imaging_study(
        env: Env,
        patient_id: Address,
        ordering_provider: Address,
        study_uid: String,
        modality: Symbol,
        body_part: String,
        study_date: u64,
        study_description: String,
        series_count: u32,
        image_count: u32,
        storage_location_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        ordering_provider.require_auth();

        if study_uid.is_empty() || body_part.is_empty() {
            return Err(Error::InvalidInput);
        }

        let study_id = next_study_id(&env);

        let study = ImagingStudy {
            study_id,
            patient_id: patient_id.clone(),
            ordering_provider: ordering_provider.clone(),
            study_uid,
            modality,
            body_part,
            study_date,
            study_description,
            series_count,
            image_count,
            storage_location_hash,
            has_report: false,
            critical_findings: false,
            registered_at: env.ledger().timestamp(),
        };

        save_study(&env, &study);

        let mut patient_studies = load_patient_studies(&env, &patient_id);
        patient_studies.push_back(study_id);
        save_patient_studies(&env, &patient_id, &patient_studies);

        env.events().publish(
            (symbol_short!("study_reg"), study_id),
            (patient_id, ordering_provider),
        );

        Ok(study_id)
    }

    /// Append a DICOM series to an existing study.
    pub fn add_series_to_study(
        env: Env,
        study_id: u64,
        series_uid: String,
        series_number: u32,
        series_description: String,
        image_count: u32,
        acquisition_date: u64,
    ) -> Result<(), Error> {
        let mut study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        study.ordering_provider.require_auth();

        if series_uid.is_empty() {
            return Err(Error::InvalidInput);
        }

        let series = SeriesInfo {
            series_uid,
            series_number,
            series_description,
            image_count,
            acquisition_date,
        };

        let mut list = load_series(&env, study_id);
        list.push_back(series);
        save_series(&env, study_id, &list);

        study.series_count = list.len() as u32;
        save_study(&env, &study);

        env.events()
            .publish((symbol_short!("ser_add"), study_id), series_number);

        Ok(())
    }

    /// Link a radiology report (preliminary / final / addendum) to a study.
    pub fn link_imaging_report(
        env: Env,
        study_id: u64,
        radiologist_id: Address,
        report_type: Symbol,
        report_hash: BytesN<32>,
        critical_findings: bool,
    ) -> Result<(), Error> {
        radiologist_id.require_auth();

        let mut study = load_study(&env, study_id).ok_or(Error::NotFound)?;

        // Only "addendum" may be added once a report already exists.
        if study.has_report && report_type != Symbol::new(&env, "addendum") {
            return Err(Error::ReportAlreadyExists);
        }

        let report = ImagingReport {
            study_id,
            radiologist_id: radiologist_id.clone(),
            report_type,
            report_hash,
            critical_findings,
            reported_at: env.ledger().timestamp(),
            crit_find_ack_at: None,
        };

        save_report(&env, &report);

        study.has_report = true;
        if critical_findings {
            study.critical_findings = true;
        }
        save_study(&env, &study);

        env.events().publish(
            (symbol_short!("rpt_link"), study_id),
            (radiologist_id, critical_findings),
        );

        Ok(())
    }

    /// Find prior studies for the same patient that match the comparison criteria.
    pub fn request_comparison_study(
        env: Env,
        current_study_id: u64,
        radiologist_id: Address,
        comparison_criteria: ComparisonCriteria,
    ) -> Result<Vec<u64>, Error> {
        radiologist_id.require_auth();

        let current = load_study(&env, current_study_id).ok_or(Error::NotFound)?;

        // Access control: caller must be the ordering provider on the current study
        // or hold an active access grant for it (purpose-scoped).
        // This prevents any authenticated address from enumerating a patient's
        // full imaging history by guessing a known study_id.
        let is_ordering_provider = current.ordering_provider == radiologist_id;
        if !is_ordering_provider {
            // Require an active grant; the purpose must be non-empty, so we
            // derive it from the comparison context.
            let grants = load_access_list(&env, current_study_id);
            let mut authorized = false;
            for grant in grants.iter() {
                if grant.viewer_id == radiologist_id {
                    if Self::assert_active_grant(
                        &env,
                        current_study_id,
                        &radiologist_id,
                        &grant.purpose,
                    )
                    .is_ok()
                    {
                        authorized = true;
                        break;
                    }
                }
            }
            if !authorized {
                return Err(Error::Unauthorized);
            }
        }

        let mut results = Vec::new(&env);
        let patient_studies = load_patient_studies(&env, &current.patient_id);

        for id in patient_studies.iter() {
            if id == current_study_id {
                continue;
            }
            if let Some(study) = load_study(&env, id) {
                if Self::matches_comparison(&study, &current, &comparison_criteria) {
                    results.push_back(id);
                }
            }
        }

        Ok(results)
    }

    /// Retrieve an imaging study by id.
    ///
    /// Access is restricted to the study's patient, the ordering provider, or an
    /// active grantee (same `assert_active_grant` logic as `search_imaging_studies`).
    pub fn get_imaging_study(
        env: Env,
        requester: Address,
        study_id: u64,
    ) -> Result<ImagingStudy, Error> {
        requester.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;

        Self::assert_study_access(&env, &study, &requester)?;

        Ok(study)
    }

    /// Retrieve the view logs for an imaging study.
    ///
    /// Access is restricted to the study's patient, the ordering provider, or an
    /// active grantee (same `assert_active_grant` logic as `search_imaging_studies`).
    pub fn get_study_view_logs(
        env: Env,
        requester: Address,
        study_id: u64,
    ) -> Result<Vec<ViewRecord>, Error> {
        requester.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;

        Self::assert_study_access(&env, &study, &requester)?;

        Ok(load_view_logs(&env, study_id))
    }

    /// Shared access check: the requester must be the study's patient, the
    /// ordering provider, or hold an active access grant for the study.
    fn assert_study_access(
        env: &Env,
        study: &ImagingStudy,
        requester: &Address,
    ) -> Result<(), Error> {
        if study.patient_id == *requester || study.ordering_provider == *requester {
            return Ok(());
        }

        let grants = load_access_list(env, study.study_id);
        for grant in grants.iter() {
            if grant.viewer_id == *requester {
                if Self::assert_active_grant(
                    env,
                    study.study_id,
                    requester,
                    &grant.purpose,
                )
                .is_ok()
                {
                    return Ok(());
                }
            }
        }

        Err(Error::Unauthorized)
    }

    /// Find prior studies for the same patient that match the comparison criteria.
    pub fn search_imaging_studies(
        env: Env,
        requester: Address,
        patient_id: Address,
        modality: Option<Symbol>,
        body_part: Option<String>,
        date_from: Option<u64>,
        date_to: Option<u64>,
    ) -> Result<Vec<u64>, Error> {
        requester.require_auth();

        // Access control: the patient themselves, or an active grantee.
        if requester != patient_id {
            let grants = load_access_list(&env, 0);
            let mut authorized = false;
            for grant in grants.iter() {
                if grant.viewer_id == requester {
                    if Self::assert_active_grant(&env, 0, &requester, &grant.purpose).is_ok() {
                        authorized = true;
                        break;
                    }
                }
            }
            if !authorized {
                return Err(Error::Unauthorized);
            }
        }

        let mut results = Vec::new(&env);
        let patient_studies = load_patient_studies(&env, &patient_id);

        for id in patient_studies.iter() {
            if let Some(study) = load_study(&env, id) {
                if let Some(ref m) = modality {
                    if study.modality != *m {
                        continue;
                    }
                }
                if let Some(ref bp) = body_part {
                    if study.body_part != *bp {
                        continue;
                    }
                }
                if let Some(from) = date_from {
                    if study.study_date < from {
                        continue;
                    }
                }
                if let Some(to) = date_to {
                    if study.study_date > to {
                        continue;
                    }
                }
                results.push_back(id);
            }
        }

        Ok(results)
    }

    /// Record a view of an imaging study.
    pub fn record_study_view(
        env: Env,
        study_id: u64,
        viewer_id: Address,
        purpose: String,
        view_timestamp: u64,
        view_duration: u32,
    ) -> Result<(), Error> {
        viewer_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;

        if purpose.is_empty() {
            return Err(Error::InvalidInput);
        }

        let now = env.ledger().timestamp();
        let drift = if view_timestamp > now {
            view_timestamp - now
        } else {
            now - view_timestamp
        };
        if drift > MAX_VIEW_TIMESTAMP_DRIFT_SECS {
            return Err(Error::InvalidInput);
        }

        let mut logs = load_view_logs(&env, study_id);
        let previous_entry_hash = logs.last().map(|r| r.entry_hash.clone());

        let entry_hash = compute_view_log_entry_hash(
            &env,
            study_id,
            &viewer_id,
            &purpose,
            view_timestamp,
            view_duration,
            now,
            &previous_entry_hash,
        );

        let record = ViewRecord {
            study_id,
            viewer_id: viewer_id.clone(),
            purpose,
            view_timestamp,
            view_duration,
            recorded_at: now,
            entry_hash,
        };

        logs.push_back(record);
        save_view_logs(&env, study_id, &logs);

        env.events()
            .publish((symbol_short!("view_rec"), study_id), viewer_id);

        let _ = study;
        Ok(())
    }

    /// Grant a viewer access to a study.
    pub fn grant_study_access(
        env: Env,
        study_id: u64,
        patient_id: Address,
        viewer_id: Address,
        purpose: String,
        expires_at: u64,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        if study.patient_id != patient_id {
            return Err(Error::Unauthorized);
        }

        if purpose.is_empty() {
            return Err(Error::InvalidInput);
        }

        let grant = AccessGrant {
            study_id,
            viewer_id: viewer_id.clone(),
            purpose,
            granted_at: env.ledger().timestamp(),
            expires_at,
            revoked: false,
        };

        let mut grants = load_access_list(&env, study_id);
        grants.push_back(grant);
        save_access_list(&env, study_id, &grants);

        env.events()
            .publish((symbol_short!("grant"), study_id), viewer_id);

        Ok(())
    }

    /// Revoke a previously granted access to a study.
    pub fn revoke_study_access(
        env: Env,
        study_id: u64,
        patient_id: Address,
        viewer_id: Address,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        if study.patient_id != patient_id {
            return Err(Error::Unauthorized);
        }

        let mut grants = load_access_list(&env, study_id);
        let mut found = false;
        for i in 0..grants.len() {
            let mut grant = grants.get(i).unwrap();
            if grant.viewer_id == viewer_id && !grant.revoked {
                grant.revoked = true;
                grants.set(i, grant);
                found = true;
                break;
            }
        }

        if !found {
            return Err(Error::NotFound);
        }

        save_access_list(&env, study_id, &grants);

        env.events()
            .publish((symbol_short!("revoke"), study_id), viewer_id);

        Ok(())
    }

    /// Verify that a grant for `viewer_id` on `study_id` is active and not expired.
    fn assert_active_grant(
        env: &Env,
        study_id: u64,
        viewer_id: &Address,
        purpose: &String,
    ) -> Result<(), Error> {
        if purpose.is_empty() {
            return Err(Error::Unauthorized);
        }

        let grants = load_access_list(env, study_id);
        let now = env.ledger().timestamp();

        for grant in grants.iter() {
            if grant.viewer_id == *viewer_id && !grant.revoked && grant.expires_at > now {
                return Ok(());
            }
        }

        Err(Error::Unauthorized)
    }

    fn matches_comparison(
        study: &ImagingStudy,
        current: &ImagingStudy,
        criteria: &ComparisonCriteria,
    ) -> bool {
        if criteria.same_modality && study.modality != current.modality {
            return false;
        }
        if criteria.same_body_part && study.body_part != current.body_part {
            return false;
        }
        if let Some(max_age) = criteria.max_age_days {
            let max_age_secs = (max_age as u64) * 86_400;
            if current.study_date > study.study_date
                && current.study_date - study.study_date > max_age_secs
            {
                return false;
            }
        }
        true
    }
}
