#[cfg(test)]
mod tests {
    use crate::{Error, HealthRecords, HealthRecordsClient};
    use patient_registry::{MedicalRegistry, MedicalRegistryClient};
    use provider_registry::{ProviderRegistry, ProviderRegistryClient};
    use shared::privacy::{EncryptedEnvelopeRef, PolicyMetadata};
    use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, String, Symbol, Vec};

    fn encrypted_ref(env: &Env, seed: u8) -> EncryptedEnvelopeRef {
        EncryptedEnvelopeRef {
            content_hash: BytesN::from_array(env, &[seed; 32]),
            envelope_uri: String::from_str(env, "enc+ipfs://bafyvalidhealthref"),
            key_version_id: String::from_str(env, "kv:v01"),
        }
    }

    fn policy(env: &Env) -> PolicyMetadata {
        PolicyMetadata {
            retention_class: Symbol::new(env, "clinical"),
            access_policy_hash: BytesN::from_array(env, &[7u8; 32]),
            purpose: Symbol::new(env, "treatment"),
        }
    }

    fn setup(env: &Env) -> (HealthRecordsClient<'static>, Address, Address) {
        let contract_id = env.register(HealthRecords, ());
        let client = HealthRecordsClient::new(env, &contract_id);
        let patient = Address::generate(env);
        let provider = Address::generate(env);
        (client, patient, provider)
    }

    #[test]
    fn test_create_record_with_consent() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);

        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "LAB_RESULT");

        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));
        let record = client.get_record(&patient, &record_id);

        assert_eq!(record.integrity_hash.len(), 32);
        let hash_bytes: Bytes = record.integrity_hash.into();
        assert_ne!(hash_bytes, Bytes::new(&env));
    }

    #[test]
    fn test_create_record_without_consent_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "LAB_RESULT");

        let result =
            client.try_create_record(&patient, &provider, &reference, &rtype, &policy(&env));
        assert_eq!(result, Err(Ok(Error::ConsentNotGranted)));
    }

    #[test]
    fn test_get_record_by_patient() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "PRESCRIPTION");
        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));

        let record = client.get_record(&patient, &record_id);
        assert_eq!(record.record_id, record_id);
    }

    #[test]
    fn test_get_record_by_consented_provider() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "DIAGNOSIS");
        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));

        let record = client.get_record(&provider, &record_id);
        assert_eq!(record.record_id, record_id);
    }

    #[test]
    fn test_get_record_unauthorized_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);
        let stranger = Address::generate(&env);

        client.grant_consent(&patient, &provider);
        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "XRAY");
        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));

        let result = client.try_get_record(&stranger, &record_id);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    }

    #[test]
    fn test_get_record_after_consent_revoked_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "LAB");
        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));

        client.revoke_consent(&patient, &provider);

        let result = client.try_get_record(&provider, &record_id);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    }

    #[test]
    fn test_verify_record_integrity_valid() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "PRESCRIPTION");
        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));
        let record = client.get_record(&patient, &record_id);

        let stored_hash: Bytes = record.integrity_hash.into();
        assert!(client.verify_record_integrity(&patient, &record_id, &stored_hash));
    }

    #[test]
    fn test_verify_record_integrity_tampered_returns_false() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "DIAGNOSIS");
        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));

        let tampered_hash = Bytes::from_array(&env, &[0u8; 32]);
        assert!(!client.verify_record_integrity(&patient, &record_id, &tampered_hash));
    }

    #[test]
    fn test_verify_integrity_unauthorized_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);
        let stranger = Address::generate(&env);

        client.grant_consent(&patient, &provider);
        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "XRAY");
        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));
        let record = client.get_record(&patient, &record_id);
        let hash: Bytes = record.integrity_hash.into();

        let result = client.try_verify_record_integrity(&stranger, &record_id, &hash);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    }

    #[test]
    fn test_verify_nonexistent_record_returns_false() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, _) = setup(&env);

        let hash = Bytes::from_array(&env, &[0u8; 32]);
        assert!(!client.verify_record_integrity(&patient, &999u64, &hash));
    }

    #[test]
    fn test_verify_wrong_length_hash_returns_false() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let reference = encrypted_ref(&env, 1);
        let rtype = String::from_str(&env, "XRAY");
        let record_id =
            client.create_record(&patient, &provider, &reference, &rtype, &policy(&env));

        let short_hash = Bytes::from_array(&env, &[0u8; 16]);
        assert!(!client.verify_record_integrity(&patient, &record_id, &short_hash));
    }
}

