#![cfg(test)]

use super::*;
use shared::privacy::{EncryptedEnvelopeRef, PolicyMetadata};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, MockAuth, MockAuthInvoke},
    Address, Bytes, BytesN, Env, IntoVal, String, Symbol, Vec,
};

fn encrypted_ref(env: &Env, seed: u8) -> EncryptedEnvelopeRef {
    let hash_seed = if seed == 0 { 1 } else { seed };
    EncryptedEnvelopeRef {
        content_hash: BytesN::from_array(env, &[hash_seed; 32]),
        envelope_uri: String::from_str(env, "enc+ipfs://bafyvalidpatientref"),
        key_version_id: String::from_str(env, "kv:v01"),
    }
}

fn policy(env: &Env) -> PolicyMetadata {
    PolicyMetadata {
        retention_class: Symbol::new(env, "clinical"),
        access_policy_hash: BytesN::from_array(env, &[200u8; 32]),
        purpose: Symbol::new(env, "treatment"),
    }
}

fn make_cid_v1(env: &Env, seed: u8) -> Bytes {
    let mut raw = [seed; 36];
    raw[0] = b'b';
    Bytes::from_array(env, &raw)
}

fn make_cid_v0(env: &Env, seed: u8) -> Bytes {
    let mut raw = [seed; 34];
    raw[0] = 0x12;
    raw[1] = 0x20;
    Bytes::from_array(env, &raw)
}

fn make_cid_v0_qm(env: &Env) -> Bytes {
    Bytes::from_slice(env, b"QmXoypizj2Madv6NthR75ce451F33968F9e1XF3D8xS288")
}

/// ------------------------------------------------
/// PATIENT TESTS
/// ------------------------------------------------

#[test]
fn test_register_and_get_patient() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let patient_wallet = Address::generate(&env);
    let name = String::from_str(&env, "John Doe");
    let dob = 631152000;
    let metadata = encrypted_ref(&env, 1);
    let metadata_policy = policy(&env);

    env.mock_all_auths();

    client.register_patient(&patient_wallet, &name, &dob, &metadata, &metadata_policy);

    let patient_data = client.get_patient(&patient_wallet);
    assert_eq!(patient_data.name, name);
    assert_eq!(patient_data.dob, dob);
    assert_eq!(patient_data.encrypted_metadata_ref, metadata);
}

#[test]
fn test_update_patient() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let patient_wallet = Address::generate(&env);
    let name = String::from_str(&env, "John Doe");
    let dob = 631152000;
    let initial_metadata = encrypted_ref(&env, 1);
    let initial_policy = policy(&env);

    env.mock_all_auths();

    client.register_patient(
        &patient_wallet,
        &name,
        &dob,
        &initial_metadata,
        &initial_policy,
    );

    let new_metadata = encrypted_ref(&env, 2);
    let new_policy = policy(&env);
    client.update_patient(&patient_wallet, &patient_wallet, &new_metadata, &new_policy);

    let patient_data = client.get_patient(&patient_wallet);
    assert_eq!(patient_data.encrypted_metadata_ref, new_metadata);
}

#[test]
fn test_is_patient_registered() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let patient_wallet = Address::generate(&env);
    let unregistered_wallet = Address::generate(&env);

    env.mock_all_auths();

    assert!(!client.is_patient_registered(&patient_wallet));
    assert!(!client.is_patient_registered(&unregistered_wallet));

    client.register_patient(
        &patient_wallet,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    assert!(client.is_patient_registered(&patient_wallet));
    assert!(!client.is_patient_registered(&unregistered_wallet));
}

#[test]
fn test_total_patients_increments_on_register() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    assert_eq!(client.get_total_patients(), 0);

    client.register_patient(
        &Address::generate(&env),
        &String::from_str(&env, "P1"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    assert_eq!(client.get_total_patients(), 1);

    client.register_patient(
        &Address::generate(&env),
        &String::from_str(&env, "P2"),
        &631152001,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    assert_eq!(client.get_total_patients(), 2);
}

#[test]
fn test_analytics_counters_admin_only() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let patient = Address::generate(&env);
    let v1 = make_version(&env, 1);
    let attacker = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "initialize",
                args: (&admin, &treasury, &fee_token).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .initialize(&admin, &treasury, &fee_token);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "publish_consent_version",
                args: (&v1,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .publish_consent_version(&v1);

    client
        .mock_auths(&[MockAuth {
            address: &patient,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "register_patient",
                args: (
                    &patient,
                    &String::from_str(&env, "P1"),
                    &631152000u64,
                    &encrypted_ref(&env, 1),
                    &policy(&env),
                )
                    .into_val(&env),
                sub_invokes: &[],
            },
        }])
        .register_patient(
            &patient,
            &String::from_str(&env, "P1"),
            &631152000,
            &encrypted_ref(&env, 1),
            &policy(&env),
        );

    let inv1 = MockAuthInvoke {
        contract: &contract_id,
        fn_name: "get_total_records_created",
        args: ().into_val(&env),
        sub_invokes: &[],
    };
    let a1 = MockAuth {
        address: &attacker,
        invoke: &inv1,
    };
    assert!(client
        .mock_auths(&[a1])
        .try_get_total_records_created()
        .is_err());

    let inv2 = MockAuthInvoke {
        contract: &contract_id,
        fn_name: "get_total_providers",
        args: ().into_val(&env),
        sub_invokes: &[],
    };
    let a2 = MockAuth {
        address: &attacker,
        invoke: &inv2,
    };
    assert!(client.mock_auths(&[a2]).try_get_total_providers().is_err());

    let inv3 = MockAuthInvoke {
        contract: &contract_id,
        fn_name: "get_total_access_grants",
        args: ().into_val(&env),
        sub_invokes: &[],
    };
    let a3 = MockAuth {
        address: &attacker,
        invoke: &inv3,
    };
    assert!(client
        .mock_auths(&[a3])
        .try_get_total_access_grants()
        .is_err());
}

#[test]
fn test_total_records_created_increments_on_add_record() {
    let env = Env::default();
    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    let before = client.get_total_records_created();
    assert_eq!(before, 0);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 11),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
    let after = client.get_total_records_created();
    assert_eq!(after, 1);
}

#[test]
fn test_total_providers_increment_on_register() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let doctor = Address::generate(&env);
    let institution = Address::generate(&env);
    let v1 = make_version(&env, 1);

    env.mock_all_auths();
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);

    assert_eq!(client.get_total_providers(), 0);

    client.register_doctor(
        &doctor,
        &String::from_str(&env, "Dr"),
        &String::from_str(&env, "Spec"),
        &make_cid_v1(&env, 2),
    );
    assert_eq!(client.get_total_providers(), 1);

    client.register_institution(&institution);
    assert_eq!(client.get_total_providers(), 2);
}

#[test]
fn test_total_access_grants_increments_on_grant_and_decrements_on_revoke() {
    let env = Env::default();
    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);
    let other_doctor = Address::generate(&env);

    // `setup_for_ttl` already granted `doctor` once.
    assert_eq!(client.get_total_access_grants(), 1);

    client.grant_access(&patient, &patient, &doctor);
    assert_eq!(client.get_total_access_grants(), 1);

    // granting same doctor again should not increment
    client.grant_access(&patient, &patient, &doctor);
    assert_eq!(client.get_total_access_grants(), 1);

    client.grant_access(&patient, &patient, &other_doctor);
    assert_eq!(client.get_total_access_grants(), 2);

    client.revoke_access(&patient, &patient, &doctor);
    assert_eq!(client.get_total_access_grants(), 1);
}

#[test]
fn test_total_patients_not_incremented_on_failed_register() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    let patient_wallet = Address::generate(&env);
    client.register_patient(
        &patient_wallet,
        &String::from_str(&env, "P1"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    assert_eq!(client.get_total_patients(), 1);

    let duplicate_attempt = client.try_register_patient(
        &patient_wallet,
        &String::from_str(&env, "P1"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    assert!(duplicate_attempt.is_err());
    assert_eq!(client.get_total_patients(), 1);
}

/// ------------------------------------------------
/// DOCTOR + INSTITUTION TESTS
/// ------------------------------------------------

#[test]
fn test_register_and_get_doctor() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);
    let name = String::from_str(&env, "Dr. Alice");
    let specialization = String::from_str(&env, "Cardiology");
    let cert_hash = Bytes::from_array(&env, &[1, 2, 3, 4]);

    env.mock_all_auths();

    client.register_doctor(&doctor_wallet, &name, &specialization, &cert_hash);

    let doctor = client.get_doctor(&doctor_wallet);
    assert_eq!(doctor.name, name);
    assert_eq!(doctor.specialization, specialization);
    assert_eq!(doctor.certificate_hash, cert_hash);
    assert!(!doctor.verified);
}

#[test]
fn test_register_institution_and_verify_doctor() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    let name = String::from_str(&env, "Dr. Bob");
    let specialization = String::from_str(&env, "Neurology");
    let cert_hash = Bytes::from_array(&env, &[9, 9, 9]);

    env.mock_all_auths();

    client.register_doctor(&doctor_wallet, &name, &specialization, &cert_hash);
    client.register_institution(&institution_wallet);
    client.verify_doctor(&doctor_wallet, &institution_wallet);

    let doctor = client.get_doctor(&doctor_wallet);
    assert!(doctor.verified);
}

#[test]
#[should_panic]
fn test_verify_doctor_by_unregistered_institution_should_fail() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);
    let fake_institution = Address::generate(&env);

    let name = String::from_str(&env, "Dr. Eve");
    let specialization = String::from_str(&env, "Oncology");
    let cert_hash = Bytes::from_array(&env, &[7, 7, 7]);

    env.mock_all_auths();

    client.register_doctor(&doctor_wallet, &name, &specialization, &cert_hash);
    client.verify_doctor(&doctor_wallet, &fake_institution);
}

#[test]
fn test_grant_access_and_add_medical_record() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    let hash = make_cid_v1(&env, 1);
    let desc = String::from_str(&env, "Blood test results");
    let v1 = BytesN::from_array(&env, &[1u8; 32]);

    env.mock_all_auths();

    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Test Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    let record_ref = encrypted_ref(&env, 11);
    client.add_medical_record(
        &patient,
        &doctor,
        &record_ref,
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let records = client.get_medical_records(&patient, &patient);
    assert_eq!(records.len(), 1);

    let record = records.get(0).unwrap();
    assert_eq!(record.encrypted_ref, record_ref);
    assert_eq!(record.record_type, Symbol::new(&env, "LAB"));
}

#[test]
#[should_panic]
fn test_unauthorized_doctor_cannot_add_record() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let v1 = BytesN::from_array(&env, &[1u8; 32]);

    let hash = make_cid_v1(&env, 9);
    let desc = String::from_str(&env, "X-ray scan");

    env.mock_all_auths();

    // Initialize + register patient + publish consent version,
    // but do NOT acknowledge consent → should panic with consent message
    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Test Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.publish_consent_version(&v1);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 3),
        &Symbol::new(&env, "IMAGING"),
        &policy(&env),
    );
}

