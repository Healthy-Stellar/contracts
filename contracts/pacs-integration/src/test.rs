#![cfg(test)]

use soroban_sdk::{testutils::{Address as _, Ledger}, Address, BytesN, Env, String, Symbol, Vec};

use crate::types::{ComparisonCriteria, Error, ImagingFilters};
use crate::{PacsContract, PacsContractClient};

// ─── helpers ────────────────────────────────────────────────────────────────

fn setup(env: &Env) -> (PacsContractClient<'_>, Address, Address) {
    let id = env.register(PacsContract, ());
    let client = PacsContractClient::new(env, &id);
    let patient = Address::generate(env);
    let provider = Address::generate(env);
    (client, patient, provider)
}

fn dummy_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0xABu8; 32])
}

fn register_ct_chest<'a>(
    env: &Env,
    client: &PacsContractClient<'a>,
    patient: &Address,
    provider: &Address,
) -> u64 {
    client.register_imaging_study(
        patient,
        provider,
        &String::from_str(env, "1.2.840.10008.5.1.4.1.1.2"),
        &Symbol::new(env, "CT"),
        &String::from_str(env, "Chest"),
        &1_700_000_000_u64,
        &String::from_str(env, "CT Chest w contrast"),
        &2_u32,
        &40_u32,
        &dummy_hash(env),
    )
}

// ─── tests ──────────────────────────────────────────────────────────────────

#[test]
fn register_study_increments_id() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    assert_eq!(register_ct_chest(&env, &client, &patient, &provider), 1);
    assert_eq!(register_ct_chest(&env, &client, &patient, &provider), 2);
}

#[test]
fn register_study_empty_uid_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    let result = client.try_register_imaging_study(
        &patient,
        &provider,
        &String::from_str(&env, ""),
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Chest"),
        &1_700_000_000_u64,
        &String::from_str(&env, "desc"),
        &1_u32,
        &10_u32,
        &dummy_hash(&env),
    );
    assert!(result.is_err());
}

#[test]
fn add_series_to_study_ok() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    let sid = register_ct_chest(&env, &client, &patient, &provider);

    client.add_series_to_study(
        &sid,
        &String::from_str(&env, "1.2.3.4.5.1"),
        &1_u32,
        &String::from_str(&env, "Axial"),
        &20_u32,
        &1_700_000_100_u64,
    );
}

#[test]
fn add_series_nonexistent_study_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _patient, _provider) = setup(&env);

    let result = client.try_add_series_to_study(
        &99_u64,
        &String::from_str(&env, "1.2.3"),
        &1_u32,
        &String::from_str(&env, "ax"),
        &5_u32,
        &0_u64,
    );
    assert!(result.is_err());
}

#[test]
fn link_report_and_addendum_ok() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let rad = Address::generate(&env);

    client.link_imaging_report(
        &sid,
        &rad,
        &Symbol::new(&env, "final"),
        &dummy_hash(&env),
        &false,
    );
    client.link_imaging_report(
        &sid,
        &rad,
        &Symbol::new(&env, "addendum"),
        &dummy_hash(&env),
        &true,
    );
}

#[test]
fn duplicate_final_report_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let rad = Address::generate(&env);

    client.link_imaging_report(
        &sid,
        &rad,
        &Symbol::new(&env, "final"),
        &dummy_hash(&env),
        &false,
    );
    let result = client.try_link_imaging_report(
        &sid,
        &rad,
        &Symbol::new(&env, "final"),
        &dummy_hash(&env),
        &false,
    );
    assert!(result.is_err());
}

#[test]
fn comparison_study_returns_prior_match() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    let prior = register_ct_chest(&env, &client, &patient, &provider);
    let current = register_ct_chest(&env, &client, &patient, &provider);

    // Ordering provider is always authorized.
    let criteria = ComparisonCriteria {
        modality: Some(Symbol::new(&env, "CT")),
        body_part: String::from_str(&env, "Chest"),
        max_age_days: 365,
        same_side: false,
    };

    let matches = client.request_comparison_study(&current, &provider, &criteria);
    assert!(matches.contains(prior));
    assert!(!matches.contains(current));
}

