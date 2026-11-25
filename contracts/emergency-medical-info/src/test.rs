#![cfg(test)]
#![allow(deprecated)]

use super::*;
use soroban_sdk::{testutils::Address as _, BytesN, Env, Symbol, Vec};

fn hash(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn create_test_emergency_contacts(env: &Env) -> Vec<EmergencyContact> {
    let mut contacts = Vec::new(env);

    contacts.push_back(EmergencyContact {
        contact_label_hash: hash(env, 11),
        relationship_class: Symbol::new(env, "spouse"),
        contact_hash: BytesN::from_array(env, &[1u8; 32]),
        priority: 1,
    });

    contacts.push_back(EmergencyContact {
        contact_label_hash: hash(env, 12),
        relationship_class: Symbol::new(env, "parent"),
        contact_hash: BytesN::from_array(env, &[2u8; 32]),
        priority: 2,
    });

    contacts
}

#[test]
fn test_set_emergency_profile() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    env.mock_all_auths();

    let blood_type = Symbol::new(&env, "O_POS");
    let mut allergies = Vec::new(&env);
    allergies.push_back(hash(&env, 21));

    let mut conditions = Vec::new(&env);
    conditions.push_back(hash(&env, 31));
    conditions.push_back(hash(&env, 31));

    let mut medications = Vec::new(&env);
    medications.push_back(hash(&env, 41));
    medications.push_back(hash(&env, 41));

    let contacts = create_test_emergency_contacts(&env);

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &None,
    );

    // Verify profile was created
    assert!(client.has_emergency_profile(&patient, &patient));

    let profile = client.get_emergency_info(&patient, &patient);
    assert_eq!(profile.blood_type, blood_type);
    assert_eq!(profile.active_condition_hashes.len(), 2);
    assert_eq!(profile.current_medication_hashes.len(), 2);
    assert_eq!(profile.emergency_contacts.len(), 2);
    assert!(!profile.dnr_status);
}

#[test]
fn test_emergency_access_request() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.register_responder(&admin, &provider);

    // Setup emergency profile
    let blood_type = Symbol::new(&env, "AB_NEG");
    let mut allergies = Vec::new(&env);
    allergies.push_back(hash(&env, 21));
    let conditions = Vec::new(&env);
    let medications = Vec::new(&env);
    let contacts = create_test_emergency_contacts(&env);

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &None,
    );

    // Emergency access request
    let emergency_type = Symbol::new(&env, "CARDIAC");
    let justification = hash(&env, 51);
    let location = hash(&env, 52);

    let profile = client.emergency_access_request(
        &provider,
        &patient,
        &emergency_type,
        &justification,
        &location,
    );

    assert_eq!(profile.blood_type, blood_type);
    assert_eq!(profile.emergency_contacts.len(), 2);

    // Verify access was logged
    let logs = client.get_emergency_access_logs(&patient, &patient);
    assert_eq!(logs.len(), 1);
    assert_eq!(logs.get(0).unwrap().provider_id, provider);
    assert_eq!(logs.get(0).unwrap().emergency_type, emergency_type);
}

#[test]
fn test_emergency_access_request_rejects_unregistered_responder() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let unregistered_caller = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);

    // Setup emergency profile
    let blood_type = Symbol::new(&env, "AB_NEG");
    let mut allergies = Vec::new(&env);
    allergies.push_back(hash(&env, 21));
    let conditions = Vec::new(&env);
    let medications = Vec::new(&env);
    let contacts = create_test_emergency_contacts(&env);

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &None,
    );

    // A caller who was never registered as an emergency responder -- merely
    // controlling an address is not proof of the emergency-responder role.
    let result = client.try_emergency_access_request(
        &unregistered_caller,
        &patient,
        &Symbol::new(&env, "CARDIAC"),
        &hash(&env, 51),
        &hash(&env, 52),
    );
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));

    // No access log should have been written for the rejected attempt.
    let logs = client.get_emergency_access_logs(&patient, &patient);
    assert_eq!(logs.len(), 0);
}

#[test]
fn test_add_critical_alert() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    let alert_type = Symbol::new(&env, "ALLERGY");
    let alert_text = hash(&env, 61);
    let severity = Symbol::new(&env, "CRITICAL");

    client.initialize(&admin);
    client.register_responder(&admin, &provider);

    client.add_critical_alert(&patient, &provider, &alert_type, &alert_text, &severity);

    let alerts = client.get_critical_alerts(&patient, &patient);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts.get(0).unwrap().alert_type, alert_type);
    assert_eq!(alerts.get(0).unwrap().severity, severity);
}