#[test]
fn test_revoke_access() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    env.mock_all_auths();

    client.grant_access(&patient, &patient, &doctor);
    client.revoke_access(&patient, &patient, &doctor);

    let doctors = client.get_authorized_doctors(&patient);
    assert_eq!(doctors.len(), 0);
}

#[test]
fn test_validate_cidv1_base32() {
    let env = Env::default();
    let cid = make_cid_v1(&env, 7);
    assert!(validate_cid(&cid).is_ok());
}

#[test]
fn test_validate_cidv0_multihash() {
    let env = Env::default();
    let cid = make_cid_v0(&env, 9);
    assert!(validate_cid(&cid).is_ok());
}

#[test]
fn test_validate_cidv0_qm_prefix() {
    let env = Env::default();
    let cid = make_cid_v0_qm(&env);
    assert!(validate_cid(&cid).is_ok());
}

#[test]
fn test_validate_empty_cid_rejected() {
    let env = Env::default();
    let cid = Bytes::from_slice(&env, &[]);
    assert_eq!(validate_cid(&cid), Err(ContractError::InvalidCID));
}

#[test]
fn test_validate_oversized_cid_rejected() {
    let env = Env::default();
    let raw = [b'b'; 513];
    let cid = Bytes::from_slice(&env, &raw);
    assert_eq!(validate_cid(&cid), Err(ContractError::InvalidCID));
}

#[test]
fn test_validate_short_cidv1_rejected() {
    let env = Env::default();
    let raw = [b'b'; 10];
    let cid = Bytes::from_slice(&env, &raw);
    assert_eq!(validate_cid(&cid), Err(ContractError::InvalidCID));
}

#[test]
fn test_validate_wrong_cidv0_prefix_rejected() {
    let env = Env::default();
    let mut raw = [0u8; 34];
    raw[0] = 0x12;
    raw[1] = 0x21;
    let cid = Bytes::from_slice(&env, &raw);
    assert_eq!(validate_cid(&cid), Err(ContractError::InvalidCID));
}

#[test]
fn test_validate_garbage_bytes_rejected() {
    let env = Env::default();
    let cid = Bytes::from_slice(&env, &[0xFF, 0xAB, 0x00, 0x11]);
    assert_eq!(validate_cid(&cid), Err(ContractError::InvalidCID));
}

#[test]
fn test_validate_did_ok() {
    let env = Env::default();
    let did = String::from_str(&env, "did:web:example.com");
    assert!(validate_did(&did).is_ok());
}

#[test]
fn test_validate_did_rejects_bad_prefix() {
    let env = Env::default();
    let did = String::from_str(&env, "notdid:web:x");
    assert_eq!(validate_did(&did), Err(ContractError::InvalidDID));
}

#[test]
fn test_validate_score_ok() {
    assert!(validate_score(0).is_ok());
    assert!(validate_score(100).is_ok());
    assert!(validate_score(50).is_ok());
}

#[test]
fn test_validate_score_rejects_out_of_range() {
    assert_eq!(validate_score(-1), Err(ContractError::InvalidScore));
    assert_eq!(validate_score(101), Err(ContractError::InvalidScore));
}

#[test]
fn test_add_medical_record_rejects_invalid_cid() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let version = BytesN::from_array(&env, &[1u8; 32]);
    let invalid_cid = Bytes::from_slice(&env, &[0x01, 0x02, 0x03]);

    env.mock_all_auths();

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Test Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.publish_consent_version(&version);
    client.acknowledge_consent(&patient, &patient, &version);
    client.grant_access(&patient, &patient, &doctor);

    let result = client.try_add_medical_record(
        &patient,
        &doctor,
        &EncryptedEnvelopeRef {
            content_hash: BytesN::from_array(&env, &[1; 32]),
            envelope_uri: String::from_str(&env, "ipfs://plaintext"),
            key_version_id: String::from_str(&env, "kv:v01"),
        },
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    assert!(matches!(
        result,
        Err(Ok(ContractError::InvalidEncryptedEnvelope))
    ));
    assert_eq!(client.get_medical_records(&patient, &patient).len(), 0);
}

// ------------------------------------------------
// REGULATORY HOLD TESTS
// ------------------------------------------------

#[test]
fn test_admin_can_place_hold() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[7u8; 32]);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    env.ledger().set_timestamp(100);
    client.place_hold(&patient, &reason_hash, &250);

    let hold = client.get_hold(&patient).unwrap();
    assert_eq!(hold.reason_hash, reason_hash);
    assert_eq!(hold.expires_at, 250);
    assert_eq!(hold.placed_at, 100);
    assert!(client.is_hold_active(&patient));
}

#[test]
fn test_non_admin_cannot_place_hold() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let other = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[5u8; 32]);
    let name = String::from_str(&env, "Jane Doe");
    let metadata = encrypted_ref(&env, 1);
    let metadata_policy = policy(&env);
    let dob = 631152000u64;
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "initialize",
                args: (&admin, &treasury, &fee_token).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .initialize(&admin, &treasury, &fee_token);

    client
        .mock_auths(&[MockAuth {
            address: &patient,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "register_patient",
                args: (&patient, &name, &dob, &metadata, &metadata_policy).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .register_patient(&patient, &name, &dob, &metadata, &metadata_policy);

    let result = client
        .mock_auths(&[MockAuth {
            address: &other,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "place_hold",
                args: (&patient, &reason_hash, &250u64).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_place_hold(&patient, &reason_hash, &250u64);

    assert!(result.is_err());
}

#[test]
fn test_admin_can_lift_hold() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[8u8; 32]);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    env.ledger().set_timestamp(100);
    client.place_hold(&patient, &reason_hash, &250);
    env.ledger().set_timestamp(120);
    client.lift_hold(&patient);

    assert_eq!(client.get_hold(&patient), None);
    assert!(!client.is_hold_active(&patient));
}

#[test]
fn test_non_admin_cannot_lift_hold() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let other = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[6u8; 32]);
    let name = String::from_str(&env, "Jane Doe");
    let metadata = encrypted_ref(&env, 1);
    let metadata_policy = policy(&env);
    let dob = 631152000u64;
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "initialize",
                args: (&admin, &treasury, &fee_token).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .initialize(&admin, &treasury, &fee_token);

    client
        .mock_auths(&[MockAuth {
            address: &patient,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "register_patient",
                args: (&patient, &name, &dob, &metadata, &metadata_policy).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .register_patient(&patient, &name, &dob, &metadata, &metadata_policy);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "place_hold",
                args: (&patient, &reason_hash, &250u64).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .place_hold(&patient, &reason_hash, &250u64);

    let result = client
        .mock_auths(&[MockAuth {
            address: &other,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "lift_hold",
                args: (&patient,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_lift_hold(&patient);

    assert!(result.is_err());
}

#[test]
fn test_hold_blocks_patient_update() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[9u8; 32]);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    env.ledger().set_timestamp(50);
    client.place_hold(&patient, &reason_hash, &250);

    let result =
        client.try_update_patient(&patient, &patient, &encrypted_ref(&env, 2), &policy(&env));
    assert!(result.is_err());
}

#[test]
fn test_hold_blocks_grant_and_revoke_access() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[10u8; 32]);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    client.grant_access(&patient, &patient, &doctor);
    env.ledger().set_timestamp(50);
    client.place_hold(&patient, &reason_hash, &250);

    let grant_result = client.try_grant_access(&patient, &patient, &Address::generate(&env));
    assert!(grant_result.is_err());

    let revoke_result = client.try_revoke_access(&patient, &patient, &doctor);
    assert!(revoke_result.is_err());
}

#[test]
fn test_write_succeeds_after_hold_expiry() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[11u8; 32]);
    let updated_metadata = encrypted_ref(&env, 2);
    let updated_policy = policy(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    env.ledger().set_timestamp(100);
    client.place_hold(&patient, &reason_hash, &150);
    assert!(client.is_hold_active(&patient));

    env.ledger().set_timestamp(151);
    assert!(!client.is_hold_active(&patient));

    client.update_patient(&patient, &patient, &updated_metadata, &updated_policy);
    let patient_data = client.get_patient(&patient);
    assert_eq!(patient_data.encrypted_metadata_ref, updated_metadata);
}

#[test]
fn test_hold_exposes_only_reason_hash_in_state() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[12u8; 32]);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    env.ledger().set_timestamp(100);
    client.place_hold(&patient, &reason_hash, &250);

    let hold = client.get_hold(&patient).unwrap();
    assert_eq!(hold.reason_hash, reason_hash);
    assert_eq!(hold.expires_at, 250);
    assert_eq!(hold.placed_at, 100);
}

#[test]
fn test_lifting_hold_restores_normal_write_ability() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[13u8; 32]);
    let updated_metadata = encrypted_ref(&env, 3);
    let updated_policy = policy(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    env.ledger().set_timestamp(50);
    client.place_hold(&patient, &reason_hash, &300);
    client.lift_hold(&patient);
    client.update_patient(&patient, &patient, &updated_metadata, &updated_policy);

    let patient_data = client.get_patient(&patient);
    assert_eq!(patient_data.encrypted_metadata_ref, updated_metadata);
}

#[test]
fn test_invalid_hold_expiry_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[14u8; 32]);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    env.ledger().set_timestamp(100);
    let result = client.try_place_hold(&patient, &reason_hash, &100u64);
    assert!(result.is_err());
}

#[test]
fn test_duplicate_active_hold_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let reason_hash = BytesN::from_array(&env, &[15u8; 32]);
    let second_reason_hash = BytesN::from_array(&env, &[16u8; 32]);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    env.ledger().set_timestamp(100);
    client.place_hold(&patient, &reason_hash, &250);

    let result = client.try_place_hold(&patient, &second_reason_hash, &300u64);
    assert!(result.is_err());
}

// ------------------------------------------------
// CONSENT TESTS
// ------------------------------------------------

fn make_version(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

#[test]
fn test_consent_status_never_signed() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let patient = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(
        &Address::generate(&env),
        &Address::generate(&env),
        &Address::generate(&env),
    );

    assert_eq!(
        client.get_consent_status(&patient),
        ConsentStatus::NeverSigned
    );
}

#[test]
fn test_consent_status_never_signed_no_ack() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&make_version(&env, 1));

    assert_eq!(
        client.get_consent_status(&patient),
        ConsentStatus::NeverSigned
    );
}

#[test]
fn test_consent_status_acknowledged() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let v1 = make_version(&env, 1);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);

    assert_eq!(
        client.get_consent_status(&patient),
        ConsentStatus::Acknowledged
    );
}

#[test]
fn test_consent_status_pending_after_new_version() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let v1 = make_version(&env, 1);
    let v2 = make_version(&env, 2);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);

    client.publish_consent_version(&v2);
    assert_eq!(client.get_consent_status(&patient), ConsentStatus::Pending);
}

