#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec};

use crate::{PacsContract, PacsContractClient};
use crate::types::{ComparisonCriteria, ImagingFilters};

// ─── helpers ────────────────────────────────────────────────────────────────

fn setup(env: &Env) -> (PacsContractClient, Address, Address) {
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

    let rad = Address::generate(&env);
    let criteria = ComparisonCriteria {
        modality: Some(Symbol::new(&env, "CT")),
        body_part: String::from_str(&env, "Chest"),
        max_age_days: 365,
        same_side: false,
    };

    let matches = client.request_comparison_study(&current, &rad, &criteria);
    assert!(matches.contains(&prior));
    assert!(!matches.contains(&current));
}

#[test]
fn comparison_study_wrong_modality_no_match() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);

    register_ct_chest(&env, &client, &patient, &provider);
    let current = register_ct_chest(&env, &client, &patient, &provider);

    let rad = Address::generate(&env);
    let criteria = ComparisonCriteria {
        modality: Some(Symbol::new(&env, "MRI")),
        body_part: String::from_str(&env, "Chest"),
        max_age_days: 365,
        same_side: false,
    };
    let matches = client.request_comparison_study(&current, &rad, &criteria);
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
        &None,
    );
    client.track_study_views(&sid, &viewer, &1_700_001_000_u64, &30_u32);
}

#[test]
fn patient_and_provider_can_view_without_grant() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);

    client.track_study_views(&sid, &patient, &1_700_001_100_u64, &10_u32);
    client.track_study_views(&sid, &provider, &1_700_001_200_u64, &5_u32);
}

#[test]
fn unauthorized_viewer_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let stranger = Address::generate(&env);

    let result = client.try_track_study_views(&sid, &stranger, &0_u64, &0_u32);
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
fn anonymize_study_returns_uid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, provider) = setup(&env);
    let sid = register_ct_chest(&env, &client, &patient, &provider);
    let researcher = Address::generate(&env);

    let uid = client.anonymize_study(
        &sid,
        &researcher,
        &Symbol::new(&env, "full"),
        &String::from_str(&env, "cancer study"),
    );
    assert!(uid.len() > 0);
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
    let results = client.search_imaging_studies(&patient, &patient, &filters);
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
    let results = client.search_imaging_studies(&patient, &patient, &filters);
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
    let results = client.search_imaging_studies(&patient, &patient, &filters);
    assert_eq!(results.len(), 1);
}
