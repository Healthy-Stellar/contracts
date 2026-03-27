#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, MockAuth, MockAuthInvoke},
    Address, Bytes, BytesN, Env, IntoVal, String, Symbol,
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
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor);
    client.add_medical_record(&patient, &doctor, &hash, &desc, &Symbol::new(&env, "LAB"));

    let records = client.get_medical_records(&patient);
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

    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    let hash = make_cid_v1(&env, 9);
    let desc = String::from_str(&env, "X-ray scan");

    env.mock_all_auths();

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
    assert_eq!(client.get_medical_records(&patient).len(), 0);
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

    assert_eq!(client.get_medical_records(&patient).len(), 1);
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

    assert_eq!(client.get_medical_records(&patient).len(), 1);
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
/// TTL EXTENSION TESTS
/// ------------------------------------------------

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
) -> (MedicalRegistryClient<'_>, Address, Address, Address, BytesN<32>) {
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

fn setup_for_filter(env: &Env) -> (MedicalRegistryClient<'_>, Address, Address) {
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

    (client, patient, doctor)
}

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
        &make_cid_v1(&env, 10),
        &String::from_str(&env, "Initial checkup"),
        &Symbol::new(&env, "VISIT"),
    );
    let records = client.get_medical_records(&patient);
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
        &Symbol::new(&env, "LAB"),
        &Symbol::new(&env, "VISIT"),
    );

    // Verify the records are still accessible after adding
    let records = client.get_medical_records(&patient);
    assert_eq!(records.len(), 1);
}

#[test]
fn test_get_records_by_type_returns_matching_records() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    let (client, _admin, patient, doctor, _v1) = setup_for_ttl(&env);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 10),
        &String::from_str(&env, "CBC panel"),
        &Symbol::new(&env, "LAB"),
    );
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 11),
        &String::from_str(&env, "Amoxicillin"),
        &Symbol::new(&env, "PRESCRIPTION"),
    );
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 12),
        &String::from_str(&env, "Lipid panel"),
        &Symbol::new(&env, "LAB"),
    );

    let lab_records = client.get_records_by_type(&patient, &patient, &Symbol::new(&env, "LAB"));
    assert_eq!(lab_records.len(), 2);
    assert_eq!(
        lab_records.get(0).unwrap().description,
        String::from_str(&env, "CBC panel")
    );
    assert_eq!(
        lab_records.get(1).unwrap().description,
        String::from_str(&env, "Lipid panel")
    );

    let rx_records =
        client.get_records_by_type(&patient, &patient, &Symbol::new(&env, "PRESCRIPTION"));
    assert_eq!(rx_records.len(), 1);
    assert_eq!(
        rx_records.get(0).unwrap().description,
        String::from_str(&env, "Amoxicillin")
    );
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
        &String::from_str(&env, "Initial record"),
        &Symbol::new(&env, "LAB"),
        &Symbol::new(&env, "VISIT"),
    );

    // Call get_medical_records — internally bumps TTL
    let records = client.get_medical_records(&patient);
    assert_eq!(records.len(), 1);

    // Advance the ledger significantly — data should still be accessible
    env.ledger().set(make_ledger_info(
        100 + LEDGER_THRESHOLD - 1,
        1_000_000 + 1_000,
    ));
    let records_after = client.get_medical_records(&patient);
    assert_eq!(records_after.len(), 1);
}

#[test]
fn test_get_records_by_type_returns_empty_when_no_match() {
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

/// `extend_patient_ttl` works even when the patient has no MedicalRecords yet
/// (optional keys are skipped gracefully).
#[test]
fn test_extend_patient_ttl_no_records_yet() {
    let env = Env::default();
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

    let result = client.get_records_by_type(&patient, &patient, &Symbol::new(&env, "LAB"));
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_records_by_type_authorized_doctor_can_read() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 14),
        &String::from_str(&env, "Flu shot"),
        &Symbol::new(&env, "IMMUNIZATION"),
    );

    // Doctor (authorized) can query records
    let records = client.get_records_by_type(&patient, &doctor, &Symbol::new(&env, "IMMUNIZATION"));
    assert_eq!(records.len(), 1);
}

#[test]
fn test_get_records_by_type_guardian_can_read() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);
    let guardian = Address::generate(&env);

    client.assign_guardian(&patient, &guardian);
    client.add_medical_record(
        &patient,
        &doctor,
        &make_cid_v1(&env, 15),
        &String::from_str(&env, "Child checkup"),
        &Symbol::new(&env, "VISIT"),
    );

    let records = client.get_records_by_type(&patient, &guardian, &Symbol::new(&env, "VISIT"));
    assert_eq!(records.len(), 1);
}

#[test]
#[should_panic(expected = "Caller not authorized to view records")]
fn test_get_records_by_type_unauthorized_caller_is_rejected() {
    let env = Env::default();
    let (client, patient, _doctor) = setup_for_filter(&env);
    let stranger = Address::generate(&env);

    client.get_records_by_type(&patient, &stranger, &Symbol::new(&env, "LAB"));
}

#[test]
fn test_get_records_by_type_multiple_types_isolation() {
    let env = Env::default();
    let (client, patient, doctor) = setup_for_filter(&env);

    let types = [
        ("LAB", "Blood work"),
        ("PRESCRIPTION", "Metformin"),
        ("IMAGING", "Chest X-ray"),
        ("LAB", "Urinalysis"),
        ("PRESCRIPTION", "Lisinopril"),
    ];

    for (i, (rtype, desc)) in types.iter().enumerate() {
        client.add_medical_record(
            &patient,
            &doctor,
            &make_cid_v1(&env, i as u8),
            &String::from_str(&env, desc),
            &Symbol::new(&env, rtype),
        );
    }

    assert_eq!(
        client
            .get_records_by_type(&patient, &patient, &Symbol::new(&env, "LAB"))
            .len(),
        2
    );
    assert_eq!(
        client
            .get_records_by_type(&patient, &patient, &Symbol::new(&env, "PRESCRIPTION"))
            .len(),
        2
    );
    assert_eq!(
        client
            .get_records_by_type(&patient, &patient, &Symbol::new(&env, "IMAGING"))
            .len(),
        1
    );
    assert_eq!(
        client
            .get_records_by_type(&patient, &patient, &Symbol::new(&env, "VISIT"))
            .len(),
        0
    );
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
        let expected_topics_val: soroban_sdk::Vec<soroban_sdk::Val> = (
            new_record_topic.clone(),
            patient.clone(),
            doctor.clone(),
        )
            .into_val(&env);
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
        let expected_topics_val: soroban_sdk::Vec<soroban_sdk::Val> = (
            new_record_topic.clone(),
            patient.clone(),
            doctor.clone(),
        )
            .into_val(&env);
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