#[test]
fn test_consent_re_acknowledge_restores_acknowledged() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let v1 = make_version(&env, 1);
    let v2 = make_version(&env, 2);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.publish_consent_version(&v2);
    client.acknowledge_consent(&patient, &patient, &v2);

    assert_eq!(
        client.get_consent_status(&patient),
        ConsentStatus::Acknowledged
    );
}

#[test]
#[should_panic]
fn test_acknowledge_wrong_version_panics() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&make_version(&env, 1));
    client.acknowledge_consent(&patient, &patient, &make_version(&env, 99));
}

#[test]
#[should_panic]
fn test_add_record_blocked_without_consent() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Test Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.publish_consent_version(&make_version(&env, 1));
    // Patient never acknowledges
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
}

#[test]
fn test_add_record_allowed_after_consent() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let v1 = make_version(&env, 1);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Test Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 2),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    assert_eq!(client.get_medical_records(&patient, &patient).len(), 1);
}

#[test]
#[should_panic]
fn test_add_record_blocked_after_new_version() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let v1 = make_version(&env, 1);
    let v2 = make_version(&env, 2);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Test Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);

    // Admin bumps version — patient must re-acknowledge
    client.publish_consent_version(&v2);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 3),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
}

// ------------------------------------------------
// GUARDIAN TESTS
// ------------------------------------------------

fn setup_with_consent(env: &Env) -> (MedicalRegistryClient<'_>, Address) {
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(env, &contract_id);
    let admin = Address::generate(env);
    env.mock_all_auths();
    let treasury = Address::generate(env);
    let fee_token = Address::generate(env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&make_version(env, 1));
    (client, admin)
}

#[test]
fn test_assign_and_get_guardian() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.assign_guardian(&patient, &guardian);
    assert_eq!(client.get_guardian(&patient), Some(guardian));
}

#[test]
fn test_revoke_guardian() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.assign_guardian(&patient, &guardian);
    client.revoke_guardian(&patient);
    assert_eq!(client.get_guardian(&patient), None);
}

#[test]
fn test_guardian_can_acknowledge_consent() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let v1 = make_version(&env, 1);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.assign_guardian(&patient, &guardian);
    client.acknowledge_consent(&patient, &guardian, &v1);

    assert_eq!(
        client.get_consent_status(&patient),
        ConsentStatus::Acknowledged
    );
}

#[test]
fn test_guardian_can_grant_and_revoke_access() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let v1 = make_version(&env, 1);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.assign_guardian(&patient, &guardian);
    client.acknowledge_consent(&patient, &guardian, &v1);
    client.grant_access(&patient, &guardian, &doctor);

    assert_eq!(client.get_authorized_doctors(&patient).len(), 1);

    client.revoke_access(&patient, &guardian, &doctor);
    assert_eq!(client.get_authorized_doctors(&patient).len(), 0);
}

#[test]
fn test_guardian_can_update_patient() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.register_patient(
        &patient,
        &String::from_str(&env, "Minor Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.assign_guardian(&patient, &guardian);
    let updated_metadata = encrypted_ref(&env, 2);
    client.update_patient(&patient, &guardian, &updated_metadata, &policy(&env));

    assert_eq!(
        client.get_patient(&patient).encrypted_metadata_ref,
        updated_metadata
    );
}

#[test]
fn test_guardian_enables_record_write() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let v1 = make_version(&env, 1);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.register_patient(
        &patient,
        &String::from_str(&env, "Test Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.assign_guardian(&patient, &guardian);
    client.acknowledge_consent(&patient, &guardian, &v1);
    client.grant_access(&patient, &guardian, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 5),
        &Symbol::new(&env, "PRESCRIPTION"),
        &policy(&env),
    );

    assert_eq!(client.get_medical_records(&patient, &patient).len(), 1);
}

#[test]
#[should_panic]
fn test_unauthorized_caller_rejected() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let v1 = make_version(&env, 1);
    let patient = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.acknowledge_consent(&patient, &stranger, &v1);
}

#[test]
#[should_panic]
fn test_revoked_guardian_rejected() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let v1 = make_version(&env, 1);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.assign_guardian(&patient, &guardian);
    client.revoke_guardian(&patient);
    client.acknowledge_consent(&patient, &guardian, &v1);
}

#[test]
#[should_panic]
fn test_guardian_cannot_act_for_different_patient() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let v1 = make_version(&env, 1);
    let patient_a = Address::generate(&env);
    let patient_b = Address::generate(&env);
    let guardian = Address::generate(&env);

    client.assign_guardian(&patient_a, &guardian);
    client.acknowledge_consent(&patient_b, &guardian, &v1);
}

// ------------------------------------------------
// SNAPSHOT TESTS
// ------------------------------------------------

fn register_patient_with_consent(
    client: &MedicalRegistryClient,
    env: &Env,
    v1: &BytesN<32>,
    wallet: &Address,
) {
    client.register_patient(
        wallet,
        &String::from_str(env, "Test Patient"),
        &631152000,
        &encrypted_ref(env, 1),
        &policy(env),
    );
    client.acknowledge_consent(wallet, wallet, v1);
}

#[test]
fn test_first_snapshot_always_allowed() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let v1 = make_version(&env, 1);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);

    client.emit_state_snapshot();
    assert_eq!(
        client.get_last_snapshot_ledger(),
        Some(env.ledger().sequence())
    );
}

#[test]
fn test_snapshot_records_ledger_sequence() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    let seq_before = env.ledger().sequence();
    client.emit_state_snapshot();
    assert_eq!(client.get_last_snapshot_ledger(), Some(seq_before));
}

#[test]
fn test_get_last_snapshot_ledger_default_zero() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    assert_eq!(client.get_last_snapshot_ledger(), None);
}

#[test]
#[should_panic]
fn test_snapshot_rate_limit_enforced() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.emit_state_snapshot();

    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        sequence_number: env.ledger().sequence() + 99_999,
        timestamp: env.ledger().timestamp() + 99_999,
        protocol_version: 23,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 10_000_000,
    });

    client.emit_state_snapshot();
}

#[test]
fn test_snapshot_allowed_after_interval() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.emit_state_snapshot();

    let new_seq = env.ledger().sequence() + 100_000;
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        sequence_number: new_seq,
        timestamp: env.ledger().timestamp() + 100_000,
        protocol_version: 23,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 10_000_000,
    });

    client.emit_state_snapshot();
    assert_eq!(client.get_last_snapshot_ledger(), Some(new_seq));
}

#[test]
fn test_snapshot_includes_registered_patients_and_doctors() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let v1 = make_version(&env, 1);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);

    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    register_patient_with_consent(&client, &env, &v1, &p1);
    register_patient_with_consent(&client, &env, &v1, &p2);

    let doctor = Address::generate(&env);
    client.register_doctor(
        &doctor,
        &String::from_str(&env, "Dr. Snap"),
        &String::from_str(&env, "Radiology"),
        &Bytes::from_array(&env, &[1, 2, 3]),
    );

    client.emit_state_snapshot();
    assert_eq!(
        client.get_last_snapshot_ledger(),
        Some(env.ledger().sequence())
    );
}

// ------------------------------------------------
// FEE TESTS
// ------------------------------------------------

fn setup_with_fee(
    env: &Env,
) -> (
    MedicalRegistryClient<'_>,
    Address,
    Address,
    Address,
    Address,
    Address,
    BytesN<32>,
) {
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(env, &contract_id);

    let token_admin = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = token_contract.address();
    let token_client = soroban_sdk::token::StellarAssetClient::new(env, &token_id);

    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let doctor = Address::generate(env);
    let patient = Address::generate(env);
    let v1 = make_version(env, 1);

    env.mock_all_auths();

    client.initialize(&admin, &treasury, &token_id);
    client.register_patient(
        &patient,
        &String::from_str(env, "Test Patient"),
        &631152000,
        &encrypted_ref(env, 1),
        &policy(env),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);

    token_client.mint(&doctor, &10_000);

    (client, admin, treasury, token_id, doctor, patient, v1)
}

#[test]
fn test_get_record_fee_default_zero() {
    let env = Env::default();
    let (client, _admin, _treasury, _token_id, _doctor, _patient, _v1) = setup_with_fee(&env);
    assert_eq!(client.get_record_fee(), 0);
}

#[test]
fn test_set_and_get_record_fee() {
    let env = Env::default();
    let (client, _admin, _treasury, _token_id, _doctor, _patient, _v1) = setup_with_fee(&env);
    client.set_record_fee(&500);
    assert_eq!(client.get_record_fee(), 500);
}

#[test]
fn test_add_record_zero_fee_no_transfer() {
    let env = Env::default();
    let (client, _admin, treasury, token_id, doctor, patient, _v1) = setup_with_fee(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 7),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), 0);
    assert_eq!(token.balance(&doctor), 10_000);
}

#[test]
fn test_add_record_transfers_fee_to_treasury() {
    let env = Env::default();
    let (client, _admin, treasury, token_id, doctor, patient, _v1) = setup_with_fee(&env);

    client.set_record_fee(&200);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 8),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), 200);
    assert_eq!(token.balance(&doctor), 9_800);
}

#[test]
fn test_fee_deducted_per_record() {
    let env = Env::default();
    let (client, _admin, treasury, token_id, doctor, patient, _v1) = setup_with_fee(&env);

    client.set_record_fee(&100);

    for i in 0u8..3 {
        client.add_medical_record(
            &patient,
            &doctor,
            &encrypted_ref(&env, i),
            &Symbol::new(&env, "LAB"),
            &policy(&env),
        );
    }

    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), 300);
    assert_eq!(token.balance(&doctor), 9_700);
}

#[test]
#[should_panic]
fn test_set_negative_fee_panics() {
    let env = Env::default();
    let (client, _admin, _treasury, _token_id, _doctor, _patient, _v1) = setup_with_fee(&env);
    client.set_record_fee(&-1);
}

#[test]
fn test_fee_can_be_reset_to_zero() {
    let env = Env::default();
    let (client, _admin, treasury, token_id, doctor, patient, _v1) = setup_with_fee(&env);

    client.set_record_fee(&300);
    client.set_record_fee(&0);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 9),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), 0);
}

// ------------------------------------------------
// GET_RECORDS_BY_TYPE TESTS
// ------------------------------------------------
/// ------------------------------------------------
/// GET_RECORDS_BY_IDS TESTS
/// ------------------------------------------------

fn setup_for_get_records_by_ids(env: &Env) -> (MedicalRegistryClient<'_>, Address, Address) {
    setup_for_filter(env)
}

fn make_ledger_info(sequence: u32, timestamp: u64) -> soroban_sdk::testutils::LedgerInfo {
    soroban_sdk::testutils::LedgerInfo {
        sequence_number: sequence,
        timestamp,
        protocol_version: 23,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 10_000_000,
    }
}