#[cfg(test)]
mod cross_contract_correlation_tests {
    use crate::{HealthRecords, HealthRecordsClient};
    use patient_registry::{MedicalRegistry, MedicalRegistryClient};
    use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String};

    fn corr_id(env: &Env, seed: u8) -> BytesN<32> {
        BytesN::from_array(env, &[seed; 32])
    }

    fn setup_both(
        env: &Env,
    ) -> (
        HealthRecordsClient<'static>,
        MedicalRegistryClient<'static>,
    ) {
        let hr_id = env.register(HealthRecords, ());
        let pr_id = env.register(MedicalRegistry, ());
        (
            HealthRecordsClient::new(env, &hr_id),
            MedicalRegistryClient::new(env, &pr_id),
        )
    }

    /// Scenario 1: same correlation ID links one incident in each contract.
    #[test]
    fn test_correlated_incidents_appear_in_both_contracts() {
        let env = Env::default();
        env.mock_all_auths();
        let (hr, pr) = setup_both(&env);
        let reporter = Address::generate(&env);
        let cid = corr_id(&env, 0xAA);

        let hr_incident = hr.report_incident(
            &reporter,
            &1u32,
            &String::from_str(&env, "hr: unauthorized access"),
            &Some(cid.clone()),
        );
        let pr_incident = pr.report_incident(
            &reporter,
            &1u32,
            &String::from_str(&env, "pr: patient not found"),
            &Some(cid.clone()),
        );

        // Each contract's own storage holds the incident under the correlation index.
        let hr_ids = hr.get_incidents_by_correlation_id(&cid);
        let pr_ids = pr.get_incidents_by_correlation_id(&cid);

        assert_eq!(hr_ids.len(), 1);
        assert_eq!(hr_ids.get(0).unwrap(), hr_incident);

        assert_eq!(pr_ids.len(), 1);
        assert_eq!(pr_ids.get(0).unwrap(), pr_incident);
    }

    /// Scenario 2: multiple incidents across both contracts share one correlation ID.
    #[test]
    fn test_multiple_incidents_same_correlation_id() {
        let env = Env::default();
        env.mock_all_auths();
        let (hr, pr) = setup_both(&env);
        let reporter = Address::generate(&env);
        let cid = corr_id(&env, 0xBB);

        hr.report_incident(
            &reporter,
            &2u32,
            &String::from_str(&env, "hr: consent revoked"),
            &Some(cid.clone()),
        );
        hr.report_incident(
            &reporter,
            &3u32,
            &String::from_str(&env, "hr: integrity check failed"),
            &Some(cid.clone()),
        );
        pr.report_incident(
            &reporter,
            &2u32,
            &String::from_str(&env, "pr: duplicate registration"),
            &Some(cid.clone()),
        );

        assert_eq!(hr.get_incidents_by_correlation_id(&cid).len(), 2);
        assert_eq!(pr.get_incidents_by_correlation_id(&cid).len(), 1);
    }

    /// Scenario 3: incidents without a correlation ID are not returned by the query.
    #[test]
    fn test_uncorrelated_incidents_not_returned() {
        let env = Env::default();
        env.mock_all_auths();
        let (hr, pr) = setup_both(&env);
        let reporter = Address::generate(&env);
        let cid = corr_id(&env, 0xCC);

        // Fire incidents with no correlation ID.
        hr.report_incident(
            &reporter,
            &9u32,
            &String::from_str(&env, "hr: unrelated error"),
            &None,
        );
        pr.report_incident(
            &reporter,
            &9u32,
            &String::from_str(&env, "pr: unrelated error"),
            &None,
        );

        // The correlation index for `cid` must be empty.
        assert_eq!(hr.get_incidents_by_correlation_id(&cid).len(), 0);
        assert_eq!(pr.get_incidents_by_correlation_id(&cid).len(), 0);
    }

    /// Scenario 4: different correlation IDs are kept isolated from each other.
    #[test]
    fn test_different_correlation_ids_are_isolated() {
        let env = Env::default();
        env.mock_all_auths();
        let (hr, _pr) = setup_both(&env);
        let reporter = Address::generate(&env);
        let cid_a = corr_id(&env, 0x01);
        let cid_b = corr_id(&env, 0x02);

        hr.report_incident(
            &reporter,
            &1u32,
            &String::from_str(&env, "incident for A"),
            &Some(cid_a.clone()),
        );
        hr.report_incident(
            &reporter,
            &2u32,
            &String::from_str(&env, "incident for B"),
            &Some(cid_b.clone()),
        );

        assert_eq!(hr.get_incidents_by_correlation_id(&cid_a).len(), 1);
        assert_eq!(hr.get_incidents_by_correlation_id(&cid_b).len(), 1);
    }
}

    mod cross_contract_workflow_tests {
        use super::*;
        use patient_registry::{MedicalRegistry, MedicalRegistryClient};
        use provider_registry::{ProviderRegistry, ProviderRegistryClient};
        use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec};

        #[test]
        fn test_provider_patient_healthrecord_record_creation_flow() {
            let env = Env::default();
            env.mock_all_auths();

            let provider_registry_id = env.register_contract(None, ProviderRegistry);
            let patient_registry_id = env.register_contract(None, MedicalRegistry);
            let hr_contract_id = env.register_contract(None, HealthRecords);

            let provider_client = ProviderRegistryClient::new(&env, &provider_registry_id);
            let patient_client = MedicalRegistryClient::new(&env, &patient_registry_id);
            let hr_client = HealthRecordsClient::new(&env, &hr_contract_id);

            let admin = Address::generate(&env);
            let provider = Address::generate(&env);
            let patient = Address::generate(&env);

            provider_client.initialize(&admin);
            provider_client.register_provider(
                &admin,
                &provider,
                &String::from_str(&env, "Provider One"),
                &String::from_str(&env, "General Practice"),
                &String::from_str(&env, "LIC-100"),
                &BytesN::from_array(&env, &[1u8; 32]),
                &Address::generate(&env),
                &BytesN::from_array(&env, &[2u8; 32]),
                &u64::MAX,
                &BytesN::from_array(&env, &[3u8; 32]),
            );

            patient_client.register_patient(
                &patient,
                &String::from_str(&env, "Alice Patient"),
                &631152000u64,
                &encrypted_ref(&env, 5),
                &policy(&env),
            );

            let provider_is_registered: bool = env.invoke_contract(
                &provider_registry_id,
                &Symbol::new(&env, "is_provider"),
                vec![&env, provider.clone().into_val(&env)],
            );
            assert!(provider_is_registered);

            let patient_is_registered: bool = env.invoke_contract(
                &patient_registry_id,
                &Symbol::new(&env, "is_patient_registered"),
                vec![&env, patient.clone().into_val(&env)],
            );
            assert!(patient_is_registered);

            hr_client.grant_consent(&patient, &provider);

            let record_id = hr_client
                .create_record(
                    &patient,
                    &provider,
                    &encrypted_ref(&env, 9),
                    &String::from_str(&env, "DIAGNOSIS"),
                    &policy(&env),
                )
                .unwrap();
            let record = hr_client.get_record(&patient, &record_id);

            assert_eq!(record.patient, patient);
            assert_eq!(record.provider, provider);
            assert_eq!(record.record_type, String::from_str(&env, "DIAGNOSIS"));
        }

        #[test]
        fn test_cross_contract_error_propagation_for_missing_consent() {
            let env = Env::default();
            env.mock_all_auths();

            let provider_registry_id = env.register_contract(None, ProviderRegistry);
            let patient_registry_id = env.register_contract(None, MedicalRegistry);
            let hr_contract_id = env.register_contract(None, HealthRecords);

            let provider_client = ProviderRegistryClient::new(&env, &provider_registry_id);
            let patient_client = MedicalRegistryClient::new(&env, &patient_registry_id);

            let provider = Address::generate(&env);
            let patient = Address::generate(&env);

            provider_client.initialize(&Address::generate(&env));
            provider_client.register_provider(
                &Address::generate(&env),
                &provider,
                &String::from_str(&env, "Provider Two"),
                &String::from_str(&env, "Specialty"),
                &String::from_str(&env, "LIC-200"),
                &BytesN::from_array(&env, &[4u8; 32]),
                &Address::generate(&env),
                &BytesN::from_array(&env, &[5u8; 32]),
                &u64::MAX,
                &BytesN::from_array(&env, &[6u8; 32]),
            );

            patient_client.register_patient(
                &patient,
                &String::from_str(&env, "Bob Patient"),
                &631152000u64,
                &encrypted_ref(&env, 5),
                &policy(&env),
            );

            let result: Result<u64, Error> = env.invoke_contract(
                &hr_contract_id,
                &Symbol::new(&env, "create_record"),
                vec![
                    &env,
                    patient.clone().into_val(&env),
                    provider.clone().into_val(&env),
                    encrypted_ref(&env, 10).into_val(&env),
                    String::from_str(&env, "LAB").into_val(&env),
                    policy(&env).into_val(&env),
                ],
            );

            assert_eq!(result, Err(Error::ConsentNotGranted));
        }

        #[test]
        fn test_consent_grant_access_revoke_denies_provider() {
            let env = Env::default();
            env.mock_all_auths();
            let (client, patient, provider) = setup(&env);

            client.grant_consent(&patient, &provider);
            let record_id = client
                .create_record(
                    &patient,
                    &provider,
                    &encrypted_ref(&env, 11),
                    &String::from_str(&env, "XRAY"),
                    &policy(&env),
                )
                .unwrap();

            let record = client.get_record(&provider, &record_id);
            assert_eq!(record.record_id, record_id);

            client.revoke_consent(&patient, &provider);
            let result = client.try_get_record(&provider, &record_id);
            assert_eq!(result, Err(Ok(Error::Unauthorized)));
        }
    }

