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
    contract, contractimpl, symbol_short, vec, Address, Bytes, BytesN, Env, IntoVal, String,
    Symbol, Vec,
};
use soroban_sdk::xdr::ToXdr;
use storage::*;
use types::*;

const MAX_VIEW_TIMESTAMP_DRIFT_SECS: u64 = 300;

/// Upper bound on the number of items returned by any paged read.
pub const MAX_PAGE_LIMIT: u32 = 50;

fn clamp_limit(limit: u32) -> u32 {
    if limit == 0 || limit > MAX_PAGE_LIMIT {
        MAX_PAGE_LIMIT
    } else {
        limit
    }
}

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
    /// One-time setup: stores the admin and the provider-registry contract used
    /// to verify ordering providers.
    pub fn initialize(env: Env, admin: Address, provider_registry: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &provider_registry);
        Ok(())
    }

    /// Admin-only: point the contract at a different provider-registry.
    pub fn set_provider_registry(
        env: Env,
        admin: Address,
        provider_registry: Address,
    ) -> Result<(), Error> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if stored_admin != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &provider_registry);
        Ok(())
    }

    /// Register a new DICOM imaging study and return its on-chain study_id.
    ///
    /// The ordering provider must be an active provider in the configured
    /// provider-registry.
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
        ttl_config::extend_instance_ttl(&env);
        ordering_provider.require_auth();

        Self::assert_registered_provider(&env, &ordering_provider)?;

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

        push_patient_study(&env, &patient_id, study_id);

        // patient_id is intentionally not published: the event must not link
        // a study to a patient on a public ledger.
        env.events()
            .publish((symbol_short!("study_reg"), study_id), ordering_provider);

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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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
    ///
    /// Scans the patient's study index from `start` over at most `limit`
    /// entries (capped at `MAX_PAGE_LIMIT`).
    pub fn request_comparison_study(
        env: Env,
        current_study_id: u64,
        radiologist_id: Address,
        comparison_criteria: ComparisonCriteria,
        start: u64,
        limit: u32,
    ) -> Result<Vec<u64>, Error> {
        ttl_config::extend_instance_ttl(&env);
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
        for sid in patient_studies.iter() {
            if sid == current_study_id {
                continue;
            }
            if let Some(study) = load_study(&env, sid) {
                if comparison_criteria.matches(&study) {
                    results.push_back(sid);
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

        env.events()
            .publish((symbol_short!("cmp_req"), current_study_id), radiologist_id);

        Ok(matches)
    }

    /// Grant a viewer access to a study (optionally time-limited).
    pub fn grant_access(
        env: Env,
        study_id: u64,
        grantee: Address,
        purpose: String,
        expires_at: Option<u64>,
    ) -> Result<(), Error> {
        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        study.ordering_provider.require_auth();

        if purpose.is_empty() {
            return Err(Error::InvalidInput);
        }

        let grant = AccessGrant {
            grantee: grantee.clone(),
            study_id: Some(study_id),
            expires_at,
            revoked: false,
        };
        save_access_grant(&env, study_id, &grantee, &grant);

        env.events()
            .publish((symbol_short!("grant"), study_id), grantee);

        Ok(())
    }

    /// Revoke a previously granted access to a study.
    pub fn revoke_access(
        env: Env,
        study_id: u64,
        grantee: Address,
    ) -> Result<(), Error> {
        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        study.ordering_provider.require_auth();

        let mut grant =
            load_access_grant(&env, study_id, &grantee).ok_or(Error::NotFound)?;
        grant.revoked = true;
        save_access_grant(&env, study_id, &grantee, &grant);

        env.events()
            .publish((symbol_short!("revoke"), study_id), grantee);

        Ok(())
    }

    /// Record a view-log entry for a study (bounded, hash-chained).
    pub fn record_view(
        env: Env,
        study_id: u64,
        viewer_id: Address,
        purpose: String,
        view_timestamp: u64,
        view_duration: u32,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        viewer_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        let now = env.ledger().timestamp();

        let is_ordering_provider = study.ordering_provider == viewer_id;
        if !is_ordering_provider {
            Self::assert_active_grant(&env, study_id, &viewer_id, &purpose)?;
        }

        let recorded_at = env.ledger().timestamp();
        if view_timestamp > recorded_at + MAX_VIEW_TIMESTAMP_DRIFT_SECS {
            return Err(Error::InvalidInput);
        }

        let logs = load_view_logs(&env, study_id);
        let previous_entry_hash = logs.last().map(|entry| entry.entry_hash);

        if let Some(previous_ts) = load_viewer_last_view_timestamp(&env, study_id, &viewer_id) {
            if view_timestamp <= previous_ts {
                return Err(Error::NonMonotonicViewTimestamp);
            }
        }

        let previous_entry_hash = load_view_log_head(&env, study_id).last_hash;
        let entry_hash = compute_view_log_entry_hash(
            &env,
            study_id,
            &viewer_id,
            &purpose,
            view_timestamp,
            view_duration,
            recorded_at,
            &previous_entry_hash,
        );

        let record = ViewRecord {
            viewer_id: viewer_id.clone(),
            purpose: purpose.clone(),
            view_timestamp,
            view_duration,
            recorded_at,
            entry_hash,
        };

        save_view_logs(&env, study_id, &logs, &record);

        env.events()
            .publish((symbol_short!("view"), study_id), viewer_id);

        Ok(())
    }

    /// Assert that `viewer` holds an active (non-revoked, non-expired) grant for `study_id`.
    fn assert_active_grant(
        env: &Env,
        study_id: u64,
        viewer: &Address,
        purpose: &String,
    ) -> Result<(), Error> {
        if purpose.is_empty() {
            return Err(Error::InvalidInput);
        }

        let grant = load_access_grant(env, study_id, viewer).ok_or(Error::Unauthorized)?;

        if grant.revoked {
            return Err(Error::Unauthorized);
        }

        // A grant scoped to a different study must not unlock this one.
        if let Some(grant_study) = grant.study_id {
            if grant_study != study_id {
                return Err(Error::Unauthorized);
            }
        }

        if let Some(expires_at) = grant.expires_at {
            if env.ledger().timestamp() >= expires_at {
                return Err(Error::Unauthorized);
            }
        }

        Ok(())
    }
    String::from_bytes(env, &buf)
}