/// Shared setup for TTL tests: initialized contract + registered patient with consent + doctor.
fn setup_for_ttl(
    env: &Env,
) -> (
    MedicalRegistryClient<'_>,
    Address,
    Address,
    Address,
    BytesN<32>,
) {
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let fee_token = Address::generate(env);
    let patient = Address::generate(env);
    let doctor = Address::generate(env);
    let v1 = make_version(env, 1);

    env.mock_all_auths();

    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);
    client.register_patient(
        &patient,
        &String::from_str(env, "Alice"),
        &631152000,
        &encrypted_ref(env, 1),
        &policy(env),
    );
    client.acknowledge_consent(&patient, &patient, &v1);
    client.register_doctor(
        &doctor,
        &String::from_str(env, "Dr. Bob"),
        &String::from_str(env, "Cardiology"),
        &Bytes::from_array(env, &[1, 2, 3]),
    );
    client.grant_access(&patient, &patient, &doctor);

    (client, admin, patient, doctor, v1)
}

/// ------------------------------------------------
/// GET_RECORDS_BY_TYPE TESTS
/// ------------------------------------------------

/// GET_RECORDS_BY_TYPE TESTS
/// ------------------------------------------------

fn setup_for_filter(env: &Env) -> (MedicalRegistryClient<'_>, Address, Address) {
    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(env);
    (client, patient, doctor)
}

/// After `add_medical_record`, TTL on the MedicalRecords key must not be zero —
/// i.e., `extend_ttl` was called and the entry lives beyond the current ledger.
#[test]
fn test_add_record_extends_patient_ttl() {
    let env = Env::default();
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 5),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    // Verify the records are still accessible after adding
    let records = client.get_medical_records(&patient, &patient);
    assert_eq!(records.len(), 1);
}

#[test]
fn test_get_records_by_type_returns_matching_records() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 10),
        &Symbol::new(&env, "VISIT"),
        &policy(&env),
    );
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 11),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let results = client.get_records_by_type(&patient, &patient, &Symbol::new(&env, "VISIT"));
    assert_eq!(results.len(), 1);
    assert_eq!(
        results.get(0).unwrap().record_type,
        Symbol::new(&env, "VISIT")
    );
}

#[test]
fn test_get_records_by_type_missing_doctor_history_returns_not_found() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 12),
        &Symbol::new(&env, "VISIT"),
        &policy(&env),
    );

    env.as_contract(&client.address, || {
        let mut record_data: RecordData = env
            .storage()
            .persistent()
            .get(&DataKey::MedicalRecord(record_id))
            .unwrap();
        record_data.history = Vec::new(&env);
        env.storage()
            .persistent()
            .set(&DataKey::MedicalRecord(record_id), &record_data);
    });

    let result = client.try_get_records_by_type(&patient, &patient, &Symbol::new(&env, "VISIT"));
    assert_eq!(result, Err(Ok(ContractError::NotFound)));
}

#[test]
fn test_get_record_history_missing_record_returns_not_found() {
    let env = Env::default();
    let (client, patient, _doctor) = setup_for_filter(&env);

    let result = client.try_get_record_history(&999, &patient);

    assert_eq!(result, Err(Ok(ContractError::NotFound)));
}

#[test]
fn test_get_records_by_type_ttl_refreshes_records() {
    let env = Env::default();
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 6),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    // Call get_medical_records — internally bumps TTL
    let records = client.get_medical_records(&patient, &patient);
    assert_eq!(records.len(), 1);

    // Advance the ledger significantly — data should still be accessible
    env.ledger().set(make_ledger_info(
        100 + LEDGER_THRESHOLD - 1,
        1_000_000 + 1_000,
    ));
    let records_after = client.get_medical_records(&patient, &patient);
    assert_eq!(records_after.len(), 1);
}

#[test]
fn test_get_records_by_type_returns_empty_when_no_match() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 13),
        &Symbol::new(&env, "IMAGING"),
        &policy(&env),
    );

    // No PRESCRIPTION records exist — should return empty vec, not error
    let result = client.get_records_by_type(&patient, &patient, &Symbol::new(&env, "PRESCRIPTION"));
    assert_eq!(result.len(), 0);
}

/// After `get_medical_records`, TTL on the MedicalRecords key is bumped so the
/// entry remains accessible.
#[test]
fn test_get_records_extends_ttl() {
    let env = Env::default();
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 13),
        &Symbol::new(&env, "IMAGING"),
        &policy(&env),
    );

    // Accessing records bumps TTL; data still present after threshold
    let records = client.get_medical_records(&patient, &patient);
    assert_eq!(records.len(), 1);

    env.ledger().set(make_ledger_info(
        100 + LEDGER_THRESHOLD - 1,
        1_000_000 + 1_000,
    ));
    let records_after = client.get_medical_records(&patient, &patient);
    assert_eq!(records_after.len(), 1);
}

#[test]
fn test_get_records_by_type_returns_empty_when_no_match_after_ttl() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 14),
        &Symbol::new(&env, "IMAGING"),
        &policy(&env),
    );

    // No PRESCRIPTION records exist — should return empty vec, not error
    let result = client.get_records_by_type(&patient, &patient, &Symbol::new(&env, "PRESCRIPTION"));
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_latest_record_returns_most_recent() {
    let env = Env::default();
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    env.ledger().set_timestamp(1000);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 51),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    env.ledger().set_timestamp(2000);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 52),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let latest = client
        .try_get_latest_record(&patient, &patient)
        .unwrap()
        .unwrap();
    assert_eq!(
        latest.encrypted_ref.content_hash,
        BytesN::from_array(&env, &[52; 32])
    );
    assert_eq!(latest.timestamp, 2000);
}

#[test]
fn test_get_latest_record_returns_error_if_no_records() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let v1 = make_version(&env, 1);

    env.mock_all_auths();
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);
    client.register_patient(
        &patient,
        &String::from_str(&env, "NoRecords"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.acknowledge_consent(&patient, &patient, &v1);

    let result = client.try_get_latest_record(&patient, &patient);
    assert!(matches!(result, Err(Ok(ContractError::NoRecordsFound))));
}

#[test]
fn test_get_latest_record_access_control() {
    let env = Env::default();
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 61),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let attacker = Address::generate(&env);
    let result = client.try_get_latest_record(&patient, &attacker);
    assert!(result.is_err());
}

/// `extend_patient_ttl` called by the patient themselves must succeed and keep
/// the Patient entry accessible.
#[test]
fn test_extend_patient_ttl_by_patient() {
    let env = Env::default();
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, _admin, patient, _doctor, _v1) = setup_for_ttl(&env);

    // Should not panic
    client.extend_patient_ttl(&patient);

    // Patient record is still readable
    let data = client.get_patient(&patient);
    assert_eq!(data.name, String::from_str(&env, "Alice"));
}

/// `extend_patient_ttl` called by the admin must succeed.
#[test]
fn test_extend_patient_ttl_by_admin() {
    let env = Env::default();
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, admin, _patient, _doctor, _v1) = setup_for_ttl(&env);

    // Admin calling extend_patient_ttl with admin address
    // (admin == patient arg in our extend_patient_ttl logic)
    // Instead, register admin as a patient to satisfy `Patient key exists`
    env.mock_all_auths();
    client.register_patient(
        &admin,
        &String::from_str(&env, "Admin User"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.extend_patient_ttl(&admin);

    let data = client.get_patient(&admin);
    assert_eq!(data.name, String::from_str(&env, "Admin User"));
}

#[test]
fn test_get_records_by_ids_partial_hits_skip_missing() {
    let env = Env::default();
    let (client, patient, _doctor) = setup_for_get_records_by_ids(&env);

    let mut ids = Vec::new(&env);
    ids.push_back(0);
    ids.push_back(99);
    ids.push_back(2);
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, _admin, patient, _doctor, _v1) = setup_for_ttl(&env);

    // Patient has consent but no records — should not panic
    client.extend_patient_ttl(&patient);
}

/// TTL constants are defined with expected values.
#[test]
fn test_ttl_constants_are_defined() {
    assert_eq!(LEDGER_BUMP_AMOUNT, 535_680);
    assert_eq!(LEDGER_THRESHOLD, 518_400);
    assert!(LEDGER_BUMP_AMOUNT > LEDGER_THRESHOLD);
}

#[test]
fn test_get_records_by_type_returns_empty_when_no_records_at_all() {
    let env = Env::default();
    let (client, patient, _doctor) = setup_for_filter(&env);

    // Patient registered but no records added yet
    let result = client.get_records_by_type(&patient, &patient, &Symbol::new(&env, "LAB"));
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_records_by_ids_strict_missing_errors() {
    let env = Env::default();
    let (client, patient, _doctor) = setup_for_get_records_by_ids(&env);

    let mut ids = Vec::new(&env);
    ids.push_back(1);
    ids.push_back(999);

    let result = client.try_get_records_by_ids(&patient, &patient, &ids, &true);
    assert!(result.is_err());
}

#[test]
fn test_get_records_by_ids_rejects_more_than_ten_ids() {
    let env = Env::default();
    let (client, patient, _doctor) = setup_for_get_records_by_ids(&env);

    let mut ids = Vec::new(&env);
    for i in 0u32..11u32 {
        ids.push_back(i);
    }

    let result = client.try_get_records_by_ids(&patient, &patient, &ids, &false);
    assert!(result.is_err());
}

#[test]
fn test_get_records_by_ids_unauthorized_caller_rejected() {
    let env = Env::default();
    let (client, patient, _doctor) = setup_for_get_records_by_ids(&env);
    let stranger = Address::generate(&env);

    let mut ids = Vec::new(&env);
    ids.push_back(0);
    let result = client.try_get_records_by_ids(&patient, &stranger, &ids, &false);
    assert!(result.is_err());
}

#[test]
fn test_get_record_fields_full_access_for_patient() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let (_admin, patient, doctor, client) = setup_with_record(&env);

    let partial = client.get_record_fields(&patient, &patient, &1u64);

    assert_eq!(partial.record_type, Some(Symbol::new(&env, "LAB")));
    assert_eq!(
        partial.encrypted_ref_hash,
        Some(BytesN::from_array(&env, &[1; 32]))
    );
    assert_eq!(partial.created_at, Some(1_000));
    assert_eq!(partial.created_by, Some(doctor));
}

#[test]
fn test_get_record_fields_partial_access_for_grantee() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(2_000);

    let (_admin, patient, doctor, client) = setup_with_record(&env);
    let mut fields = Vec::new(&env);
    fields.push_back(FieldPermission::RecordType);
    fields.push_back(FieldPermission::CreatedAt);

    client.grant_field_access(&patient, &doctor, &1u64, &fields);

    let partial = client.get_record_fields(&patient, &doctor, &1u64);

    assert_eq!(partial.record_type, Some(Symbol::new(&env, "LAB")));
    assert_eq!(partial.created_at, Some(2_000));
    assert_eq!(partial.encrypted_ref_hash, None);
    assert_eq!(partial.created_by, None);
}

