#![cfg(test)]
#![allow(deprecated)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

fn create_test_emergency_contacts(env: &Env) -> Vec<EmergencyContact> {
    let mut contacts = Vec::new(env);

    contacts.push_back(EmergencyContact {
        name: String::from_str(env, "Jane Doe"),
        relationship: String::from_str(env, "Spouse"),
        contact_hash: BytesN::from_array(env, &[1u8; 32]),
        priority: 1,
    });

    contacts.push_back(EmergencyContact {
        name: String::from_str(env, "John Doe Sr"),
        relationship: String::from_str(env, "Parent"),
        contact_hash: BytesN::from_array(env, &[2u8; 32]),
        priority: 2,
    });

    contacts
}

#[test]
fn test_set_emergency_profile() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    env.mock_all_auths();

    let blood_type = Symbol::new(&env, "O_POS");
    let allergies = String::from_str(&env, "Penicillin, Latex");

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes Type 2"));
    conditions.push_back(String::from_str(&env, "Hypertension"));

    let mut medications = Vec::new(&env);
    medications.push_back(String::from_str(&env, "Metformin 500mg"));
    medications.push_back(String::from_str(&env, "Lisinopril 10mg"));

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
    assert!(client.has_emergency_profile(&patient));

    let profile = client.get_emergency_info(&patient, &patient);
    assert_eq!(profile.blood_type, blood_type);
    assert_eq!(profile.active_conditions.len(), 2);
    assert_eq!(profile.current_medications.len(), 2);
    assert_eq!(profile.emergency_contacts.len(), 2);
    assert!(!profile.dnr_status);
}

#[test]
fn test_emergency_access_request() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    env.mock_all_auths();

    // Setup emergency profile
    let blood_type = Symbol::new(&env, "AB_NEG");
    let allergies = String::from_str(&env, "Shellfish");
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
    let justification = String::from_str(&env, "Patient unconscious, cardiac arrest");
    let location = String::from_str(&env, "ER Room 3");

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
    let logs = client.get_emergency_access_logs(&patient);
    assert_eq!(logs.len(), 1);
    assert_eq!(logs.get(0).unwrap().provider_id, provider);
    assert_eq!(logs.get(0).unwrap().emergency_type, emergency_type);
}

#[test]
fn test_add_critical_alert() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    env.mock_all_auths();

    let alert_type = Symbol::new(&env, "ALLERGY");
    let alert_text = String::from_str(&env, "Severe reaction to contrast dye");
    let severity = Symbol::new(&env, "CRITICAL");

    client.add_critical_alert(&patient, &provider, &alert_type, &alert_text, &severity);

    let alerts = client.get_critical_alerts(&patient);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts.get(0).unwrap().alert_type, alert_type);
    assert_eq!(alerts.get(0).unwrap().severity, severity);
}

#[test]
fn test_multiple_critical_alerts() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    env.mock_all_auths();

    // Add multiple alerts
    client.add_critical_alert(
        &patient,
        &provider,
        &Symbol::new(&env, "ALLERGY"),
        &String::from_str(&env, "Penicillin allergy"),
        &Symbol::new(&env, "HIGH"),
    );

    client.add_critical_alert(
        &patient,
        &provider,
        &Symbol::new(&env, "CONDITION"),
        &String::from_str(&env, "Hemophilia"),
        &Symbol::new(&env, "CRITICAL"),
    );

    let alerts = client.get_critical_alerts(&patient);
    assert_eq!(alerts.len(), 2);
}

#[test]
fn test_notify_emergency_contacts() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    env.mock_all_auths();

    // Setup profile with contacts
    let blood_type = Symbol::new(&env, "A_POS");
    let allergies = String::from_str(&env, "None");
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

    // Notify contacts
    let emergency_type = Symbol::new(&env, "TRAUMA");
    let notification_time = env.ledger().timestamp();

    let notified_contacts =
        client.notify_emergency_contacts(&patient, &emergency_type, &notification_time);

    assert_eq!(notified_contacts.len(), 2);
    assert_eq!(notified_contacts.get(0).unwrap().priority, 1);
}

#[test]
fn test_record_dnr_order() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    env.mock_all_auths();

    // Setup profile first
    let blood_type = Symbol::new(&env, "B_POS");
    let allergies = String::from_str(&env, "None");
    let conditions = Vec::new(&env);
    let medications = Vec::new(&env);
    let contacts = Vec::new(&env);

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &None,
    );

    // Record DNR
    let dnr_hash = BytesN::from_array(&env, &[42u8; 32]);
    let effective_date = env.ledger().timestamp();

    client.record_dnr_order(&patient, &provider, &dnr_hash, &effective_date);

    // Verify DNR was recorded
    let dnr = client.get_dnr_order(&patient);
    assert!(dnr.is_some());
    assert_eq!(dnr.unwrap().dnr_document_hash, dnr_hash);

    // Verify profile DNR status updated
    let profile = client.get_emergency_info(&patient, &patient);
    assert!(profile.dnr_status);
}

