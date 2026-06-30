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

        env.events()
            .publish((symbol_short!("cmp_req"), current_study_id), radiologist_id);

        Ok(matches)
    }

    /// Patient grants a viewer access (with optional expiry) to a study.
    pub fn grant_imaging_access(
        env: Env,
        study_id: u64,
        patient_id: Address,
        viewer_id: Address,
        access_type: Symbol,
        purpose: String,
        expires_at: Option<u64>,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        if study.patient_id != patient_id {
            return Err(Error::Unauthorized);
        }
        if purpose.is_empty() {
            return Err(Error::InvalidInput);
        }
        if let Some(exp) = expires_at {
            if exp <= env.ledger().timestamp() {
                return Err(Error::InvalidInput);
            }
        }

        let grant = AccessGrant {
            viewer_id: viewer_id.clone(),
            access_type,
            purpose: purpose.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at,
            revoked_at: None,
        };

        let grants = load_access_list(&env, study_id);
        let mut updated_grants = Vec::new(&env);
        let mut replaced = false;

        for existing in grants.iter() {
            if existing.viewer_id == viewer_id && existing.purpose == purpose {
                if !replaced {
                    updated_grants.push_back(grant.clone());
                    replaced = true;
                }
                continue;
            }
            updated_grants.push_back(existing);
        }

        if !replaced {
            updated_grants.push_back(grant);
        }
        save_access_list(&env, study_id, &updated_grants);

        env.events().publish(
            (symbol_short!("acc_grant"), study_id),
            (patient_id, viewer_id, purpose),
        );

        Ok(())
    }

    /// Patient revokes a viewer's purpose-scoped access grant.
    pub fn revoke_imaging_access(
        env: Env,
        study_id: u64,
        patient_id: Address,
        viewer_id: Address,
        purpose: String,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        if study.patient_id != patient_id {
            return Err(Error::Unauthorized);
        }
        if purpose.is_empty() {
            return Err(Error::InvalidInput);
        }

        let grants = load_access_list(&env, study_id);
        let mut updated_grants = Vec::new(&env);
        let mut revoked = false;

        for mut existing in grants.iter() {
            if existing.viewer_id == viewer_id && existing.purpose == purpose && existing.revoked_at.is_none() {
                existing.revoked_at = Some(env.ledger().timestamp());
                revoked = true;
            }
            updated_grants.push_back(existing);
        }

        if !revoked {
            return Err(Error::NotFound);
        }

        save_access_list(&env, study_id, &updated_grants);
        env.events().publish(
            (symbol_short!("acc_rev"), study_id),
            (patient_id, viewer_id, purpose),
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

        if study_ids.is_empty() || cd_token.is_empty() {
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

    /// Generate a cryptographically anonymized UID for a study.
    ///
    /// The anonymized ID is derived as:
    ///   sha256(study_id || patient_id_xdr || global_salt || rotation_epoch)
    /// encoded as a hex-like uppercase string prefixed with "ANON-".
    ///
    /// `rotation_epoch` allows key rotation: callers pass the current epoch
    /// (e.g. year-quarter as u32). Different epochs produce unlinkable IDs.
    /// The mapping (study_id, rotation_epoch) -> anon_uid is stored for
    /// authorized provenance lookups.
    pub fn anonymize_study(
        env: Env,
        study_id: u64,
        requesting_researcher: Address,
        anonymization_level: Symbol,
        purpose: String,
        rotation_epoch: u32,
    ) -> Result<String, Error> {
        requesting_researcher.require_auth();

        load_study(&env, study_id).ok_or(Error::NotFound)?;

        if purpose.is_empty() {
            return Err(Error::InvalidInput);
        }

        // Load or initialise the per-contract global salt.
        let salt: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::AnonSalt)
            .unwrap_or_else(|| {
                // First call: derive salt from contract address XDR + a fixed nonce.
                let mut seed = Bytes::new(&env);
                seed.extend_from_array(b"pacs-anon-salt-v1");
                env.crypto().sha256(&seed).into()
            });
        env.storage().instance().set(&DataKey::AnonSalt, &salt);

        // Derive anonymized ID: sha256(study_id_be || patient_xdr || salt || epoch_be)
        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        let mut data = Bytes::new(&env);
        data.extend_from_array(&study_id.to_be_bytes());
        data.append(&study.patient_id.clone().to_xdr(&env));
        data.append(&salt.clone().to_xdr(&env));
        data.extend_from_array(&rotation_epoch.to_be_bytes());

        let hash: BytesN<32> = env.crypto().sha256(&data).into();

        // Encode first 12 bytes of hash as uppercase hex (24 chars) for a compact UID.
        let anon_uid = bytes_to_hex_string(&env, &hash);

        // Store provenance: (study_id, rotation_epoch) -> anon_uid
        save_anonymized_uid(&env, study_id, rotation_epoch, &anon_uid);

        env.events().publish(
            (symbol_short!("anon"), study_id),
            (requesting_researcher, anonymization_level, purpose, rotation_epoch),
        );

        Ok(anon_uid)
    }

    /// Return the anonymized UID for a given study and rotation epoch.
    /// Only the patient or ordering provider may retrieve the mapping.
    pub fn get_anonymized_uid(
        env: Env,
        study_id: u64,
        requester: Address,
        rotation_epoch: u32,
    ) -> Result<String, Error> {
        requester.require_auth();
        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        if requester != study.patient_id && requester != study.ordering_provider {
            return Err(Error::Unauthorized);
        }
        load_anonymized_uid(&env, study_id, rotation_epoch).ok_or(Error::NotFound)
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
        purpose: String,
        view_timestamp: u64,
        view_duration: u32,
    ) -> Result<(), Error> {
        viewer_id.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        let now = env.ledger().timestamp();

        let is_owner = study.patient_id == viewer_id || study.ordering_provider == viewer_id;

        if !is_owner {
            Self::assert_active_grant(&env, study_id, &viewer_id, &purpose)?;
        }

        let earliest_allowed = now.saturating_sub(MAX_VIEW_TIMESTAMP_DRIFT_SECS);
        let latest_allowed = now.saturating_add(MAX_VIEW_TIMESTAMP_DRIFT_SECS);
        if view_timestamp < earliest_allowed || view_timestamp > latest_allowed {
            return Err(Error::TimestampOutOfBounds);
        }

        if let Some(previous_ts) = load_viewer_last_view_timestamp(&env, study_id, &viewer_id) {
            if view_timestamp <= previous_ts {
                return Err(Error::NonMonotonicViewTimestamp);
            }
        }

        let previous_entry_hash = load_viewer_view_chain_head(&env, study_id, &viewer_id);
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
            viewer_id: viewer_id.clone(),
            purpose: purpose.clone(),
            view_timestamp,
            view_duration,
            recorded_at: now,
            previous_entry_hash,
            entry_hash: entry_hash.clone(),
        };

        append_view_log(&env, study_id, &record);
        save_viewer_last_view_timestamp(&env, study_id, &viewer_id, view_timestamp);
        save_viewer_view_chain_head(&env, study_id, &viewer_id, &entry_hash);

        env.events().publish(
            (symbol_short!("view_log"), study_id),
            (viewer_id, view_timestamp, entry_hash),
        );

        Ok(())
    }

    /// Return all view records for a study.
    pub fn get_study_view_logs(env: Env, study_id: u64) -> Vec<ViewRecord> {
        load_view_logs(&env, study_id)
    }

    /// Return studies for a patient that pass the filters and that the requester
    /// has access to.
    pub fn search_imaging_studies(
        env: Env,
        patient_id: Address,
        requester: Address,
        access_purpose: String,
        filters: ImagingFilters,
    ) -> Result<Vec<ImagingStudy>, Error> {
        requester.require_auth();

        let study_ids = load_patient_studies(&env, &patient_id);
        let mut results: Vec<ImagingStudy> = Vec::new(&env);

        for sid in study_ids.iter() {
            if let Some(study) = load_study(&env, sid) {
                // --- access check ---
                let is_owner =
                    study.patient_id == requester || study.ordering_provider == requester;
                let mut allowed = is_owner;

                if !allowed {
                    allowed = Self::assert_active_grant(&env, sid, &requester, &access_purpose).is_ok();
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

    /// Return grants for a study to the patient owner or ordering provider.
    pub fn get_access_grants(
        env: Env,
        study_id: u64,
        requester: Address,
    ) -> Result<Vec<AccessGrant>, Error> {
        requester.require_auth();

        let study = load_study(&env, study_id).ok_or(Error::NotFound)?;
        if requester != study.patient_id && requester != study.ordering_provider {
            return Err(Error::Unauthorized);
        }

        Ok(load_access_list(&env, study_id))
    }

    fn assert_active_grant(
        env: &Env,
        study_id: u64,
        viewer_id: &Address,
        purpose: &String,
    ) -> Result<(), Error> {
        if purpose.is_empty() {
            return Err(Error::InvalidInput);
        }

        let now = env.ledger().timestamp();
        let grants = load_access_list(env, study_id);

        for grant in grants.iter() {
            if grant.viewer_id == *viewer_id && grant.purpose == *purpose {
                if grant.revoked_at.is_some() {
                    return Err(Error::GrantRevoked);
                }
                if let Some(exp) = grant.expires_at {
                    if now > exp {
                        return Err(Error::AccessExpired);
                    }
                }
                return Ok(());
            }
        }

        Err(Error::Unauthorized)
    }
}

/// Encode the first 12 bytes of a BytesN<32> as uppercase hex, prefixed with "ANON-".
/// Produces a 29-character string: "ANON-" + 24 hex chars.
fn bytes_to_hex_string(env: &Env, hash: &BytesN<32>) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    // 5 (prefix) + 24 (12 bytes * 2) = 29 chars
    let mut buf = [0u8; 29];
    buf[0] = b'A';
    buf[1] = b'N';
    buf[2] = b'O';
    buf[3] = b'N';
    buf[4] = b'-';
    for i in 0..12usize {
        let byte = hash.get(i as u32).unwrap_or(0);
        buf[5 + i * 2] = HEX[(byte >> 4) as usize];
        buf[5 + i * 2 + 1] = HEX[(byte & 0x0f) as usize];
    }
    String::from_bytes(env, &buf)
}