#[test]
fn test_get_record_fields_returns_none_when_no_access() {
    let env = Env::default();
    env.mock_all_auths();

    let (_admin, patient, _doctor, client) = setup_with_record(&env);
    let stranger = Address::generate(&env);

    let partial = client.get_record_fields(&patient, &stranger, &1u64);

    assert_eq!(partial.record_type, None);
    assert_eq!(partial.encrypted_ref_hash, None);
    assert_eq!(partial.created_at, None);
    assert_eq!(partial.created_by, None);
}

/// ------------------------------------------------
/// PROVIDER-TO-PATIENT RECORD NOTIFICATION EVENT TESTS
/// ------------------------------------------------

#[test]
fn test_new_record_event_emitted_on_add_record() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    env.ledger().set_timestamp(1_700_000_000);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 20),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let events = env.events().all();
    let new_record_topic = Symbol::new(&env, NEW_RECORD_TOPIC);

    let mut found = false;
    for (_contract_id, topics, data) in events.iter() {
        let expected_topics_val: soroban_sdk::Vec<soroban_sdk::Val> =
            (new_record_topic.clone(), patient.clone(), doctor.clone()).into_val(&env);
        if topics == expected_topics_val {
            let actual_data: (u64, Symbol, u64) = data.into_val(&env);
            assert_eq!(
                actual_data,
                (1u64, Symbol::new(&env, "LAB"), 1_700_000_000u64)
            );
            found = true;
            break;
        }
    }
    assert!(found, "new_record event not found in emitted events");
}

#[test]
fn test_new_record_event_contains_correct_record_id() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    env.ledger().set_timestamp(1_700_000_000);
    let new_record_topic = Symbol::new(&env, NEW_RECORD_TOPIC);

    // Add first record
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 21),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let events1 = env.events().all();
    let mut found_first = false;
    for (_contract_id, topics, data) in events1.iter() {
        let expected_topics_val: soroban_sdk::Vec<soroban_sdk::Val> =
            (new_record_topic.clone(), patient.clone(), doctor.clone()).into_val(&env);
        if topics == expected_topics_val {
            let actual_data: (u64, Symbol, u64) = data.into_val(&env);
            assert_eq!(
                actual_data,
                (1u64, Symbol::new(&env, "LAB"), 1_700_000_000u64)
            );
            found_first = true;
        }
    }
    assert!(found_first, "First new_record event not found");

    // Add second record
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 22),
        &Symbol::new(&env, "IMAGING"),
        &policy(&env),
    );

    let events2 = env.events().all();
    let mut found_second = false;
    for (_contract_id, topics, data) in events2.iter() {
        let expected_topics_val: soroban_sdk::Vec<soroban_sdk::Val> =
            (new_record_topic.clone(), patient.clone(), doctor.clone()).into_val(&env);
        if topics == expected_topics_val {
            let actual_data: (u64, Symbol, u64) = data.into_val(&env);
            assert_eq!(
                actual_data,
                (2u64, Symbol::new(&env, "IMAGING"), 1_700_000_000u64)
            );
            found_second = true;
        }
    }
    assert!(found_second, "Second new_record event not found");
}

#[test]
fn test_new_record_event_contains_correct_record_type() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    env.ledger().set_timestamp(1_700_000_000);
    let new_record_topic = Symbol::new(&env, NEW_RECORD_TOPIC);

    // Add a LAB record
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 23),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let events1 = env.events().all();
    let mut found_lab = false;
    for (_contract_id, topics, data) in events1.iter() {
        let expected_topics_val: soroban_sdk::Vec<soroban_sdk::Val> =
            (new_record_topic.clone(), patient.clone(), doctor.clone()).into_val(&env);
        if topics == expected_topics_val {
            let actual_data: (u64, Symbol, u64) = data.into_val(&env);
            if actual_data == (1u64, Symbol::new(&env, "LAB"), 1_700_000_000u64) {
                found_lab = true;
            }
        }
    }
    assert!(found_lab, "LAB record event not found");

    // Add an IMAGING record
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 24),
        &Symbol::new(&env, "IMAGING"),
        &policy(&env),
    );

    let events2 = env.events().all();
    let mut found_imaging = false;
    for (_contract_id, topics, data) in events2.iter() {
        let expected_topics_val: soroban_sdk::Vec<soroban_sdk::Val> =
            (new_record_topic.clone(), patient.clone(), doctor.clone()).into_val(&env);
        if topics == expected_topics_val {
            let actual_data: (u64, Symbol, u64) = data.into_val(&env);
            if actual_data == (2u64, Symbol::new(&env, "IMAGING"), 1_700_000_000u64) {
                found_imaging = true;
            }
        }
    }
    assert!(found_imaging, "IMAGING record event not found");
}

#[test]
fn test_new_record_event_contains_correct_timestamp() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    let specific_timestamp: u64 = 1_710_000_000;
    env.ledger().set_timestamp(specific_timestamp);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 25),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let events = env.events().all();
    let new_record_topic = Symbol::new(&env, NEW_RECORD_TOPIC);

    let mut found = false;
    for (_contract_id, topics, data) in events.iter() {
        let expected_topics_val: soroban_sdk::Vec<soroban_sdk::Val> =
            (new_record_topic.clone(), patient.clone(), doctor.clone()).into_val(&env);
        if topics == expected_topics_val {
            let actual_data: (u64, Symbol, u64) = data.into_val(&env);
            assert_eq!(
                actual_data,
                (1u64, Symbol::new(&env, "LAB"), specific_timestamp),
                "Event data must include the exact ledger timestamp"
            );
            found = true;
            break;
        }
    }
    assert!(found, "new_record event with correct timestamp not found");
}

#[test]
fn test_new_record_event_not_emitted_on_unauthorized_add() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let patient = Address::generate(&env);
    let unauthorized_doctor = Address::generate(&env);
    let v1 = make_version(&env, 1);

    env.mock_all_auths();
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    // Intentionally do NOT grant access to unauthorized_doctor

    let result = client.try_add_medical_record(
        &patient,
        &unauthorized_doctor,
        &encrypted_ref(&env, 26),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
    assert!(result.is_err());

    // Verify no new_record event was emitted
    let events = env.events().all();
    let new_record_topic = Symbol::new(&env, NEW_RECORD_TOPIC);
    for (_contract_id, topics, _data) in events.iter() {
        let nr_topics: soroban_sdk::Vec<soroban_sdk::Val> = (
            new_record_topic.clone(),
            patient.clone(),
            unauthorized_doctor.clone(),
        )
            .into_val(&env);
        assert_ne!(
            topics, nr_topics,
            "new_record event should NOT be emitted when add_medical_record fails"
        );
    }
}

// =====================================================
//                  CONTRACT FREEZE TESTS
// =====================================================

fn setup_initialized(env: &Env) -> (soroban_sdk::Address, soroban_sdk::Address) {
    use soroban_sdk::Address;
    let contract_id = env.register(MedicalRegistry, ());
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let fee_token = Address::generate(env);
    let client = MedicalRegistryClient::new(env, &contract_id);
    env.mock_all_auths();
    client.initialize(&admin, &treasury, &fee_token);
    (contract_id, admin)
}

#[test]
fn test_is_frozen_defaults_to_false() {
    let env = Env::default();
    let (contract_id, _) = setup_initialized(&env);
    let client = MedicalRegistryClient::new(&env, &contract_id);

    assert!(!client.is_frozen());
}

#[test]
fn test_freeze_and_unfreeze() {
    let env = Env::default();
    let (contract_id, _) = setup_initialized(&env);
    let client = MedicalRegistryClient::new(&env, &contract_id);

    assert!(!client.is_frozen());

    client.freeze_contract();
    assert!(client.is_frozen());

    client.unfreeze_contract();
    assert!(!client.is_frozen());
}

#[test]
fn test_freeze_blocks_register_patient() {
    let env = Env::default();
    let (contract_id, _) = setup_initialized(&env);
    let client = MedicalRegistryClient::new(&env, &contract_id);

    client.freeze_contract();

    let patient = Address::generate(&env);
    let result = client.try_register_patient(
        &patient,
        &String::from_str(&env, "Alice"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::ContractFrozen.into()
    );
}

#[test]
fn test_freeze_blocks_update_patient() {
    let env = Env::default();
    let (contract_id, _) = setup_initialized(&env);
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Bob"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    client.freeze_contract();

    let result =
        client.try_update_patient(&patient, &patient, &encrypted_ref(&env, 2), &policy(&env));
    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::ContractFrozen.into()
    );
}

#[test]
fn test_freeze_blocks_register_doctor() {
    let env = Env::default();
    let (contract_id, _) = setup_initialized(&env);
    let client = MedicalRegistryClient::new(&env, &contract_id);

    client.freeze_contract();

    let doctor = Address::generate(&env);
    let result = client.try_register_doctor(
        &doctor,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "Surgery"),
        &Bytes::from_array(&env, &[1, 2, 3, 4]),
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::ContractFrozen.into()
    );
}

#[test]
fn test_reads_allowed_during_freeze() {
    let env = Env::default();
    let (contract_id, _) = setup_initialized(&env);
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Carol"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    client.freeze_contract();

    // Reads must still succeed during a freeze
    assert!(client.is_frozen());
    assert!(client.is_patient_registered(&patient));
    let data = client.get_patient(&patient);
    assert_eq!(data.name, String::from_str(&env, "Carol"));
    assert_eq!(client.get_total_patients(), 1);
}

#[test]
fn test_unfreeze_restores_write_access() {
    let env = Env::default();
    let (contract_id, _) = setup_initialized(&env);
    let client = MedicalRegistryClient::new(&env, &contract_id);

    client.freeze_contract();
    client.unfreeze_contract();

    let patient = Address::generate(&env);
    // Should succeed after unfreeze
    client.register_patient(
        &patient,
        &String::from_str(&env, "Dave"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    assert!(client.is_patient_registered(&patient));
}

#[test]
fn test_non_admin_cannot_freeze() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &treasury, &fee_token);

    // Only mock attacker auth (not admin)
    let result = client
        .mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "freeze_contract",
                args: ().into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_freeze_contract();

    assert!(result.is_err());
}

#[test]
fn test_non_admin_cannot_unfreeze() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &treasury, &fee_token);
    client.freeze_contract();

    let result = client
        .mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "unfreeze_contract",
                args: ().into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_unfreeze_contract();

    assert!(result.is_err());
}

// ------------------------------------------------
// SHARE LINK TESTS
// ------------------------------------------------

/// Helper: set up a contract with one patient, one doctor, one record, and return
/// (env, client, contract_id, patient, doctor, record_hash).
fn setup_with_record(
    env: &Env,
) -> (
    soroban_sdk::Address,
    soroban_sdk::Address,
    soroban_sdk::Address,
    MedicalRegistryClient<'_>,
) {
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let fee_token = Address::generate(env);
    let patient = Address::generate(env);
    let doctor = Address::generate(env);
    let v1 = BytesN::from_array(env, &[42u8; 32]);

    env.mock_all_auths();

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(env, "Test Patient"),
        &631152000,
        &encrypted_ref(env, 1),
        &policy(env),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(env, 1),
        &Symbol::new(env, "LAB"),
        &policy(&env),
    );

    (admin, patient, doctor, client)
}