#[test]
fn test_add_critical_alert_rejects_unregistered_responder() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let unregistered_caller = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);

    // An address that merely signs the call but was never registered as an
    // emergency responder must not be able to fabricate a critical alert.
    let result = client.try_add_critical_alert(
        &patient,
        &unregistered_caller,
        &Symbol::new(&env, "ALLERGY"),
        &hash(&env, 61),
        &Symbol::new(&env, "CRITICAL"),
    );
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));

    let alerts = client.get_critical_alerts(&patient, &patient);
    assert_eq!(alerts.len(), 0);
}

#[test]
fn test_multiple_critical_alerts() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.register_responder(&admin, &provider);

    // Add multiple alerts
    client.add_critical_alert(
        &patient,
        &provider,
        &Symbol::new(&env, "ALLERGY"),
        &hash(&env, 51),
        &Symbol::new(&env, "HIGH"),
    );

    client.add_critical_alert(
        &patient,
        &provider,
        &Symbol::new(&env, "CONDITION"),
        &hash(&env, 51),
        &Symbol::new(&env, "CRITICAL"),
    );

    let alerts = client.get_critical_alerts(&patient, &patient);
    assert_eq!(alerts.len(), 2);
}

#[test]
fn test_add_critical_alert_caps_at_max() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.register_responder(&admin, &provider);

    let alert_type = Symbol::new(&env, "ALLERGY");
    let severity = Symbol::new(&env, "CRITICAL");

    // Fill the alert list up to the cap.
    for i in 0..MAX_CRITICAL_ALERTS {
        client.add_critical_alert(
            &patient,
            &provider,
            &alert_type,
            &hash(&env, (i % 255) as u8),
            &severity,
        );
    }

    let alerts = client.get_critical_alerts(&patient, &patient);
    assert_eq!(alerts.len(), MAX_CRITICAL_ALERTS);

    // One more alert must not brick the alerting path: it either evicts the
    // oldest entry or is rejected with TooManyAlerts, but the stored list must
    // never exceed the cap.
    let result = client.try_add_critical_alert(
        &patient,
        &provider,
        &alert_type,
        &hash(&env, 200),
        &severity,
    );
    match result {
        Ok(_) => {
            let alerts = client.get_critical_alerts(&patient, &patient);
            assert_eq!(alerts.len(), MAX_CRITICAL_ALERTS);
        }
        Err(Ok(Error::TooManyAlerts)) => {
            let alerts = client.get_critical_alerts(&patient, &patient);
            assert_eq!(alerts.len(), MAX_CRITICAL_ALERTS);
        }
        other => panic!("unexpected result: {:?}", other),
    }
}

#[test]
fn test_get_critical_alerts_rejects_unauthorized_caller() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let stranger = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.register_responder(&admin, &provider);

    client.add_critical_alert(
        &patient,
        &provider,
        &Symbol::new(&env, "ALLERGY"),
        &hash(&env, 61),
        &Symbol::new(&env, "CRITICAL"),
    );

    // A caller with no relationship to the patient must not read PHI metadata.
    let result = client.try_get_critical_alerts(&patient, &stranger);
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));

    // The patient themselves may read their own alerts.
    let alerts = client.get_critical_alerts(&patient, &patient);
    assert_eq!(alerts.len(), 1);

    // A registered responder may read the alerts.
    let alerts = client.get_critical_alerts(&patient, &provider);
    assert_eq!(alerts.len(), 1);
}

#[test]
fn test_get_dnr_order_rejects_unauthorized_caller() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let stranger = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.register_responder(&admin, &provider);

    let blood_type = Symbol::new(&env, "O_POS");
    let allergies = Vec::new(&env);
    let conditions = Vec::new(&env);
    let medications = Vec::new(&env);
    let contacts = create_test_emergency_contacts(&env);

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &None,
    );

    // A stranger must not be able to enumerate DNR status.
    let result = client.try_get_dnr_order(&patient, &stranger);
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));

    // The patient may read their own DNR order.
    let _ = client.get_dnr_order(&patient, &patient);
}

#[test]
fn test_has_emergency_profile_rejects_unauthorized_caller() {
    let env = Env::default();
    let contract_id = env.register(EmergencyMedicalInfo, ());
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let stranger = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize(&admin);
    client.register_responder(&admin, &provider);

    let blood_type = Symbol::new(&env, "O_POS");
    let allergies = Vec::new(&env);
    let conditions = Vec::new(&env);
    let medications = Vec::new(&env);
    let contacts = create_test_emergency_contacts(&env);

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &None,
    );

    // A stranger must not be able to probe for profile existence.
    let result = client.try_has_emergency_profile(&patient, &stranger);
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));

    // The patient and a registered responder may check.
    assert!(client.has_emergency_profile(&patient, &patient));
    assert!(client.has_emergency_profile(&patient, &provider));
}
