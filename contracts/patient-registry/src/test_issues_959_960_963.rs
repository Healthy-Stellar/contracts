#![cfg(test)]

use super::*;
use provider_registry::ProviderRegistry;
use shared::privacy::{EncryptedEnvelopeRef, PolicyMetadata};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Symbol, vec,
};

fn test_encrypted_ref(env: &Env, seed: u8) -> EncryptedEnvelopeRef {
    let hash_seed = if seed == 0 { 1 } else { seed };
    EncryptedEnvelopeRef {
        content_hash: BytesN::from_array(env, &[hash_seed; 32]),
        envelope_uri: String::from_str(env, "enc+ipfs://bafyvalidpatientref"),
        key_version_id: String::from_str(env, "kv:v01"),
    }
}

fn test_policy(env: &Env) -> PolicyMetadata {
    PolicyMetadata {
        retention_class: Symbol::new(env, "clinical"),
        access_policy_hash: BytesN::from_array(env, &[200u8; 32]),
        purpose: Symbol::new(env, "treatment"),
    }
}

fn setup_test_registry<'a>(
    env: &'a Env,
) -> (
    Address,
    Address,
    Address,
    MedicalRegistryClient<'a>,
    provider_registry::ProviderRegistryClient<'a>,
) {
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    let fee_token = Address::generate(env);

    let registry_contract_id = env.register(ProviderRegistry, ());
    let registry_client = provider_registry::ProviderRegistryClient::new(env, &registry_contract_id);
    registry_client.initialize(&admin);

    env.mock_all_auths();
    client.initialize(&admin, &treasury, &fee_token);
    client.set_provider_registry(&registry_contract_id);

    (admin, treasury, fee_token, client, registry_client)
}

fn register_test_provider(
    env: &Env,
    admin: &Address,
    doctor: &Address,
    registry_client: &provider_registry::ProviderRegistryClient<'_>,
) {
    registry_client.register_provider(
        admin,
        doctor,
        &String::from_str(env, "Dr. Smith"),
        &String::from_str(env, "cardiology"),
        &String::from_str(env, "LIC-001"),
        &BytesN::from_array(env, &[1u8; 32]),
        admin,
        &BytesN::from_array(env, &[2u8; 32]),
        &(env.ledger().timestamp() + 100_000),
        &BytesN::from_array(env, &[3u8; 32]),
    );
}