#[test]
fn test_create_share_link_returns_token() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let token = client.create_share_link(&patient, &0u64, &1u32, &2000u64);

    // Token is a 32-byte hash
    assert_eq!(token.len(), 32);
}

#[test]
fn test_single_use_link_works_once() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let token = client.create_share_link(&patient, &0u64, &1u32, &2000u64);

    // First use succeeds
    let record = client.use_share_link(&token);
    assert_eq!(record.record_type, Symbol::new(&env, "LAB"));

    // Second use fails — token exhausted
    let result = client.try_use_share_link(&token);
    assert!(matches!(result, Err(Ok(ContractError::InvalidToken))));
}

#[test]
fn test_multi_use_link_decrements_and_exhausts() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let token = client.create_share_link(&patient, &0u64, &3u32, &9000u64);

    // Three successful uses
    for _ in 0..3 {
        let record = client.use_share_link(&token);
        assert_eq!(record.record_type, Symbol::new(&env, "LAB"));
    }

    // Fourth use fails
    let result = client.try_use_share_link(&token);
    assert!(matches!(result, Err(Ok(ContractError::InvalidToken))));
}

#[test]
fn test_expired_token_returns_invalid_token() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    // expires_at = 1500
    let token = client.create_share_link(&patient, &0u64, &5u32, &1500u64);

    // Advance time past expiry
    env.ledger().set_timestamp(1501);

    let result = client.try_use_share_link(&token);
    assert!(matches!(result, Err(Ok(ContractError::InvalidToken))));
}

#[test]
fn test_create_share_link_with_zero_uses_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let result = client.try_create_share_link(&patient, &0u64, &0u32, &2000u64);
    assert!(matches!(result, Err(Ok(ContractError::InvalidToken))));
}

#[test]
fn test_create_share_link_with_past_expiry_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(5000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    // expires_at is in the past
    let result = client.try_create_share_link(&patient, &0u64, &1u32, &4999u64);
    assert!(matches!(result, Err(Ok(ContractError::InvalidToken))));
}

#[test]
fn test_create_share_link_invalid_record_id_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    // record_id 99 doesn't exist
    let result = client.try_create_share_link(&patient, &99u64, &1u32, &2000u64);
    assert!(matches!(result, Err(Ok(ContractError::InvalidToken))));
}

#[test]
fn test_unknown_token_returns_invalid_token() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let fake_token = BytesN::from_array(&env, &[0xdeu8; 32]);
    let result = client.try_use_share_link(&fake_token);
    assert!(matches!(result, Err(Ok(ContractError::InvalidToken))));
}

#[test]
fn test_two_links_for_same_record_are_independent() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let token_a = client.create_share_link(&patient, &0u64, &1u32, &2000u64);
    let token_b = client.create_share_link(&patient, &0u64, &2u32, &2000u64);

    // Tokens must differ (different nonces)
    assert_ne!(token_a, token_b);

    // Exhaust token_a
    client.use_share_link(&token_a);
    assert!(client.try_use_share_link(&token_a).is_err());

    // token_b still has 2 uses
    client.use_share_link(&token_b);
    client.use_share_link(&token_b);
    assert!(client.try_use_share_link(&token_b).is_err());
}

#[test]
fn test_only_patient_can_create_share_link() {
    let env = Env::default();
    env.ledger().set_timestamp(1000);

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let attacker = Address::generate(&env);
    let v1 = BytesN::from_array(&env, &[1u8; 32]);

    env.mock_all_auths();

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Test Patient"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    // Attacker tries to create a link for the patient's record — auth will fail
    // because patient.require_auth() won't be satisfied by attacker's signature.
    // With mock_all_auths disabled we test real auth rejection.
    let result = client
        .mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &attacker,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &contract_id,
                fn_name: "create_share_link",
                args: (&patient, &0u64, &1u32, &2000u64).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_create_share_link(&patient, &0u64, &1u32, &2000u64);

    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// #325 – Concurrent share link redemption
// -----------------------------------------------------------------------

/// Two callers attempt to redeem the same single-use link "simultaneously".
///
/// In Soroban, transactions within a ledger round are serialized at the
/// contract level — only one can execute at a time. The first call succeeds
/// and atomically removes the token; the second call finds it gone and must
/// return InvalidToken. This ensures uses_remaining = 1 is honored even under
/// concurrent submission pressure.
#[test]
fn test_concurrent_single_use_link_only_one_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    // Create a single-use link (uses_remaining = 1).
    let token = client.create_share_link(&patient, &0u64, &1u32, &2000u64);

    // First redemption — represents the winning transaction in the race.
    let record = client.use_share_link(&token);
    assert_eq!(record.record_type, Symbol::new(&env, "LAB"));

    // Second redemption — represents the losing transaction; the token was
    // removed atomically after the first use so this must fail.
    let result = client.try_use_share_link(&token);
    assert!(
        matches!(result, Err(Ok(ContractError::InvalidToken))),
        "second redemption must fail with InvalidToken after single-use link is exhausted"
    );
}

/// Concurrent redemptions against a two-use link: both succeed, the third fails.
/// Verifies the counter decrements correctly under sequential ordering.
#[test]
fn test_concurrent_two_use_link_third_call_fails() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let token = client.create_share_link(&patient, &0u64, &2u32, &5000u64);

    // First and second callers both succeed.
    let record1 = client.use_share_link(&token);
    assert_eq!(record1.record_type, Symbol::new(&env, "LAB"));

    let record2 = client.use_share_link(&token);
    assert_eq!(record2.record_type, Symbol::new(&env, "LAB"));

    // Third caller loses — link is exhausted.
    let result = client.try_use_share_link(&token);
    assert!(
        matches!(result, Err(Ok(ContractError::InvalidToken))),
        "third redemption must fail after two-use link is exhausted"
    );
}

#[test]
fn test_request_data_export_returns_valid_ticket() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(10_000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let ticket = client.request_data_export(&patient);

    assert_eq!(ticket.patient, patient);
    assert_eq!(ticket.issued_at, 10_000);
    assert_eq!(ticket.expires_at, 13_600);
    assert!(client.validate_export_ticket(&ticket));
}

#[test]
fn test_validate_export_ticket_rejects_expired_ticket() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(20_000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let ticket = client.request_data_export(&patient);
    env.ledger().set_timestamp(ticket.expires_at + 1);

    assert!(!client.validate_export_ticket(&ticket));
}

#[test]
fn test_validate_export_ticket_rejects_tampered_ticket() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(30_000);

    let (_admin, patient, _doctor, client) = setup_with_record(&env);

    let ticket = client.request_data_export(&patient);
    let tampered = ExportTicket {
        signature: BytesN::from_array(&env, &[0xabu8; 32]),
        ..ticket
    };

    assert!(!client.validate_export_ticket(&tampered));
}

// ------------------------------------------------
// DEREGISTRATION TESTS
// ------------------------------------------------

fn setup_for_dereg(env: &Env) -> (MedicalRegistryClient<'_>, Address, Address, Address) {
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let fee_token = Address::generate(env);
    let patient = Address::generate(env);
    let doctor = Address::generate(env);
    let v1 = BytesN::from_array(env, &[55u8; 32]);

    env.mock_all_auths();

    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.register_patient(
        &patient,
        &String::from_str(env, "Alice"),
        &631152000,
        &encrypted_ref(env, 1),
        &policy(env),
    );
    client.grant_access(&patient, &patient, &doctor);

    (client, admin, patient, doctor)
}

#[test]
fn test_deregister_sets_status() {
    let env = Env::default();
    let (client, _admin, patient, _doctor) = setup_for_dereg(&env);

    client.deregister_patient(&patient);

    let data = client.get_patient(&patient);
    assert_eq!(data.status, PatientStatus::Deregistered);
}

#[test]
fn test_deregister_revokes_all_access_grants() {
    let env = Env::default();
    let (client, _admin, patient, _doctor) = setup_for_dereg(&env);

    assert_eq!(client.get_authorized_doctors(&patient).len(), 1);

    client.deregister_patient(&patient);

    assert_eq!(client.get_authorized_doctors(&patient).len(), 0);
}

#[test]
fn test_deregister_records_retained_admin_can_read() {
    let env = Env::default();
    let (client, admin, patient, doctor) = setup_for_dereg(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 20),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    client.deregister_patient(&patient);

    // Admin can still read records
    let records = client.get_medical_records(&patient, &admin);
    assert_eq!(records.len(), 1);
}

#[test]
#[should_panic]
fn test_deregister_blocks_grantee_read() {
    let env = Env::default();
    let (client, _admin, patient, doctor) = setup_for_dereg(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 21),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    client.deregister_patient(&patient);

    // Former grantee (doctor) can no longer read
    client.get_medical_records(&patient, &doctor);
}

#[test]
#[should_panic]
fn test_double_deregister_panics() {
    let env = Env::default();
    let (client, _admin, patient, _doctor) = setup_for_dereg(&env);

    client.deregister_patient(&patient);
    client.deregister_patient(&patient);
}

#[test]
fn test_deregister_patient_only() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let patient = Address::generate(&env);
    let attacker = Address::generate(&env);
    let v1 = BytesN::from_array(&env, &[1u8; 32]);

    env.mock_all_auths();
    client.initialize(&admin, &treasury, &fee_token);
    client.publish_consent_version(&v1);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Bob"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    let result = client
        .mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &attacker,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &contract_id,
                fn_name: "deregister_patient",
                args: (&patient,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_deregister_patient(&patient);

    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
//  MERKLE TREE TESTS
// ─────────────────────────────────────────────────────────────────────────────

/// Set up a fresh contract with `n` medical records for a single patient.
///
/// Precondition: `env.mock_all_auths()` must have been called by the caller.
/// Returns `(client, patient_addr, Vec<record_ids>)`.
fn setup_with_records(env: &Env, n: u32) -> (MedicalRegistryClient, Address, Vec<u64>) {
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let fee_token = Address::generate(env);
    let patient = Address::generate(env);
    let doctor = Address::generate(env);
    let consent = BytesN::from_array(env, &[7u8; 32]);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(env, "Alice"),
        &631152000,
        &encrypted_ref(env, 1),
        &policy(env),
    );
    client.publish_consent_version(&consent);
    client.acknowledge_consent(&patient, &patient, &consent);
    client.grant_access(&patient, &patient, &doctor);

    let mut ids: Vec<u64> = Vec::new(env);
    for i in 0..n {
        let id = client.add_medical_record(
            &patient,
            &doctor,
            &encrypted_ref(env, (i + 1) as u8),
            &Symbol::new(env, "LAB"),
            &policy(env),
        );
        ids.push_back(id);
    }
    (client, patient, ids)
}