#[test]
fn comparison_study_granted_radiologist_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    let prior = register_ct_chest(&env, &client, &patient, &provider);
    let current = register_ct_chest(&env, &client, &patient, &provider);

    let rad = Address::generate(&env);
    // Patient grants the radiologist access to the current study.
    client.grant_imaging_access(
        &current,
        &patient,
        &rad,
        &Symbol::new(&env, "view_only"),
        &String::from_str(&env, "comparison-read"),
        &None,
    );

    let criteria = ComparisonCriteria {
        modality: Some(Symbol::new(&env, "CT")),
        body_part: String::from_str(&env, "Chest"),
        max_age_days: 365,
        same_side: false,
    };

    let matches = client.request_comparison_study(&current, &rad, &criteria);
    assert!(matches.contains(prior));
    assert!(!matches.contains(current));
}

#[test]
fn comparison_study_unauthorized_caller_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    register_ct_chest(&env, &client, &patient, &provider);
    let current = register_ct_chest(&env, &client, &patient, &provider);

    // Unrelated address — not ordering provider, no grant.
    let stranger = Address::generate(&env);
    let criteria = ComparisonCriteria {
        modality: Some(Symbol::new(&env, "CT")),
        body_part: String::from_str(&env, "Chest"),
        max_age_days: 365,
        same_side: false,
    };

    let result = client.try_request_comparison_study(&current, &stranger, &criteria);
    assert!(matches!(result, Err(Ok(Error::Unauthorized))));
}

#[test]
fn comparison_study_wrong_modality_no_match() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    register_ct_chest(&env, &client, &patient, &provider);
    let current = register_ct_chest(&env, &client, &patient, &provider);

    // Provider is the authorized caller; test is purely about filter logic.
    let criteria = ComparisonCriteria {
        modality: Some(Symbol::new(&env, "MRI")),
        body_part: String::from_str(&env, "Chest"),
        max_age_days: 365,
        same_side: false,
    };
    let matches = client.request_comparison_study(&current, &provider, &criteria);
    assert_eq!(matches.len(), 0);
}

#[test]
fn grant_access_and_track_view() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let viewer = Address::generate(&env);

    client.grant_imaging_access(
        &sid,
        &patient,
        &viewer,
        &Symbol::new(&env, "view_only"),
        &String::from_str(&env, "clinical-review"),
        &None,
    );
    env.ledger().set_timestamp(1_000);
    client.track_study_views(
        &sid,
        &viewer,
        &String::from_str(&env, "clinical-review"),
        &1_000_u64,
        &30_u32,
    );
}

#[test]
fn patient_and_provider_can_view_without_grant() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);

    env.ledger().set_timestamp(1_100);
    client.track_study_views(
        &sid,
        &patient,
        &String::from_str(&env, ""),
        &1_100_u64,
        &10_u32,
    );
    env.ledger().set_timestamp(1_200);
    client.track_study_views(
        &sid,
        &provider,
        &String::from_str(&env, ""),
        &1_200_u64,
        &5_u32,
    );
}

#[test]
fn unauthorized_viewer_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let stranger = Address::generate(&env);

    let result = client.try_track_study_views(
        &sid,
        &stranger,
        &String::from_str(&env, "clinical-review"),
        &0_u64,
        &0_u32,
    );
    assert!(result.is_err());
}

#[test]
fn create_imaging_cd_ok() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    let s1 = register_ct_chest(&env, &client, &patient, &provider);
    let s2 = register_ct_chest(&env, &client, &patient, &provider);

    let mut ids: Vec<u64> = Vec::new(&env);
    ids.push_back(s1);
    ids.push_back(s2);

    let cd_id = client.create_imaging_cd(
        &ids,
        &patient,
        &provider,
        &String::from_str(&env, "TOKEN-XYZ"),
        &1_700_002_000_u64,
    );
    assert_eq!(cd_id, 1);
}

#[test]
fn anonymize_study_returns_uid_for_patient() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);

    // Patient (study owner) may anonymize.
    let uid = client.anonymize_study(
        &sid,
        &patient,
        &Symbol::new(&env, "full"),
        &String::from_str(&env, "cancer study"),
        &1_u32,
    );
    assert!(!uid.is_empty());

    // Different epoch produces an unlinkable UID.
    let uid2 = client.anonymize_study(
        &sid,
        &patient,
        &Symbol::new(&env, "full"),
        &String::from_str(&env, "cancer study"),
        &2_u32,
    );
    assert_ne!(uid, uid2);
}