#[test]
fn test_dnr_with_advance_directives() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    env.mock_all_auths();

    let blood_type = Symbol::new(&env, "O_NEG");
    let allergies = String::from_str(&env, "None");
    let conditions = Vec::new(&env);
    let medications = Vec::new(&env);
    let contacts = Vec::new(&env);
    let advance_directives = Some(BytesN::from_array(&env, &[99u8; 32]));

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &advance_directives,
    );

    // Verify DNR was created with advance directives
    let dnr = client.get_dnr_order(&patient);
    assert!(dnr.is_some());
}

#[test]
#[should_panic(expected = "Emergency profile not found")]
fn test_emergency_access_without_profile() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    env.mock_all_auths();

    // Try to access without profile
    client.emergency_access_request(
        &provider,
        &patient,
        &Symbol::new(&env, "TRAUMA"),
        &String::from_str(&env, "Emergency"),
        &String::from_str(&env, "ER"),
    );
}

#[test]
fn test_emergency_access_audit_trail() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider1 = Address::generate(&env);
    let provider2 = Address::generate(&env);
    env.mock_all_auths();

    // Setup profile
    let blood_type = Symbol::new(&env, "A_NEG");
    let allergies = String::from_str(&env, "Aspirin");
    let conditions = Vec::new(&env);
    let medications = Vec::new(&env);
    let contacts = Vec::new(&env);

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &None,
    );

    // Multiple emergency accesses
    client.emergency_access_request(
        &provider1,
        &patient,
        &Symbol::new(&env, "TRAUMA"),
        &String::from_str(&env, "MVA victim"),
        &String::from_str(&env, "Trauma Bay 1"),
    );

    client.emergency_access_request(
        &provider2,
        &patient,
        &Symbol::new(&env, "CONSULT"),
        &String::from_str(&env, "Specialist consult"),
        &String::from_str(&env, "ICU"),
    );

    // Verify audit trail
    let logs = client.get_emergency_access_logs(&patient);
    assert_eq!(logs.len(), 2);
    assert_eq!(logs.get(0).unwrap().provider_id, provider1);
    assert_eq!(logs.get(1).unwrap().provider_id, provider2);
}

#[test]
fn test_has_emergency_profile() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    env.mock_all_auths();

    // Initially no profile
    assert!(!client.has_emergency_profile(&patient));

    // Create profile
    let blood_type = Symbol::new(&env, "O_POS");
    let allergies = String::from_str(&env, "None");
    let conditions = Vec::new(&env);
    let medications = Vec::new(&env);
    let contacts = Vec::new(&env);

    client.set_emergency_profile(
        &patient,
        &blood_type,
        &allergies,
        &conditions,
        &medications,
        &contacts,
        &None,
    );

    // Now has profile
    assert!(client.has_emergency_profile(&patient));
}

#[test]
fn test_comprehensive_emergency_scenario() {
    let env = Env::default();
    let contract_id = env.register_contract(None, EmergencyMedicalInfo);
    let client = EmergencyMedicalInfoClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    env.mock_all_auths();

    // 1. Setup comprehensive emergency profile
    let blood_type = Symbol::new(&env, "AB_POS");
    let allergies = String::from_str(&env, "Penicillin, Sulfa drugs");

    let mut conditions = Vec::new(&env);
    conditions.push_back(String::from_str(&env, "Diabetes Type 1"));
    conditions.push_back(String::from_str(&env, "Asthma"));

    let mut medications = Vec::new(&env);
    medications.push_back(String::from_str(&env, "Insulin pump"));
    medications.push_back(String::from_str(&env, "Albuterol inhaler"));

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

    // 2. Add critical alerts
    client.add_critical_alert(
        &patient,
        &provider,
        &Symbol::new(&env, "ALLERGY"),
        &String::from_str(&env, "Anaphylaxis risk"),
        &Symbol::new(&env, "CRITICAL"),
    );

    // 3. Emergency access
    let profile = client.emergency_access_request(
        &provider,
        &patient,
        &Symbol::new(&env, "RESP_DIST"),
        &String::from_str(&env, "Severe asthma attack"),
        &String::from_str(&env, "ER"),
    );

    // 4. Notify contacts
    let notified = client.notify_emergency_contacts(
        &patient,
        &Symbol::new(&env, "RESP_DIST"),
        &env.ledger().timestamp(),
    );

    // Verify complete scenario
    assert_eq!(profile.blood_type, blood_type);
    assert_eq!(profile.active_conditions.len(), 2);
    assert_eq!(notified.len(), 2);

    let alerts = client.get_critical_alerts(&patient);
    assert_eq!(alerts.len(), 1);

    let logs = client.get_emergency_access_logs(&patient);
    assert_eq!(logs.len(), 1);
}