#[test]
fn test_patient_can_revoke_doctor_access_while_on_hold() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (admin, _treasury, _fee_token, client, reg_client) = setup_test_registry(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let v1 = BytesN::from_array(&env, &[42u8; 32]);

    register_test_provider(&env, &admin, &doctor, &reg_client);

    client.register_patient(
        &patient,
        &String::from_str(&env, "Alice"),
        &631152000,
        &test_encrypted_ref(&env, 1),
        &test_policy(&env),
    );
    client.register_doctor(
        &doctor,
        &String::from_str(&env, "Dr Bob"),
        &String::from_str(&env, "General"),
        &soroban_sdk::Bytes::from_slice(&env, b"cert123"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor, &1u64);

    assert_eq!(client.get_authorized_doctors(&patient).len(), 1);

    // Place a legal/regulatory hold on patient
    let reason = BytesN::from_array(&env, &[99u8; 32]);
    client.place_hold(&patient, &reason, &3600u64);

    // Revocation MUST succeed even during active hold (Issue #959)
    let revoke_res = client.try_revoke_access(&patient, &patient, &doctor, &2u64);
    assert!(revoke_res.is_ok());
    assert_eq!(client.get_authorized_doctors(&patient).len(), 0);
}

#[test]
fn test_revoke_access_cleans_up_field_access_grants() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (admin, _treasury, _fee_token, client, reg_client) = setup_test_registry(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let v1 = BytesN::from_array(&env, &[42u8; 32]);

    register_test_provider(&env, &admin, &doctor, &reg_client);

    client.register_patient(
        &patient,
        &String::from_str(&env, "Alice"),
        &631152000,
        &test_encrypted_ref(&env, 1),
        &test_policy(&env),
    );
    client.register_doctor(
        &doctor,
        &String::from_str(&env, "Dr Bob"),
        &String::from_str(&env, "General"),
        &soroban_sdk::Bytes::from_slice(&env, b"cert123"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor, &1u64);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &test_encrypted_ref(&env, 2),
        &Symbol::new(&env, "LAB"),
        &test_policy(&env),
        &1u64,
    );

    // Grant field access
    let mut fields = vec![&env];
    fields.push_back(FieldPermission::RecordType);
    client.grant_field_access(&patient, &doctor, &record_id, &fields);

    // Doctor can view partial record
    let partial = client.get_record_fields(&patient, &doctor, &record_id);
    assert!(partial.record_type.is_some());

    // Revoke access
    client.revoke_access(&patient, &patient, &doctor, &2u64);

    // After revocation, field access is wiped
    let partial_after = client.get_record_fields(&patient, &doctor, &record_id);
    assert!(partial_after.record_type.is_none());
}

#[test]
fn test_share_link_recipient_bound_and_revocation() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (admin, _treasury, _fee_token, client, reg_client) = setup_test_registry(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let impostor = Address::generate(&env);
    let v1 = BytesN::from_array(&env, &[42u8; 32]);

    register_test_provider(&env, &admin, &doctor, &reg_client);

    client.register_patient(
        &patient,
        &String::from_str(&env, "Alice"),
        &631152000,
        &test_encrypted_ref(&env, 1),
        &test_policy(&env),
    );
    client.register_doctor(
        &doctor,
        &String::from_str(&env, "Dr Bob"),
        &String::from_str(&env, "General"),
        &soroban_sdk::Bytes::from_slice(&env, b"cert123"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor, &1u64);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &test_encrypted_ref(&env, 2),
        &Symbol::new(&env, "LAB"),
        &test_policy(&env),
        &1u64,
    );

    // Create share link bound to recipient (Issue #960)
    let token = client.create_share_link(&patient, &recipient, &record_id, &2u32, &5000u64);

    // Impostor trying to redeem returns NotAuthorized
    let impostor_res = client.try_use_share_link(&impostor, &token);
    assert!(matches!(impostor_res, Err(Ok(ContractError::NotAuthorized))));

    // Authorized recipient redeems successfully
    let record = client.use_share_link(&recipient, &token);
    assert_eq!(record.record_id, record_id);

    // Patient revokes the share link
    let revoke_res = client.try_revoke_share_link(&patient, &token);
    assert!(revoke_res.is_ok());

    // Subsequent use by recipient fails because token is revoked
    let use_after_revoke = client.try_use_share_link(&recipient, &token);
    assert!(matches!(use_after_revoke, Err(Ok(ContractError::InvalidToken))));
}

#[test]
fn test_deleted_record_not_shareable_and_deregister_purges() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (admin, _treasury, _fee_token, client, reg_client) = setup_test_registry(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let v1 = BytesN::from_array(&env, &[42u8; 32]);

    register_test_provider(&env, &admin, &doctor, &reg_client);

    client.register_patient(
        &patient,
        &String::from_str(&env, "Alice"),
        &631152000,
        &test_encrypted_ref(&env, 1),
        &test_policy(&env),
    );
    client.register_doctor(
        &doctor,
        &String::from_str(&env, "Dr Bob"),
        &String::from_str(&env, "General"),
        &soroban_sdk::Bytes::from_slice(&env, b"cert123"),
    );
    client.publish_consent_version(&v1);
    client.acknowledge_consent(&patient, &patient, &v1);
    client.grant_access(&patient, &patient, &doctor, &1u64);

    let record_id = client.add_medical_record(
        &patient,
        &doctor,
        &test_encrypted_ref(&env, 2),
        &Symbol::new(&env, "LAB"),
        &test_policy(&env),
        &1u64,
    );

    // Create share link before deletion
    let token = client.create_share_link(&patient, &recipient, &record_id, &5u32, &5000u64);

    // Soft delete record
    client.soft_delete_record(&record_id, &patient);

    // Using existing share link for deleted record returns AlreadyDeleted
    let use_res = client.try_use_share_link(&recipient, &token);
    assert!(matches!(use_res, Err(Ok(ContractError::AlreadyDeleted))));

    // Creating new share link for deleted record returns AlreadyDeleted
    let create_res = client.try_create_share_link(&patient, &recipient, &record_id, &1u32, &5000u64);
    assert!(matches!(create_res, Err(Ok(ContractError::AlreadyDeleted))));
}

#[test]
fn test_paginated_patients_and_doctors_and_snapshot() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);

    let (_admin, _treasury, _fee_token, client, _reg_client) = setup_test_registry(&env);

    // Register 3 patients
    let mut patients = vec![&env];
    for i in 1..=3 {
        let p = Address::generate(&env);
        client.register_patient(
            &p,
            &String::from_str(&env, "Patient"),
            &631152000,
            &test_encrypted_ref(&env, i),
            &test_policy(&env),
        );
        patients.push_back(p);
    }

    // Register 3 doctors
    let mut doctors = vec![&env];
    for _ in 1..=3 {
        let d = Address::generate(&env);
        client.register_doctor(
            &d,
            &String::from_str(&env, "Doctor"),
            &String::from_str(&env, "Specialty"),
            &soroban_sdk::Bytes::from_slice(&env, b"cert"),
        );
        doctors.push_back(d);
    }

    // Test paginated retrieval (Issue #963)
    let page1_patients = client.get_patients_paginated(&0u64, &2u64);
    assert_eq!(page1_patients.len(), 2);
    assert_eq!(page1_patients.get(0).unwrap(), patients.get(0).unwrap());
    assert_eq!(page1_patients.get(1).unwrap(), patients.get(1).unwrap());

    let page2_patients = client.get_patients_paginated(&2u64, &2u64);
    assert_eq!(page2_patients.len(), 1);
    assert_eq!(page2_patients.get(0).unwrap(), patients.get(2).unwrap());

    let all_doctors = client.get_doctors_paginated(&0u64, &10u64);
    assert_eq!(all_doctors.len(), 3);

    // Emit state snapshot without loading massive Vec into memory
    let snap_res = client.try_emit_state_snapshot();
    assert!(snap_res.is_ok());
}