// ── Record versioning tests (#471) ───────────────────────────────────────────

#[cfg(test)]
mod record_versioning_tests {
    use crate::{Error, HealthRecords, HealthRecordsClient};
    use shared::privacy::{EncryptedEnvelopeRef, PolicyMetadata};
    use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol};

    fn encrypted_ref(env: &Env, seed: u8) -> EncryptedEnvelopeRef {
        EncryptedEnvelopeRef {
            content_hash: BytesN::from_array(env, &[seed; 32]),
            envelope_uri: String::from_str(env, "enc+ipfs://bafyvalidhealthref"),
            key_version_id: String::from_str(env, "kv:v01"),
        }
    }

    fn policy(env: &Env) -> PolicyMetadata {
        PolicyMetadata {
            retention_class: Symbol::new(env, "clinical"),
            access_policy_hash: BytesN::from_array(env, &[7u8; 32]),
            purpose: Symbol::new(env, "treatment"),
        }
    }

    fn setup(env: &Env) -> (HealthRecordsClient<'static>, Address, Address) {
        let contract_id = env.register(HealthRecords, ());
        let client = HealthRecordsClient::new(env, &contract_id);
        let patient = Address::generate(env);
        let provider = Address::generate(env);
        (client, patient, provider)
    }

    /// New records start at version 1.
    #[test]
    fn test_create_record_starts_at_version_one() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let record_id = client.create_record(
            &patient,
            &provider,
            &encrypted_ref(&env, 1),
            &String::from_str(&env, "LAB"),
            &policy(&env),
        );

        let record = client.get_record(&patient, &record_id);
        assert_eq!(record.version, 1);
    }

    /// Version counter increments on each update.
    #[test]
    fn test_update_record_increments_version() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let record_id = client.create_record(
            &patient,
            &provider,
            &encrypted_ref(&env, 1),
            &String::from_str(&env, "DIAGNOSIS"),
            &policy(&env),
        );

        let new_version = client.update_record(
            &patient,
            &record_id,
            &encrypted_ref(&env, 2),
            &String::from_str(&env, "DIAGNOSIS_UPDATED"),
            &policy(&env),
        );
        assert_eq!(new_version, 2);

        let record = client.get_record(&patient, &record_id);
        assert_eq!(record.version, 2);
    }

    /// All prior versions remain readable via get_record_version.
    #[test]
    fn test_prior_versions_remain_readable() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let record_id = client.create_record(
            &patient,
            &provider,
            &encrypted_ref(&env, 1),
            &String::from_str(&env, "PRESCRIPTION"),
            &policy(&env),
        );

        // Update twice.
        client.update_record(
            &patient,
            &record_id,
            &encrypted_ref(&env, 2),
            &String::from_str(&env, "PRESCRIPTION_V2"),
            &policy(&env),
        );
        client.update_record(
            &patient,
            &record_id,
            &encrypted_ref(&env, 3),
            &String::from_str(&env, "PRESCRIPTION_V3"),
            &policy(&env),
        );

        // Current version (v3) is accessible via get_record.
        let current = client.get_record(&patient, &record_id);
        assert_eq!(current.version, 3);
        assert_eq!(current.record_type, String::from_str(&env, "PRESCRIPTION_V3"));

        // v1 remains readable.
        let v1 = client.get_record_version(&patient, &record_id, &1u32);
        assert_eq!(v1.version, 1);
        assert_eq!(v1.record_type, String::from_str(&env, "PRESCRIPTION"));

        // v2 remains readable.
        let v2 = client.get_record_version(&patient, &record_id, &2u32);
        assert_eq!(v2.version, 2);
        assert_eq!(v2.record_type, String::from_str(&env, "PRESCRIPTION_V2"));

        // Current version accessible via get_record_version too.
        let v3 = client.get_record_version(&patient, &record_id, &3u32);
        assert_eq!(v3.version, 3);
    }

    /// Retrieving a non-existent version returns VersionNotFound.
    #[test]
    fn test_get_nonexistent_version_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let record_id = client.create_record(
            &patient,
            &provider,
            &encrypted_ref(&env, 1),
            &String::from_str(&env, "XRAY"),
            &policy(&env),
        );

        // Version 2 does not exist yet.
        let result = client.try_get_record_version(&patient, &record_id, &2u32);
        assert_eq!(result, Err(Ok(Error::VersionNotFound)));
    }

    /// A consented provider can update a record and read prior versions.
    #[test]
    fn test_provider_can_update_and_read_versions() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let record_id = client.create_record(
            &patient,
            &provider,
            &encrypted_ref(&env, 4),
            &String::from_str(&env, "BLOOD_WORK"),
            &policy(&env),
        );

        client.update_record(
            &provider,
            &record_id,
            &encrypted_ref(&env, 5),
            &String::from_str(&env, "BLOOD_WORK_V2"),
            &policy(&env),
        );

        let v1 = client.get_record_version(&provider, &record_id, &1u32);
        assert_eq!(v1.record_type, String::from_str(&env, "BLOOD_WORK"));

        let current = client.get_record(&provider, &record_id);
        assert_eq!(current.version, 2);
    }

    /// An unauthorized caller cannot update a record.
    #[test]
    fn test_unauthorized_update_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);
        let stranger = Address::generate(&env);

        client.grant_consent(&patient, &provider);
        let record_id = client.create_record(
            &patient,
            &provider,
            &encrypted_ref(&env, 6),
            &String::from_str(&env, "MRI"),
            &policy(&env),
        );

        let result = client.try_update_record(
            &stranger,
            &record_id,
            &encrypted_ref(&env, 7),
            &String::from_str(&env, "MRI_V2"),
            &policy(&env),
        );
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    }

    /// Multi-version diff scenario: encrypted_ref changes track between versions.
    #[test]
    fn test_multi_version_diff_encrypted_ref() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        client.grant_consent(&patient, &provider);
        let ref_v1 = encrypted_ref(&env, 10);
        let ref_v2 = encrypted_ref(&env, 20);
        let ref_v3 = encrypted_ref(&env, 30);

        let record_id = client.create_record(
            &patient,
            &provider,
            &ref_v1.clone(),
            &String::from_str(&env, "CT_SCAN"),
            &policy(&env),
        );

        client.update_record(
            &patient,
            &record_id,
            &ref_v2.clone(),
            &String::from_str(&env, "CT_SCAN"),
            &policy(&env),
        );
        client.update_record(
            &patient,
            &record_id,
            &ref_v3.clone(),
            &String::from_str(&env, "CT_SCAN"),
            &policy(&env),
        );

        let v1 = client.get_record_version(&patient, &record_id, &1u32);
        let v2 = client.get_record_version(&patient, &record_id, &2u32);
        let v3 = client.get_record_version(&patient, &record_id, &3u32);

        assert_eq!(v1.encrypted_ref.content_hash, ref_v1.content_hash);
        assert_eq!(v2.encrypted_ref.content_hash, ref_v2.content_hash);
        assert_eq!(v3.encrypted_ref.content_hash, ref_v3.content_hash);
    }
}
