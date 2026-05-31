#[cfg(test)]
mod tests {
    use crate::{Error, HealthRecords, HealthRecordsClient};
    use shared::privacy::{EncryptedEnvelopeRef, PolicyMetadata};
    use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, String, Symbol};

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
