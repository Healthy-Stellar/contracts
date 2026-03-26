#[cfg(test)]
mod tests {
    use crate::{HealthRecords, HealthRecordsClient};
    use soroban_sdk::{testutils::Address as _, Address, Bytes, Env, String};

    fn setup(env: &Env) -> (HealthRecordsClient<'static>, Address, Address) {
        let contract_id = env.register(HealthRecords, ());
        let client = HealthRecordsClient::new(env, &contract_id);
        let patient = Address::generate(env);
        let provider = Address::generate(env);
        (client, patient, provider)
    }

    #[test]
    fn test_create_record_stores_integrity_hash() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        let cid = String::from_str(&env, "QmTestCID123");
        let rtype = String::from_str(&env, "LAB_RESULT");

        let record_id = client.create_record(&patient, &provider, &cid, &rtype);
        let record = client.get_record(&record_id);

        // integrity_hash must be 32 bytes and non-zero
        assert_eq!(record.integrity_hash.len(), 32);
        let hash_bytes: Bytes = record.integrity_hash.into();
        assert_ne!(hash_bytes, Bytes::new(&env));
    }

    #[test]
    fn test_verify_record_integrity_valid() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        let cid = String::from_str(&env, "QmValidCID");
        let rtype = String::from_str(&env, "PRESCRIPTION");

        let record_id = client.create_record(&patient, &provider, &cid, &rtype);
        let record = client.get_record(&record_id);

        let stored_hash: Bytes = record.integrity_hash.into();
        assert!(client.verify_record_integrity(&record_id, &stored_hash));
    }

    #[test]
    fn test_verify_record_integrity_tampered_returns_false() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        let cid = String::from_str(&env, "QmOriginalCID");
        let rtype = String::from_str(&env, "DIAGNOSIS");

        let record_id = client.create_record(&patient, &provider, &cid, &rtype);

        // Provide a tampered (all-zeros) 32-byte hash
        let tampered_hash = Bytes::from_array(&env, &[0u8; 32]);
        assert!(!client.verify_record_integrity(&record_id, &tampered_hash));
    }

    #[test]
    fn test_verify_nonexistent_record_returns_false() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _, _) = setup(&env);

        let hash = Bytes::from_array(&env, &[0u8; 32]);
        assert!(!client.verify_record_integrity(&999u64, &hash));
    }

    #[test]
    fn test_verify_wrong_length_hash_returns_false() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, patient, provider) = setup(&env);

        let cid = String::from_str(&env, "QmCID");
        let rtype = String::from_str(&env, "XRAY");

        let record_id = client.create_record(&patient, &provider, &cid, &rtype);

        // Only 16 bytes — wrong length
        let short_hash = Bytes::from_array(&env, &[0u8; 16]);
        assert!(!client.verify_record_integrity(&record_id, &short_hash));
    }
}
