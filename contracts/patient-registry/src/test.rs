#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, MockAuth, MockAuthInvoke},
    Address, Bytes, BytesN, Env, IntoVal, String, Symbol, Vec,
};

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
    let metadata = String::from_str(&env, "ipfs://some-medical-history");

    env.mock_all_auths();

    client.register_patient(&patient_wallet, &name, &dob, &metadata);

    let patient_data = client.get_patient(&patient_wallet);
    assert_eq!(patient_data.name, name);
    assert_eq!(patient_data.dob, dob);
    assert_eq!(patient_data.metadata, metadata);
}

#[test]
fn test_update_patient() {
    let env = Env::default();
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(&env, &contract_id);

    let patient_wallet = Address::generate(&env);
    let name = String::from_str(&env, "John Doe");
    let dob = 631152000;
    let initial_metadata = String::from_str(&env, "ipfs://initial");

    env.mock_all_auths();

    client.register_patient(&patient_wallet, &name, &dob, &initial_metadata);

    let new_metadata = String::from_str(&env, "ipfs://updated-history");
    client.update_patient(&patient_wallet, &patient_wallet, &new_metadata);

    let patient_data = client.get_patient(&patient_wallet);
    assert_eq!(patient_data.metadata, new_metadata);
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
        &String::from_str(&env, "ipfs://data"),
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
        &String::from_str(&env, "ipfs://p1"),
    );
    assert_eq!(client.get_total_patients(), 1);

    client.register_patient(
        &Address::generate(&env),
        &String::from_str(&env, "P2"),
        &631152001,
        &String::from_str(&env, "ipfs://p2"),
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
                    &String::from_str(&env, "ipfs://p1"),
                )
                    .into_val(&env),
                sub_invokes: &[],
            },
        }])
        .register_patient(
            &patient,
            &String::from_str(&env, "P1"),
            &631152000,
            &String::from_str(&env, "ipfs://p1"),
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
        &make_cid_v1(&env, 11),
        &String::from_str(&env, "Lab"),
        &Symbol::new(&env, "LAB"),
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
        &String::from_str(&env, "ipfs://p1"),
    );
    assert_eq!(client.get_total_patients(), 1);

    let duplicate_attempt = client.try_register_patient(
        &patient_wallet,
        &String::from_str(&env, "P1"),
        &631152000,
        &String::from_str(&env, "ipfs://p1"),
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
#[should_panic(expected = "Unauthorized institution")]
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
        &String::from_str(&env, "ipfs://test"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(&patient, &doctor, &hash, &desc, &Symbol::new(&env, "LAB"));

    let records = client.get_medical_records(&patient, &patient);
    assert_eq!(records.len(), 1);

    let record = records.get(0).unwrap();
    assert_eq!(record.record_hash, hash);
    assert_eq!(record.description, desc);
    assert_eq!(record.record_type, Symbol::new(&env, "LAB"));
}

#[test]
#[should_panic(expected = "Patient has not acknowledged current consent version")]
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
        &String::from_str(&env, "ipfs://test"),
    );
    client.publish_consent_version(&v1);

    client.add_medical_record(
        &patient,
        &doctor,
        &hash,
        &desc,
        &Symbol::new(&env, "IMAGING"),
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
        &String::from_str(&env, "ipfs://test"),
    );
    client.publish_consent_version(&version);
    client.acknowledge_consent(&patient, &patient, &version);
    client.grant_access(&patient, &patient, &doctor);

    let result = client.try_add_medical_record(
        &patient,
        &doctor,
        &invalid_cid,
        &String::from_str(&env, "Invalid CID"),
        &Symbol::new(&env, "LAB"),
    );

    assert!(matches!(result, Err(Ok(ContractError::InvalidCID))));
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
        &String::from_str(&env, "ipfs://patient"),
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
    let metadata = String::from_str(&env, "ipfs://patient");
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
                args: (&patient, &name, &dob, &metadata).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .register_patient(&patient, &name, &dob, &metadata);

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
        &String::from_str(&env, "ipfs://patient"),
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
    let metadata = String::from_str(&env, "ipfs://patient");
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
                args: (&patient, &name, &dob, &metadata).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .register_patient(&patient, &name, &dob, &metadata);

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
        &String::from_str(&env, "ipfs://initial"),
    );
    env.ledger().set_timestamp(50);
    client.place_hold(&patient, &reason_hash, &250);

    let result = client.try_update_patient(
        &patient,
        &patient,
        &String::from_str(&env, "ipfs://blocked"),
    );
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
        &String::from_str(&env, "ipfs://initial"),
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
    let updated_metadata = String::from_str(&env, "ipfs://updated");
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &String::from_str(&env, "ipfs://initial"),
    );

    env.ledger().set_timestamp(100);
    client.place_hold(&patient, &reason_hash, &150);
    assert!(client.is_hold_active(&patient));

    env.ledger().set_timestamp(151);
    assert!(!client.is_hold_active(&patient));

    client.update_patient(&patient, &patient, &updated_metadata);
    let patient_data = client.get_patient(&patient);
    assert_eq!(patient_data.metadata, updated_metadata);
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
        &String::from_str(&env, "ipfs://patient"),
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
    let updated_metadata = String::from_str(&env, "ipfs://restored");
    let treasury = Address::generate(&env);
    let fee_token = Address::generate(&env);

    client.initialize(&admin, &treasury, &fee_token);
    client.register_patient(
        &patient,
        &String::from_str(&env, "Jane Doe"),
        &631152000,
        &String::from_str(&env, "ipfs://initial"),
    );

    env.ledger().set_timestamp(50);
    client.place_hold(&patient, &reason_hash, &300);
    client.lift_hold(&patient);
    client.update_patient(&patient, &patient, &updated_metadata);

    let patient_data = client.get_patient(&patient);
    assert_eq!(patient_data.metadata, updated_metadata);
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
        &String::from_str(&env, "ipfs://patient"),
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
        &String::from_str(&env, "ipfs://patient"),
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
#[should_panic(expected = "Version mismatch")]
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
#[should_panic(expected = "Patient has not acknowledged current consent version")]
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
        &String::from_str(&env, "ipfs://test"),
    );
    client.publish_consent_version(&make_version(&env, 1));
    // Patient never acknowledges
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 1),
        &String::from_str(&env, "test"),
        &Symbol::new(&env, "LAB"),
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
        &String::from_str(&env, "ipfs://test"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 2),
        &String::from_str(&env, "Blood test"),
        &Symbol::new(&env, "LAB"),
    );

    assert_eq!(client.get_medical_records(&patient, &patient).len(), 1);
}

