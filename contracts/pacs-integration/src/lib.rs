#![no_std]

mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, BytesN, Env, String, Symbol, Vec,
};
use storage::*;
use types::*;

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

        if study_uid.len() == 0 || body_part.len() == 0 {
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

        if series_uid.len() == 0 {
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
        let patient_ids = load_patient_studies(&env, &current.patient_id);

        let now = env.ledger().timestamp();
        let max_age_secs = (comparison_criteria.max_age_days as u64) * 86_400;

        let mut matches: Vec<u64> = Vec::new(&env);

        for sid in patient_ids.iter() {
            if sid == current_study_id {
                continue;
            }
            if let Some(s) = load_study(&env, sid) {
                if let Some(ref m) = comparison_criteria.modality {
                    if s.modality != *m {
                        continue;
                    }
                }
                if s.body_part != comparison_criteria.body_part {
                    continue;
                }
                if max_age_secs > 0 && now > s.study_date && now - s.study_date > max_age_secs {
                    continue;
                }
                matches.push_back(sid);
            }
        }

        env.events().publish(
            (symbol_short!("cmp_req"), current_study_id),
            radiologist_id,
        );

        Ok(matches)
    }

    /// Patient grants a viewer access (with optional expiry) to a study.
    pub fn grant_imaging_access(
        env: Env,
        study_id: u64,
        patient_id: Address,
        viewer_id: Address,
        access_type: Symbol,
        expires_at: Option<u64>,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        if study.patient_id != patient_id {
            return Err(Error::Unauthorized);
        }

        let grant = AccessGrant {
            viewer_id: viewer_id.clone(),
            access_type,
            granted_at: env.ledger().timestamp(),
            expires_at,
        };

        let mut grants = load_access_list(&env, study_id);
        grants.push_back(grant);
        save_access_list(&env, study_id, &grants);

        env.events().publish(
            (symbol_short!("acc_grant"), study_id),
            (patient_id, viewer_id),
        );

        Ok(())
    }

    /// Bundle multiple studies into a portable CD record; returns cd_id.
    pub fn create_imaging_cd(
        env: Env,
        study_ids: Vec<u64>,
        patient_id: Address,
        requesting_provider: Address,
        cd_token: String,
        created_at: u64,
    ) -> Result<u64, Error> {
        requesting_provider.require_auth();

        if study_ids.len() == 0 || cd_token.len() == 0 {
            return Err(Error::InvalidInput);
        }

        for sid in study_ids.iter() {
            let study = load_study(&env, sid).ok_or(Error::NotFound)?;
            if study.patient_id != patient_id {
                return Err(Error::Unauthorized);
            }
        }

        let cd_id = next_cd_id(&env);

        let record = CdRecord {
            cd_id,
            study_ids,
            patient_id: patient_id.clone(),
            requesting_provider: requesting_provider.clone(),
            cd_token,
            created_at,
        };

        save_cd_record(&env, &record);

        env.events().publish(
            (symbol_short!("cd_create"), cd_id),
            (patient_id, requesting_provider),
        );

        Ok(cd_id)
    }

    /// Generate an anonymized UID for a study and record the request.
    pub fn anonymize_study(
        env: Env,
        study_id: u64,
        requesting_researcher: Address,
        anonymization_level: Symbol,
        purpose: String,
    ) -> Result<String, Error> {
        requesting_researcher.require_auth();

        // study must exist
        load_study(&env, study_id).ok_or(Error::NotFound)?;

        if purpose.len() == 0 {
            return Err(Error::InvalidInput);
        }

        // Produce a deterministic anonymized UID: prefix "ANON-" is stored on-chain.
        // Richer derivation (e.g. hashing) would be done off-chain using study_id.
        let anon_uid = String::from_str(&env, "ANON-");
        save_anonymized_uid(&env, study_id, &anon_uid);

        env.events().publish(
            (symbol_short!("anon"), study_id),
            (requesting_researcher, anonymization_level, purpose),
        );

        Ok(anon_uid)
    }

    /// Record a quality-control review for a study.
    pub fn quality_control_review(
        env: Env,
        study_id: u64,
        reviewer_id: Address,
        quality_score: u32,
        technical_issues: Vec<String>,
        repeat_required: bool,
    ) -> Result<(), Error> {
        reviewer_id.require_auth();

        load_study(&env, study_id).ok_or(Error::NotFound)?;

        if quality_score > 100 {
            return Err(Error::InvalidInput);
        }

        let review = QcReview {
            study_id,
            reviewer_id: reviewer_id.clone(),
            quality_score,
            technical_issues,
            repeat_required,
            reviewed_at: env.ledger().timestamp(),
        };

        save_qc_review(&env, &review);

        env.events().publish(
            (symbol_short!("qc_done"), study_id),
            (reviewer_id, quality_score, repeat_required),
        );

        Ok(())
    }

    /// Audit-log a view event; enforces access-grant expiry for third parties.
    pub fn track_study_views(
        env: Env,
        study_id: u64,
        viewer_id: Address,
        view_timestamp: u64,
        view_duration: u32,
    ) -> Result<(), Error> {
        viewer_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;

        let is_owner =
            study.patient_id == viewer_id || study.ordering_provider == viewer_id;

        if !is_owner {
            let now = env.ledger().timestamp();
            let grants = load_access_list(&env, study_id);
            let mut allowed = false;
            for grant in grants.iter() {
                if grant.viewer_id == viewer_id {
                    if let Some(exp) = grant.expires_at {
                        if now > exp {
                            return Err(Error::AccessExpired);
                        }
                    }
                    allowed = true;
                    break;
                }
            }
            if !allowed {
                return Err(Error::Unauthorized);
            }
        }

        let record = ViewRecord {
            viewer_id: viewer_id.clone(),
            view_timestamp,
            view_duration,
        };

        append_view_log(&env, study_id, &record);

        env.events().publish(
            (symbol_short!("view_log"), study_id),
            (viewer_id, view_timestamp),
        );

        Ok(())
    }

    /// Return studies for a patient that pass the filters and that the requester
    /// has access to.
    pub fn search_imaging_studies(
        env: Env,
        patient_id: Address,
        requester: Address,
        filters: ImagingFilters,
    ) -> Result<Vec<ImagingStudy>, Error> {
        requester.require_auth();

        let now = env.ledger().timestamp();
        let study_ids = load_patient_studies(&env, &patient_id);
        let mut results: Vec<ImagingStudy> = Vec::new(&env);

        for sid in study_ids.iter() {
            if let Some(study) = load_study(&env, sid) {
                // --- access check ---
                let is_owner =
                    study.patient_id == requester || study.ordering_provider == requester;
                let mut allowed = is_owner;

                if !allowed {
                    let grants = load_access_list(&env, sid);
                    for grant in grants.iter() {
                        if grant.viewer_id == requester {
                            let active = match grant.expires_at {
                                Some(exp) => now <= exp,
                                None => true,
                            };
                            if active {
                                allowed = true;
                            }
                            break;
                        }
                    }
                }

                if !allowed {
                    continue;
                }

                // --- filter pass ---
                if let Some(ref m) = filters.modality {
                    if study.modality != *m {
                        continue;
                    }
                }
                if let Some(ref bp) = filters.body_part {
                    if study.body_part != *bp {
                        continue;
                    }
                }
                if let Some(start) = filters.start_date {
                    if study.study_date < start {
                        continue;
                    }
                }
                if let Some(end) = filters.end_date {
                    if study.study_date > end {
                        continue;
                    }
                }
                if let Some(crit) = filters.has_critical_findings {
                    if study.critical_findings != crit {
                        continue;
                    }
                }

                results.push_back(study);
            }
        }

        Ok(results)
    }
}