#[test]
fn anonymize_study_returns_uid_for_ordering_provider() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);

    // Ordering provider may also anonymize without a separate grant.
    let uid = client.anonymize_study(
        &sid,
        &provider,
        &Symbol::new(&env, "full"),
        &String::from_str(&env, "research"),
        &1_u32,
    );
    assert!(!uid.is_empty());
}

#[test]
fn anonymize_study_returns_uid_for_granted_researcher() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let researcher = Address::generate(&env);

    // Patient grants the researcher access with the matching purpose.
    client.grant_imaging_access(
        &sid,
        &patient,
        &researcher,
        &Symbol::new(&env, "research"),
        &String::from_str(&env, "cancer study"),
        &None,
    );

    let uid = client.anonymize_study(
        &sid,
        &researcher,
        &Symbol::new(&env, "full"),
        &String::from_str(&env, "cancer study"),
        &1_u32,
    );
    assert!(!uid.is_empty());
}

#[test]
fn anonymize_study_unauthorized_researcher_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let stranger = Address::generate(&env);

    // No grant and not patient/provider — must be rejected.
    let result = client.try_anonymize_study(
        &sid,
        &stranger,
        &Symbol::new(&env, "full"),
        &String::from_str(&env, "cancer study"),
        &1_u32,
    );
    assert!(matches!(result, Err(Ok(Error::Unauthorized))));
}

#[test]
fn quality_control_review_ok() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let reviewer = Address::generate(&env);

    let mut issues: Vec<String> = Vec::new(&env);
    issues.push_back(String::from_str(&env, "motion artifact"));

    client.quality_control_review(&sid, &reviewer, &85_u32, &issues, &false);
}

#[test]
fn qc_score_above_100_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let reviewer = Address::generate(&env);

    let issues: Vec<String> = Vec::new(&env);
    let result = client.try_quality_control_review(&sid, &reviewer, &101_u32, &issues, &false);
    assert!(result.is_err());
}

#[test]
fn search_studies_modality_filter() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    register_ct_chest(&env, &client, &patient, &provider); // CT

    let filters = ImagingFilters {
        modality: Some(Symbol::new(&env, "CT")),
        body_part: None,
        start_date: None,
        end_date: None,
        has_critical_findings: None,
    };
    let results = client.search_imaging_studies(
        &patient,
        &patient,
        &String::from_str(&env, ""),
        &filters,
    );
    assert_eq!(results.len(), 1);
}

#[test]
fn search_studies_wrong_modality_no_results() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    register_ct_chest(&env, &client, &patient, &provider);

    let filters = ImagingFilters {
        modality: Some(Symbol::new(&env, "MRI")),
        body_part: None,
        start_date: None,
        end_date: None,
        has_critical_findings: None,
    };
    let results = client.search_imaging_studies(
        &patient,
        &patient,
        &String::from_str(&env, ""),
        &filters,
    );
    assert_eq!(results.len(), 0);
}

#[test]
fn search_studies_critical_findings_filter() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let rad = Address::generate(&env);

    // mark study as having critical findings
    client.link_imaging_report(
        &sid,
        &rad,
        &Symbol::new(&env, "final"),
        &dummy_hash(&env),
        &true,
    );

    let filters = ImagingFilters {
        modality: None,
        body_part: None,
        start_date: None,
        end_date: None,
        has_critical_findings: Some(true),
    };
    let results = client.search_imaging_studies(
        &patient,
        &patient,
        &String::from_str(&env, ""),
        &filters,
    );
    assert_eq!(results.len(), 1);
}

#[test]
fn repeated_grant_deduplicates_by_viewer_and_purpose() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let viewer = Address::generate(&env);

    client.grant_imaging_access(
        &sid,
        &patient,
        &viewer,
        &Symbol::new(&env, "view_only"),
        &String::from_str(&env, "tumor-board"),
        &Some(1_700_100_000_u64),
    );
    client.grant_imaging_access(
        &sid,
        &patient,
        &viewer,
        &Symbol::new(&env, "download"),
        &String::from_str(&env, "tumor-board"),
        &Some(1_700_200_000_u64),
    );

    let grants = client.get_access_grants(&sid, &patient);
    assert_eq!(grants.len(), 1);
    let grant = grants.get(0).unwrap();
    assert_eq!(grant.access_type, Symbol::new(&env, "download"));
    assert_eq!(grant.purpose, String::from_str(&env, "tumor-board"));
    assert_eq!(grant.expires_at, Some(1_700_200_000_u64));
}