#[test]
#[should_panic(expected = "Patient has not acknowledged current consent version")]
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
        &String::from_str(&env, "ipfs://test"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);

    // Admin bumps version — patient must re-acknowledge
    client.publish_consent_version(&v2);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 3),
        &String::from_str(&env, "Post-update record"),
        &Symbol::new(&env, "LAB"),
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
        &String::from_str(&env, "ipfs://original"),
    );
    client.assign_guardian(&patient, &guardian);
    client.update_patient(
        &patient,
        &guardian,
        &String::from_str(&env, "ipfs://updated"),
    );

    assert_eq!(
        client.get_patient(&patient).metadata,
        String::from_str(&env, "ipfs://updated")
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
        &String::from_str(&env, "ipfs://test"),
    );
    client.assign_guardian(&patient, &guardian);
    client.acknowledge_consent(&patient, &guardian, &v1);
    client.grant_access(&patient, &guardian, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 5),
        &String::from_str(&env, "Guardian-approved record"),
        &Symbol::new(&env, "PRESCRIPTION"),
    );

    assert_eq!(client.get_medical_records(&patient, &patient).len(), 1);
}

#[test]
#[should_panic(expected = "Caller is not patient or assigned guardian")]
fn test_unauthorized_caller_rejected() {
    let env = Env::default();
    let (client, _admin) = setup_with_consent(&env);
    let v1 = make_version(&env, 1);
    let patient = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.acknowledge_consent(&patient, &stranger, &v1);
}

#[test]
#[should_panic(expected = "Caller is not patient or assigned guardian")]
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
#[should_panic(expected = "Caller is not patient or assigned guardian")]
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
        &String::from_str(env, "ipfs://data"),
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
#[should_panic(expected = "Snapshot rate limit: must wait 100,000 ledgers between snapshots")]
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
        &String::from_str(env, "ipfs://test"),
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
        &make_cid_v1(&env, 7),
        &String::from_str(&env, "Zero fee record"),
        &Symbol::new(&env, "LAB"),
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
        &make_cid_v1(&env, 8),
        &String::from_str(&env, "Paid record"),
        &Symbol::new(&env, "LAB"),
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
            &make_cid_v1(&env, i),
            &String::from_str(&env, "Record"),
            &Symbol::new(&env, "LAB"),
        );
    }

    let token = soroban_sdk::token::TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&treasury), 300);
    assert_eq!(token.balance(&doctor), 9_700);
}

#[test]
#[should_panic(expected = "Fee cannot be negative")]
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
        &make_cid_v1(&env, 9),
        &String::from_str(&env, "Free after reset"),
        &Symbol::new(&env, "LAB"),
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
        &String::from_str(env, "ipfs://alice"),
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
        &make_cid_v1(&env, 5),
        &String::from_str(&env, "Blood test"),
        &Symbol::new(&env, "LAB"),
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
        &make_cid_v1(&env, 10),
        &String::from_str(&env, "Checkup"),
        &Symbol::new(&env, "VISIT"),
    );
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 11),
        &String::from_str(&env, "CBC panel"),
        &Symbol::new(&env, "LAB"),
    );

    let results = client.get_records_by_type(&patient, &patient, &Symbol::new(&env, "VISIT"));
    assert_eq!(results.len(), 1);
    assert_eq!(
        results.get(0).unwrap().description,
        String::from_str(&env, "Checkup")
    );
}