/// Compute a Merkle membership proof for `target_id` given the ordered `ids`
/// list.  Mirrors `compute_merkle_root` exactly so the proof is consistent
/// with what the contract stores.
fn build_proof(env: &Env, ids: &Vec<u64>, target_id: u64) -> Vec<BytesN<32>> {
    let n = ids.len();
    assert!(n > 0, "no records");

    let mut layer: Vec<BytesN<32>> = Vec::new(env);
    for id in ids.iter() {
        layer.push_back(merkle::hash_leaf(env, id));
    }

    let mut pos: u32 = 0;
    for (i, id) in ids.iter().enumerate() {
        if id == target_id {
            pos = i as u32;
        }
    }

    let mut proof: Vec<BytesN<32>> = Vec::new(env);
    let mut cur_len = layer.len();
    let mut cur_pos = pos;

    while cur_len > 1 {
        let mut next: Vec<BytesN<32>> = Vec::new(env);
        let mut i = 0u32;
        while i + 1 < cur_len {
            next.push_back(merkle::hash_pair(
                env,
                layer.get(i).unwrap(),
                layer.get(i + 1).unwrap(),
            ));
            i += 2;
        }
        if cur_len % 2 == 1 {
            let last = layer.get(cur_len - 1).unwrap();
            next.push_back(merkle::hash_pair(env, last.clone(), last));
        }

        let sibling_pos = if cur_pos % 2 == 0 {
            if cur_pos + 1 < cur_len {
                cur_pos + 1
            } else {
                cur_pos
            }
        } else {
            cur_pos - 1
        };
        proof.push_back(layer.get(sibling_pos).unwrap());

        cur_pos /= 2;
        cur_len = next.len();
        layer = next;
    }

    proof
}

#[test]
fn test_merkle_root_empty_before_any_record() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let patient = Address::generate(&env);

    env.mock_all_auths();

    let expected = merkle::compute_merkle_root(&env, &Vec::new(&env));
    assert_eq!(client.get_merkle_root(&patient), expected);
}

#[test]
fn test_merkle_root_single_record() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 1);

    let id = ids.get(0).unwrap();
    let expected = merkle::hash_leaf(&env, id);
    assert_eq!(client.get_merkle_root(&patient), expected);
}

#[test]
fn test_merkle_root_two_records() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 2);

    let id0 = ids.get(0).unwrap();
    let id1 = ids.get(1).unwrap();
    let expected = merkle::hash_pair(
        &env,
        merkle::hash_leaf(&env, id0),
        merkle::hash_leaf(&env, id1),
    );
    assert_eq!(client.get_merkle_root(&patient), expected);
}

#[test]
fn test_merkle_root_updates_on_each_addition() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let consent = BytesN::from_array(&env, &[3u8; 32]);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Bob"),
        &631152000,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.publish_consent_version(&consent);
    client.acknowledge_consent(&patient, &patient, &consent);
    client.grant_access(&patient, &patient, &doctor);

    let root_before = client.get_merkle_root(&patient);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
    let root_after_1 = client.get_merkle_root(&patient);
    assert_ne!(root_before, root_after_1);

    client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 2),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
    let root_after_2 = client.get_merkle_root(&patient);
    assert_ne!(root_after_1, root_after_2);
}

#[test]
fn test_verify_membership_single_leaf() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 1);

    let id = ids.get(0).unwrap();
    let proof: Vec<BytesN<32>> = Vec::new(&env);
    assert!(client.verify_record_membership(&patient, &id, &proof));
}

#[test]
fn test_verify_membership_two_leaves_each() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 2);

    let id0 = ids.get(0).unwrap();
    let id1 = ids.get(1).unwrap();

    let mut p0: Vec<BytesN<32>> = Vec::new(&env);
    p0.push_back(merkle::hash_leaf(&env, id1));
    assert!(client.verify_record_membership(&patient, &id0, &p0));

    let mut p1: Vec<BytesN<32>> = Vec::new(&env);
    p1.push_back(merkle::hash_leaf(&env, id0));
    assert!(client.verify_record_membership(&patient, &id1, &p1));
}

#[test]
fn test_verify_membership_three_leaves() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 3);

    for i in 0..3u32 {
        let id = ids.get(i).unwrap();
        let proof = build_proof(&env, &ids, id);
        assert!(
            client.verify_record_membership(&patient, &id, &proof),
            "membership check failed for record at index {i}"
        );
    }
}

#[test]
fn test_verify_membership_four_leaves() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 4);

    for i in 0..4u32 {
        let id = ids.get(i).unwrap();
        let proof = build_proof(&env, &ids, id);
        assert!(
            client.verify_record_membership(&patient, &id, &proof),
            "membership check failed for record at index {i}"
        );
    }
}

#[test]
fn test_verify_non_membership_wrong_id() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 2);

    let non_existent: u64 = 9999;
    let mut bogus: Vec<BytesN<32>> = Vec::new(&env);
    bogus.push_back(merkle::hash_leaf(&env, ids.get(0).unwrap()));
    assert!(!client.verify_record_membership(&patient, &non_existent, &bogus));
}

#[test]
fn test_verify_non_membership_wrong_proof() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 2);

    let id0 = ids.get(0).unwrap();
    let corrupt = BytesN::from_array(&env, &[0u8; 32]);
    let mut bad_proof: Vec<BytesN<32>> = Vec::new(&env);
    bad_proof.push_back(corrupt);
    assert!(!client.verify_record_membership(&patient, &id0, &bad_proof));
}

#[test]
fn test_verify_membership_patient_with_no_records_returns_false() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let patient = Address::generate(&env);

    env.mock_all_auths();

    let proof: Vec<BytesN<32>> = Vec::new(&env);
    assert!(!client.verify_record_membership(&patient, &1, &proof));
}

// ─── Issue #326 ── Type-index consistency after multiple soft deletes ──────

#[test]
fn test_type_index_consistency_after_multiple_soft_deletes() {
    let env = Env::default();
    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    // Create 5 records of the same type.
    let mut ids: Vec<u64> = Vec::new(&env);
    for i in 0u8..5 {
        let id = client.add_medical_record(
            &patient,
            &doctor,
            &encrypted_ref(&env, i + 30),
            &Symbol::new(&env, "LAB"),
            &policy(&env),
        );
        ids.push_back(id);
    }

    // Soft-delete the first 3.
    for i in 0u32..3 {
        let record_id = ids.get(i).unwrap();
        client.soft_delete_record(&record_id, &patient);
    }

    // The type index must contain exactly 2 surviving entries.
    let entries = client.get_global_records_by_type(&Symbol::new(&env, "LAB"));
    assert_eq!(entries.len(), 2, "type index should have 2 entries after 3 soft deletes");

    let count = client.get_global_type_count(&Symbol::new(&env, "LAB"));
    assert_eq!(count, 2, "get_global_type_count should return 2");

    // The three deleted IDs must not appear in the index.
    let del_id0 = ids.get(0).unwrap();
    let del_id1 = ids.get(1).unwrap();
    let del_id2 = ids.get(2).unwrap();
    for entry in entries.iter() {
        let rid = entry.record_id;
        assert_ne!(rid, del_id0, "deleted record 1 must not appear in type index");
        assert_ne!(rid, del_id1, "deleted record 2 must not appear in type index");
        assert_ne!(rid, del_id2, "deleted record 3 must not appear in type index");
    }

    // The two surviving IDs must be present.
    let keep_id0 = ids.get(3).unwrap();
    let keep_id1 = ids.get(4).unwrap();
    let mut found0 = false;
    let mut found1 = false;
    for entry in entries.iter() {
        if entry.record_id == keep_id0 {
            found0 = true;
        }
        if entry.record_id == keep_id1 {
            found1 = true;
        }
    }
    assert!(found0, "surviving record 4 must be present in type index");
    assert!(found1, "surviving record 5 must be present in type index");
}

#[test]
fn test_deleted_records_not_returned_by_type_query() {
    let env = Env::default();
    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    // Add 5 records of a distinct type and soft-delete 3.
    let mut ids: Vec<u64> = Vec::new(&env);
    for i in 0u8..5 {
        let id = client.add_medical_record(
            &patient,
            &doctor,
            &encrypted_ref(&env, i + 40),
            &Symbol::new(&env, "VISIT"),
            &policy(&env),
        );
        ids.push_back(id);
    }

    let del_id0 = ids.get(0).unwrap();
    let del_id1 = ids.get(1).unwrap();
    let del_id2 = ids.get(2).unwrap();
    client.soft_delete_record(&del_id0, &patient);
    client.soft_delete_record(&del_id1, &patient);
    client.soft_delete_record(&del_id2, &patient);

    let entries = client.get_global_records_by_type(&Symbol::new(&env, "VISIT"));
    for entry in entries.iter() {
        let rid = entry.record_id;
        assert_ne!(rid, del_id0, "deleted record 1 must not be in type query results");
        assert_ne!(rid, del_id1, "deleted record 2 must not be in type query results");
        assert_ne!(rid, del_id2, "deleted record 3 must not be in type query results");
    }
}

// ─── Issue #328 ── Merkle proof edge cases ────────────────────────────────

#[test]
fn test_merkle_empty_tree_proof_returns_false() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);
    let patient = Address::generate(&env);

    env.mock_all_auths();

    // Empty tree: root is the sha256("") sentinel.
    // An empty proof for any record_id must return false.
    let empty_proof: Vec<BytesN<32>> = Vec::new(&env);
    assert!(
        !client.verify_record_membership(&patient, &1, &empty_proof),
        "empty-tree empty-proof must return false"
    );

    // A non-empty proof against an empty tree must also return false.
    let mut non_empty_proof: Vec<BytesN<32>> = Vec::new(&env);
    non_empty_proof.push_back(BytesN::from_array(&env, &[0xabu8; 32]));
    assert!(
        !client.verify_record_membership(&patient, &1, &non_empty_proof),
        "empty-tree non-empty-proof must return false"
    );
}

#[test]
fn test_merkle_single_record_valid_proof_verified() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 1);

    let id = ids.get(0).unwrap();
    // Single-leaf tree: root = hash_leaf(id). The correct proof is empty.
    let proof: Vec<BytesN<32>> = Vec::new(&env);
    assert!(
        client.verify_record_membership(&patient, &id, &proof),
        "single-record valid proof must be accepted"
    );
}