#[test]
fn revoked_grant_blocks_subsequent_reads() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let viewer = Address::generate(&env);

    client.grant_imaging_access(
        &sid,
        &patient,
        &viewer,
        &Symbol::new(&env, "view_only"),
        &String::from_str(&env, "qa-review"),
        &None,
    );
    client.revoke_imaging_access(
        &sid,
        &patient,
        &viewer,
        &String::from_str(&env, "qa-review"),
    );

    env.ledger().set_timestamp(1_500);
    let result = client.try_track_study_views(
        &sid,
        &viewer,
        &String::from_str(&env, "qa-review"),
        &1_500_u64,
        &15_u32,
    );
    assert!(matches!(result, Err(Ok(Error::GrantRevoked))));
}

#[test]
fn wrong_purpose_cannot_use_grant() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let viewer = Address::generate(&env);

    client.grant_imaging_access(
        &sid,
        &patient,
        &viewer,
        &Symbol::new(&env, "view_only"),
        &String::from_str(&env, "care-coordination"),
        &None,
    );

    env.ledger().set_timestamp(1_600);
    let result = client.try_track_study_views(
        &sid,
        &viewer,
        &String::from_str(&env, "research"),
        &1_600_u64,
        &12_u32,
    );
    assert!(matches!(result, Err(Ok(Error::Unauthorized))));
}

#[test]
fn view_timestamp_outside_drift_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let viewer = Address::generate(&env);

    client.grant_imaging_access(
        &sid,
        &patient,
        &viewer,
        &Symbol::new(&env, "view_only"),
        &String::from_str(&env, "audit"),
        &None,
    );

    env.ledger().set_timestamp(10_000);
    let result = client.try_track_study_views(
        &sid,
        &viewer,
        &String::from_str(&env, "audit"),
        &9_699_u64, // 301s behind ledger time
        &20_u32,
    );
    assert!(matches!(result, Err(Ok(Error::TimestampOutOfBounds))));
}

#[test]
fn view_timestamp_must_be_monotonic_per_viewer_study() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let viewer = Address::generate(&env);

    client.grant_imaging_access(
        &sid,
        &patient,
        &viewer,
        &Symbol::new(&env, "view_only"),
        &String::from_str(&env, "audit"),
        &None,
    );

    env.ledger().set_timestamp(2_000);
    client.track_study_views(
        &sid,
        &viewer,
        &String::from_str(&env, "audit"),
        &2_000_u64,
        &10_u32,
    );

    env.ledger().set_timestamp(2_010);
    let result = client.try_track_study_views(
        &sid,
        &viewer,
        &String::from_str(&env, "audit"),
        &2_000_u64,
        &8_u32,
    );
    assert!(matches!(result, Err(Ok(Error::NonMonotonicViewTimestamp))));
}

#[test]
fn view_records_include_hash_chain_links() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let viewer = Address::generate(&env);

    client.grant_imaging_access(
        &sid,
        &patient,
        &viewer,
        &Symbol::new(&env, "view_only"),
        &String::from_str(&env, "audit"),
        &None,
    );

    env.ledger().set_timestamp(3_000);
    client.track_study_views(
        &sid,
        &viewer,
        &String::from_str(&env, "audit"),
        &3_000_u64,
        &7_u32,
    );
    env.ledger().set_timestamp(3_020);
    client.track_study_views(
        &sid,
        &viewer,
        &String::from_str(&env, "audit"),
        &3_020_u64,
        &9_u32,
    );

    let logs = client.get_study_view_logs(&sid);
    assert_eq!(logs.len(), 2);

    let first = logs.get(0).unwrap();
    assert!(first.previous_entry_hash.is_none());
    let second = logs.get(1).unwrap();
    assert_eq!(second.previous_entry_hash, Some(first.entry_hash.clone()));
}