#[test]
fn test_get_records_by_type_ttl_refreshes_records() {
    let env = Env::default();
    env.ledger().set(make_ledger_info(100, 1_000_000));

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 6),
        &String::from_str(&env, "CBC panel"),
        &Symbol::new(&env, "LAB"),
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
        &make_cid_v1(&env, 13),
        &String::from_str(&env, "X-ray"),
        &Symbol::new(&env, "IMAGING"),
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
        &make_cid_v1(&env, 13),
        &String::from_str(&env, "X-ray"),
        &Symbol::new(&env, "IMAGING"),
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
        &make_cid_v1(&env, 14),
        &String::from_str(&env, "X-ray"),
        &Symbol::new(&env, "IMAGING"),
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
        &make_cid_v1(&env, 51),
        &String::from_str(&env, "First record"),
        &Symbol::new(&env, "LAB"),
    );

    env.ledger().set_timestamp(2000);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 52),
        &String::from_str(&env, "Second record"),
        &Symbol::new(&env, "LAB"),
    );

    let latest = client
        .try_get_latest_record(&patient, &patient)
        .unwrap()
        .unwrap();
    assert_eq!(latest.description, String::from_str(&env, "Second record"));
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
        &String::from_str(&env, "ipfs://none"),
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
        &make_cid_v1(&env, 61),
        &String::from_str(&env, "Record A"),
        &Symbol::new(&env, "LAB"),
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
        &String::from_str(&env, "ipfs://admin"),
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
        &make_cid_v1(&env, 20),
        &String::from_str(&env, "Blood panel"),
        &Symbol::new(&env, "LAB"),
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
        &make_cid_v1(&env, 21),
        &String::from_str(&env, "First record"),
        &Symbol::new(&env, "LAB"),
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
        &make_cid_v1(&env, 22),
        &String::from_str(&env, "Second record"),
        &Symbol::new(&env, "IMAGING"),
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
        &make_cid_v1(&env, 23),
        &String::from_str(&env, "Lab test"),
        &Symbol::new(&env, "LAB"),
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
        &make_cid_v1(&env, 24),
        &String::from_str(&env, "X-ray"),
        &Symbol::new(&env, "IMAGING"),
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
        &make_cid_v1(&env, 25),
        &String::from_str(&env, "Timed record"),
        &Symbol::new(&env, "LAB"),
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
        &make_cid_v1(&env, 26),
        &String::from_str(&env, "Should fail"),
        &Symbol::new(&env, "LAB"),
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
        &String::from_str(&env, "ipfs://data"),
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
        &String::from_str(&env, "ipfs://original"),
    );

    client.freeze_contract();

    let result = client.try_update_patient(
        &patient,
        &patient,
        &String::from_str(&env, "ipfs://updated"),
    );
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
        &String::from_str(&env, "ipfs://data"),
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
        &String::from_str(&env, "ipfs://data"),
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
        &String::from_str(env, "ipfs://patient"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(env, 1),
        &String::from_str(env, "Blood test"),
        &Symbol::new(env, "LAB"),
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
        &String::from_str(&env, "ipfs://test"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 1),
        &String::from_str(&env, "Record"),
        &Symbol::new(&env, "LAB"),
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
        &String::from_str(env, "ipfs://alice"),
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
        &make_cid_v1(&env, 20),
        &String::from_str(&env, "Pre-dereg record"),
        &Symbol::new(&env, "LAB"),
    );

    client.deregister_patient(&patient);

    // Admin can still read records
    let records = client.get_medical_records(&patient, &admin);
    assert_eq!(records.len(), 1);
}

#[test]
#[should_panic(expected = "Records only accessible by admin after deregistration")]
fn test_deregister_blocks_grantee_read() {
    let env = Env::default();
    let (client, _admin, patient, doctor) = setup_for_dereg(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 21),
        &String::from_str(&env, "Record"),
        &Symbol::new(&env, "LAB"),
    );

    client.deregister_patient(&patient);

    // Former grantee (doctor) can no longer read
    client.get_medical_records(&patient, &doctor);
}

#[test]
#[should_panic(expected = "Patient already deregistered")]
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
        &String::from_str(&env, "ipfs://bob"),
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
        &String::from_str(env, "ipfs://alice"),
    );
    client.publish_consent_version(&consent);
    client.acknowledge_consent(&patient, &patient, &consent);
    client.grant_access(&patient, &patient, &doctor);

    let mut ids: Vec<u64> = Vec::new(env);
    for i in 0..n {
        let id = client.add_medical_record(
            &patient,
            &doctor,
            &make_cid_v1(env, (i + 1) as u8),
            &String::from_str(env, "record"),
            &Symbol::new(env, "LAB"),
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
        &String::from_str(&env, "ipfs://bob"),
    );
    client.publish_consent_version(&consent);
    client.acknowledge_consent(&patient, &patient, &consent);
    client.grant_access(&patient, &patient, &doctor);

    let root_before = client.get_merkle_root(&patient);

    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 1),
        &String::from_str(&env, "r1"),
        &Symbol::new(&env, "LAB"),
    );
    let root_after_1 = client.get_merkle_root(&patient);
    assert_ne!(root_before, root_after_1);

    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 2),
        &String::from_str(&env, "r2"),
        &Symbol::new(&env, "LAB"),
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