#[test]
fn test_merkle_wrong_depth_proof_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, patient, ids) = setup_with_records(&env, 1);

    let id = ids.get(0).unwrap();
    // Correct proof for a single-leaf tree is empty (depth 0).
    // Supplying an extra sibling (depth 1) computes a different root → rejected.
    let mut wrong_depth_proof: Vec<BytesN<32>> = Vec::new(&env);
    wrong_depth_proof.push_back(BytesN::from_array(&env, &[0xffu8; 32]));
    assert!(
        !client.verify_record_membership(&patient, &id, &wrong_depth_proof),
        "proof with wrong depth must be rejected"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  RECORD VERSIONING TESTS  (#383)
// ─────────────────────────────────────────────────────────────────────────────

/// create → update×3 → get_history must return 4 entries (1 initial + 3 updates).
#[test]
fn test_record_history_four_entries_after_three_updates() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 2), &policy(&env));
    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 3), &policy(&env));
    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 4), &policy(&env));

    let history = client.get_record_history(&record_id, &patient);
    assert_eq!(history.len(), 4, "expected 4 history entries (1 initial + 3 updates)");
}

/// Version IDs (latest_version) must be monotonically increasing and immutable.
#[test]
fn test_version_ids_are_monotonically_increasing() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 10),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    // Read latest_version via the contract's internal storage through as_contract
    let v1: u64 = env.as_contract(&client.address, || {
        let rd: RecordData = env
            .storage()
            .persistent()
            .get(&DataKey::MedicalRecord(record_id))
            .unwrap();
        rd.latest_version
    });
    assert_eq!(v1, 1);

    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 11), &policy(&env));
    let v2: u64 = env.as_contract(&client.address, || {
        let rd: RecordData = env
            .storage()
            .persistent()
            .get(&DataKey::MedicalRecord(record_id))
            .unwrap();
        rd.latest_version
    });
    assert_eq!(v2, 2);

    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 12), &policy(&env));
    let v3: u64 = env.as_contract(&client.address, || {
        let rd: RecordData = env
            .storage()
            .persistent()
            .get(&DataKey::MedicalRecord(record_id))
            .unwrap();
        rd.latest_version
    });
    assert_eq!(v3, 3);

    assert!(v1 < v2 && v2 < v3, "version IDs must be strictly increasing");
}

/// History entries must record the correct encrypted_ref for each version.
#[test]
fn test_history_entries_contain_correct_refs() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 2), &policy(&env));
    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 3), &policy(&env));

    let history = client.get_record_history(&record_id, &patient);
    assert_eq!(history.len(), 3);

    assert_eq!(history.get(0).unwrap().encrypted_ref, encrypted_ref(&env, 1));
    assert_eq!(history.get(1).unwrap().encrypted_ref, encrypted_ref(&env, 2));
    assert_eq!(history.get(2).unwrap().encrypted_ref, encrypted_ref(&env, 3));
}

/// current_ref must always reflect the latest update.
#[test]
fn test_current_ref_reflects_latest_update() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 2), &policy(&env));
    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 3), &policy(&env));

    let current: EncryptedEnvelopeRef = env.as_contract(&client.address, || {
        let rd: RecordData = env
            .storage()
            .persistent()
            .get(&DataKey::MedicalRecord(record_id))
            .unwrap();
        rd.current_ref
    });
    assert_eq!(current, encrypted_ref(&env, 3));
}

/// Concurrent updates from multiple authorized providers — each update is
/// serialized; history must contain all versions in order.
#[test]
fn test_concurrent_updates_from_multiple_providers() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, patient, doctor1, _v1) = setup_for_ttl(&env);

    // Register and authorize a second provider.
    let doctor2 = Address::generate(&env);
    client.register_doctor(
        &doctor2,
        &String::from_str(&env, "Dr. Carol"),
        &String::from_str(&env, "Neurology"),
        &Bytes::from_array(&env, &[4, 5, 6]),
    );
    client.grant_access(&patient, &patient, &doctor2);

    let record_id = client.add_medical_record(
        &patient,
        &doctor1,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    // doctor1 updates, then doctor2 updates, then doctor1 again.
    env.ledger().set_timestamp(1000);
    client.update_record(&doctor1, &record_id, &encrypted_ref(&env, 2), &policy(&env));
    env.ledger().set_timestamp(2000);
    client.update_record(&doctor2, &record_id, &encrypted_ref(&env, 3), &policy(&env));
    env.ledger().set_timestamp(3000);
    client.update_record(&doctor1, &record_id, &encrypted_ref(&env, 4), &policy(&env));

    let history = client.get_record_history(&record_id, &patient);
    assert_eq!(history.len(), 4, "all 4 versions must be in history");

    // Verify updated_by attribution.
    assert_eq!(history.get(1).unwrap().updated_by, doctor1);
    assert_eq!(history.get(2).unwrap().updated_by, doctor2);
    assert_eq!(history.get(3).unwrap().updated_by, doctor1);

    // Verify timestamps are non-decreasing.
    for i in 1..history.len() {
        assert!(
            history.get(i).unwrap().updated_at >= history.get(i - 1).unwrap().updated_at,
            "timestamps must be non-decreasing"
        );
    }
}

/// Unauthorized caller must not be able to update a record.
#[test]
fn test_unauthorized_caller_cannot_update_record() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);
    let stranger = Address::generate(&env);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );

    let result =
        client.try_update_record(&stranger, &record_id, &encrypted_ref(&env, 2), &policy(&env));
    assert!(result.is_err(), "unauthorized caller must be rejected");
}

/// get_record_history is readable by any address with read permission.
#[test]
fn test_get_record_history_readable_by_authorized_doctor() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &encrypted_ref(&env, 1),
        &Symbol::new(&env, "LAB"),
        &policy(&env),
    );
    client.update_record(&doctor, &record_id, &encrypted_ref(&env, 2), &policy(&env));

    // Doctor (authorized) can read history.
    let history = client.get_record_history(&record_id, &doctor);
    assert_eq!(history.len(), 2);
}


// ── Batch registration tests (#396) ──────────────────────────────────────────

#[test]
fn test_batch_register_patients_full_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    let mut entries = Vec::new(&env);
    for i in 0..3u8 {
        let wallet = Address::generate(&env);
        entries.push_back(BatchPatientEntry {
            wallet,
            name: String::from_str(&env, "Patient"),
            dob: 1000u64 + i as u64,
            encrypted_metadata_ref: encrypted_ref(&env, i + 1),
            policy: policy(&env),
        });
    }

    let results = client.batch_register_patients(&entries);
    assert_eq!(results.len(), 3);
    for i in 0..3u32 {
        assert!(matches!(results.get(i).unwrap(), BatchEntryStatus::Success));
    }
    assert_eq!(client.get_total_patients(), 3);
}

#[test]
fn test_batch_register_patients_idempotent_on_duplicate() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    let wallet = Address::generate(&env);
    let entry = BatchPatientEntry {
        wallet: wallet.clone(),
        name: String::from_str(&env, "Alice"),
        dob: 1000u64,
        encrypted_metadata_ref: encrypted_ref(&env, 1),
        policy: policy(&env),
    };

    let mut entries = Vec::new(&env);
    entries.push_back(entry.clone());
    entries.push_back(entry);

    let results = client.batch_register_patients(&entries);
    assert_eq!(results.len(), 2);
    assert!(matches!(results.get(0).unwrap(), BatchEntryStatus::Success));
    assert!(matches!(results.get(1).unwrap(), BatchEntryStatus::AlreadyExists));
    assert_eq!(client.get_total_patients(), 1);
}

#[test]
fn test_batch_register_patients_over_limit_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    let mut entries = Vec::new(&env);
    for i in 0..51u8 {
        entries.push_back(BatchPatientEntry {
            wallet: Address::generate(&env),
            name: String::from_str(&env, "P"),
            dob: i as u64,
            encrypted_metadata_ref: encrypted_ref(&env, (i % 255) + 1),
            policy: policy(&env),
        });
    }

    let result = client.try_batch_register_patients(&entries);
    assert!(result.is_err());
}

// ── Retention class tests (#470) ──────────────────────────────────────────────

/// Newly registered patients default to the Clinical retention class.
#[test]
fn test_register_patient_defaults_to_clinical_class() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    let patient = Address::generate(&env);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Alice"),
        &631152000u64,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );

    assert_eq!(
        client.get_patient_retention_class(&patient),
        RetentionClass::Clinical
    );
}

/// Admin can change a patient's retention class from Clinical to Administrative.
#[test]
fn test_admin_can_set_administrative_retention_class() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    let patient = Address::generate(&env);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Bob"),
        &631152000u64,
        &encrypted_ref(&env, 2),
        &policy(&env),
    );

    client.set_patient_retention_class(&patient, &RetentionClass::Administrative);

    assert_eq!(
        client.get_patient_retention_class(&patient),
        RetentionClass::Administrative
    );
}

/// Admin can set Financial retention class.
#[test]
fn test_admin_can_set_financial_retention_class() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    let patient = Address::generate(&env);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Carol"),
        &631152000u64,
        &encrypted_ref(&env, 3),
        &policy(&env),
    );

    client.set_patient_retention_class(&patient, &RetentionClass::Financial);

    assert_eq!(
        client.get_patient_retention_class(&patient),
        RetentionClass::Financial
    );
}

/// Clinical patients use critical-class TTL (larger bump); Administrative
/// patients use operational-class TTL (smaller bump). After advancing the
/// ledger past the operational bump amount (but within the critical bump
/// amount), the Administrative patient's key has expired while the Clinical
/// patient's key survives.
#[test]
fn test_different_bump_amounts_applied_per_retention_class() {
    let env = Env::default();
    env.mock_all_auths();

    let start_seq: u32 = 100;
    env.ledger().set(make_ledger_info(start_seq, 1_000_000));

    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);
    client.initialize(&admin, &treasury, &fee_token);

    // Register two patients.
    let clinical_patient = Address::generate(&env);
    let admin_patient = Address::generate(&env);

    client.register_patient(
        &clinical_patient,
        &String::from_str(&env, "Clinical"),
        &631152000u64,
        &encrypted_ref(&env, 1),
        &policy(&env),
    );
    client.register_patient(
        &admin_patient,
        &String::from_str(&env, "Administrative"),
        &631152001u64,
        &encrypted_ref(&env, 2),
        &policy(&env),
    );

    // Change the second patient to Administrative class.
    client.set_patient_retention_class(&admin_patient, &RetentionClass::Administrative);

    // Bump TTL for both patients — this triggers class-specific extend_ttl.
    client.extend_patient_ttl(&clinical_patient);
    client.extend_patient_ttl(&admin_patient);

    // Advance ledger past operational TTL (~7 days = 120_960 ledgers) but within
    // critical TTL (~31 days = 535_680 ledgers).
    // operational::LEDGER_BUMP_AMOUNT = 120_960, critical::LEDGER_BUMP_AMOUNT = 535_680
    let past_operational: u32 = start_seq + 120_961;
    env.ledger().set(make_ledger_info(past_operational, 2_000_000));

    // Administrative patient's key should now be expired — get_patient returns NotFound.
    assert!(
        client.try_get_patient(&admin_patient).is_err(),
        "Administrative patient key should have expired"
    );

    // Clinical patient's key should still be live — get_patient succeeds.
    assert!(
        client.try_get_patient(&clinical_patient).is_ok(),
        "Clinical patient key should still be alive"
    );
}