#[test]
fn test_acknowledge_critical_finding() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let contract_id = env.register(PacsContract, ());
    let client = PacsContractClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let radiologist = Address::generate(&env);

    // Register study
    let sid = client.register_imaging_study(
        &patient,
        &provider,
        &String::from_str(&env, "1.2.3.4.5"),
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Chest"),
        &1000u64,
        &String::from_str(&env, "CT Chest"),
        &1u32,
        &1u32,
        &dummy_hash(&env),
    );

    // Link report with critical findings
    client.link_imaging_report(
        &sid,
        &radiologist,
        &Symbol::new(&env, "final"),
        &dummy_hash(&env),
        &true, // critical_findings = true
    );

    // Acknowledge critical finding
    env.ledger().set_timestamp(2000);
    client.acknowledge_critical_finding(&sid, &provider, &2000u64);

    // Verify acknowledgment was recorded
    let study = client.get_imaging_study(&sid);
    assert!(study.critical_findings);
}

#[test]
fn test_acknowledge_critical_finding_only_by_ordering_provider() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let contract_id = env.register(PacsContract, ());
    let client = PacsContractClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let other_provider = Address::generate(&env);
    let radiologist = Address::generate(&env);

    // Register study
    let sid = client.register_imaging_study(
        &patient,
        &provider,
        &String::from_str(&env, "1.2.3.4.5"),
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Chest"),
        &1000u64,
        &String::from_str(&env, "CT Chest"),
        &1u32,
        &1u32,
        &dummy_hash(&env),
    );

    // Link report with critical findings
    client.link_imaging_report(
        &sid,
        &radiologist,
        &Symbol::new(&env, "final"),
        &dummy_hash(&env),
        &true,
    );

    // Try to acknowledge with different provider - should fail
    let result = client.try_acknowledge_critical_finding(&sid, &other_provider, &2000u64);
    assert!(result.is_err());
}

#[test]
fn test_list_unack_crit_findings() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let contract_id = env.register(PacsContract, ());
    let client = PacsContractClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let radiologist = Address::generate(&env);

    // Register first study with critical findings (unacknowledged)
    let sid1 = client.register_imaging_study(
        &patient,
        &provider,
        &String::from_str(&env, "1.2.3.4.5"),
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Chest"),
        &1000u64,
        &String::from_str(&env, "CT Chest"),
        &1u32,
        &1u32,
        &dummy_hash(&env),
    );

    client.link_imaging_report(
        &sid1,
        &radiologist,
        &Symbol::new(&env, "final"),
        &dummy_hash(&env),
        &true,
    );

    // Register second study with critical findings (acknowledged)
    let sid2 = client.register_imaging_study(
        &patient,
        &provider,
        &String::from_str(&env, "1.2.3.4.6"),
        &Symbol::new(&env, "MR"),
        &String::from_str(&env, "Brain"),
        &1100u64,
        &String::from_str(&env, "MR Brain"),
        &1u32,
        &1u32,
        &dummy_hash(&env),
    );

    client.link_imaging_report(
        &sid2,
        &radiologist,
        &Symbol::new(&env, "final"),
        &dummy_hash(&env),
        &true,
    );

    // Advance time and acknowledge second study
    env.ledger().set_timestamp(2000);
    client.acknowledge_critical_finding(&sid2, &provider, &2000u64);

    // Query unacknowledged findings older than 500 seconds
    let unacknowledged = client.list_unack_crit_findings(&provider, &500u64);
    // Only the first study (sid1) should be in the result since the second study is acknowledged
    assert_eq!(unacknowledged.len(), 1);
    assert_eq!(unacknowledged.get(0).unwrap(), sid1);
}

#[test]
fn test_create_imaging_cd_unauthorized_provider() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    let s1 = register_ct_chest(&env, &client, &patient, &provider);

    let mut ids: Vec<u64> = Vec::new(&env);
    ids.push_back(s1);

    let stranger = Address::generate(&env);

    let result = client.try_create_imaging_cd(
        &ids,
        &patient,
        &stranger,
        &String::from_str(&env, "TOKEN-XYZ"),
        &1_700_002_000_u64,
    );
    assert!(result.is_err());
}
