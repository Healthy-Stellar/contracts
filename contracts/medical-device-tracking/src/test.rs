#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec};

fn dummy_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

fn register_device_helper(env: &Env, client: &MedicalDeviceRegistryClient) -> u64 {
    client.register_device(
        &String::from_str(env, "UDI-12345-ABC"),
        &Symbol::new(env, "IMPLANT"),
        &String::from_str(env, "MedCorp"),
        &String::from_str(env, "MC-100"),
        &String::from_str(env, "LOT-001"),
        &1690000000u64,
        &None,
        &dummy_hash(env),
    )
}

#[test]
fn test_register_device() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let id = register_device_helper(&env, &client);
    assert_eq!(id, 1);

    let id2 = register_device_helper(&env, &client);
    assert_eq!(id2, 2);
}

#[test]
fn test_register_device_with_expiration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let id = client.register_device(
        &String::from_str(&env, "UDI-99999-XYZ"),
        &Symbol::new(&env, "DME"),
        &String::from_str(&env, "DeviceMaker"),
        &String::from_str(&env, "DM-500"),
        &String::from_str(&env, "LOT-999"),
        &1690000000u64,
        &Some(1800000000u64),
        &dummy_hash(&env),
    );
    assert_eq!(id, 1);
}

#[test]
fn test_implant_device() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);

    let implant_id = client.implant_device(
        &patient_id,
        &device_id,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Left Hip"),
        &dummy_hash(&env),
    );
    assert_eq!(implant_id, 1);

    let requester = Address::generate(&env);
    let implants = client.get_patient_implants(&patient_id, &requester, &true);
    assert_eq!(implants.len(), 1);

    let record = implants.get(0).unwrap();
    assert_eq!(record.patient_id, patient_id);
    assert_eq!(record.device_id, device_id);
    assert!(record.is_active);
    assert!(record.removal_date.is_none());
}

#[test]
fn test_implant_device_nonexistent_returns_error() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let res = client.try_implant_device(
        &patient_id,
        &999u64,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Left Hip"),
        &dummy_hash(&env),
    );
    assert!(res.is_err());
}

#[test]
fn test_prescribe_dme() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let device_id = register_device_helper(&env, &client);

    let rx_id = client.prescribe_dme(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "WHEELCHAIR"),
        &device_id,
        &1700000000u64,
        &Some(90u64),
        &dummy_hash(&env),
    );
    assert_eq!(rx_id, 1);

    let rx_id2 = client.prescribe_dme(
        &patient_id,
        &provider_id,
        &Symbol::new(&env, "CRUTCHES"),
        &device_id,
        &1700100000u64,
        &None,
        &dummy_hash(&env),
    );
    assert_eq!(rx_id2, 2);
}

#[test]
fn test_record_device_maintenance() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let technician = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);
    let implant_id = client.implant_device(
        &patient_id,
        &device_id,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Lumbar Spine"),
        &dummy_hash(&env),
    );

    client.record_device_maintenance(
        &implant_id,
        &1710000000u64,
        &Symbol::new(&env, "CALIBRATION"),
        &technician,
        &dummy_hash(&env),
    );
    client.record_device_maintenance(
        &implant_id,
        &1720000000u64,
        &Symbol::new(&env, "INSPECTION"),
        &technician,
        &dummy_hash(&env),
    );

    let requester = Address::generate(&env);
    let implants = client.get_patient_implants(&patient_id, &requester, &true);
    let record = implants.get(0).unwrap();
    assert_eq!(record.maintenance_history.len(), 2);
}

#[test]
fn test_record_device_maintenance_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let technician = Address::generate(&env);
    let res = client.try_record_device_maintenance(
        &999u64,
        &1710000000u64,
        &Symbol::new(&env, "CALIBRATION"),
        &technician,
        &dummy_hash(&env),
    );
    assert!(res.is_err());
}

#[test]
fn test_issue_device_recall() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let manufacturer = Address::generate(&env);
    let device_id = register_device_helper(&env, &client);

    let mut device_ids: Vec<u64> = Vec::new(&env);
    device_ids.push_back(device_id);

    let recall_id = client.issue_device_recall(
        &manufacturer,
        &device_ids,
        &String::from_str(&env, "Battery failure risk"),
        &Symbol::new(&env, "CRITICAL"),
        &1750000000u64,
        &String::from_str(&env, "Immediate explantation required"),
    );
    assert_eq!(recall_id, 1);
}

#[test]
fn test_notify_affected_patients() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient1 = Address::generate(&env);
    let patient2 = Address::generate(&env);
    let provider = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);

    client.implant_device(
        &patient1,
        &device_id,
        &provider,
        &1700000000u64,
        &String::from_str(&env, "Left Knee"),
        &dummy_hash(&env),
    );
    client.implant_device(
        &patient2,
        &device_id,
        &provider,
        &1700100000u64,
        &String::from_str(&env, "Right Knee"),
        &dummy_hash(&env),
    );

    let mut device_ids: Vec<u64> = Vec::new(&env);
    device_ids.push_back(device_id);

    let recall_id = client.issue_device_recall(
        &manufacturer,
        &device_ids,
        &String::from_str(&env, "Stress fracture risk"),
        &Symbol::new(&env, "HIGH"),
        &1750000000u64,
        &String::from_str(&env, "Monitoring required"),
    );

    let affected = client.notify_affected_patients(&recall_id, &1750100000u64);
    assert_eq!(affected.len(), 2);
    assert!(affected.contains(&patient1));
    assert!(affected.contains(&patient2));
}

#[test]
fn test_notify_affected_patients_excludes_removed() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient1 = Address::generate(&env);
    let patient2 = Address::generate(&env);
    let provider = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);

    let implant_id1 = client.implant_device(
        &patient1,
        &device_id,
        &provider,
        &1700000000u64,
        &String::from_str(&env, "Chest"),
        &dummy_hash(&env),
    );
    client.implant_device(
        &patient2,
        &device_id,
        &provider,
        &1700100000u64,
        &String::from_str(&env, "Abdomen"),
        &dummy_hash(&env),
    );

    // patient1 has device removed before recall
    client.remove_implant(
        &implant_id1,
        &provider,
        &1740000000u64,
        &String::from_str(&env, "Elective removal"),
        &None,
    );

    let mut device_ids: Vec<u64> = Vec::new(&env);
    device_ids.push_back(device_id);

    let recall_id = client.issue_device_recall(
        &manufacturer,
        &device_ids,
        &String::from_str(&env, "Software defect"),
        &Symbol::new(&env, "MODERATE"),
        &1750000000u64,
        &String::from_str(&env, "Software update required"),
    );

    let affected = client.notify_affected_patients(&recall_id, &1750100000u64);
    assert_eq!(affected.len(), 1);
    assert!(affected.contains(&patient2));
}

#[test]
fn test_notify_affected_patients_recall_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let res = client.try_notify_affected_patients(&999u64, &1750100000u64);
    assert!(res.is_err());
}

#[test]
fn test_remove_implant() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);
    let implant_id = client.implant_device(
        &patient_id,
        &device_id,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Right Hip"),
        &dummy_hash(&env),
    );

    client.remove_implant(
        &implant_id,
        &provider_id,
        &1750000000u64,
        &String::from_str(&env, "Device malfunction"),
        &Some(dummy_hash(&env)),
    );

    let requester = Address::generate(&env);
    let active_implants = client.get_patient_implants(&patient_id, &requester, &true);
    assert_eq!(active_implants.len(), 0);

    let all_implants = client.get_patient_implants(&patient_id, &requester, &false);
    assert_eq!(all_implants.len(), 1);
    let record = all_implants.get(0).unwrap();
    assert!(!record.is_active);
    assert_eq!(record.removal_date, Some(1750000000u64));
}

#[test]
fn test_remove_implant_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let res = client.try_remove_implant(
        &999u64,
        &provider_id,
        &1750000000u64,
        &String::from_str(&env, "Unknown"),
        &None,
    );
    assert!(res.is_err());
}

#[test]
fn test_remove_implant_already_inactive() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);
    let implant_id = client.implant_device(
        &patient_id,
        &device_id,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Shoulder"),
        &dummy_hash(&env),
    );

    client.remove_implant(
        &implant_id,
        &provider_id,
        &1750000000u64,
        &String::from_str(&env, "Scheduled removal"),
        &None,
    );

    // Attempt duplicate removal
    let res = client.try_remove_implant(
        &implant_id,
        &provider_id,
        &1760000000u64,
        &String::from_str(&env, "Duplicate"),
        &None,
    );
    assert!(res.is_err());
}

#[test]
fn test_track_device_performance_no_complications() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);
    let implant_id = client.implant_device(
        &patient_id,
        &device_id,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Spine L4"),
        &dummy_hash(&env),
    );

    client.track_device_performance(
        &implant_id,
        &patient_id,
        &dummy_hash(&env),
        &1710000000u64,
        &None,
    );
}

#[test]
fn test_track_device_performance_with_complications() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);
    let implant_id = client.implant_device(
        &patient_id,
        &device_id,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Left Knee"),
        &dummy_hash(&env),
    );

    let mut complications: Vec<String> = Vec::new(&env);
    complications.push_back(String::from_str(&env, "Mild inflammation"));
    complications.push_back(String::from_str(&env, "Intermittent pain"));

    client.track_device_performance(
        &implant_id,
        &patient_id,
        &dummy_hash(&env),
        &1710000000u64,
        &Some(complications),
    );
}

#[test]
fn test_track_device_performance_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let res = client.try_track_device_performance(
        &999u64,
        &patient_id,
        &dummy_hash(&env),
        &1710000000u64,
        &None,
    );
    assert!(res.is_err());
}

#[test]
fn test_get_patient_implants_active_only_filter() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let requester = Address::generate(&env);

    let device_id = register_device_helper(&env, &client);

    let implant_id1 = client.implant_device(
        &patient_id,
        &device_id,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Hip"),
        &dummy_hash(&env),
    );
    client.implant_device(
        &patient_id,
        &device_id,
        &provider_id,
        &1705000000u64,
        &String::from_str(&env, "Knee"),
        &dummy_hash(&env),
    );

    client.remove_implant(
        &implant_id1,
        &provider_id,
        &1750000000u64,
        &String::from_str(&env, "Worn out"),
        &None,
    );

    let active = client.get_patient_implants(&patient_id, &requester, &true);
    assert_eq!(active.len(), 1);

    let all = client.get_patient_implants(&patient_id, &requester, &false);
    assert_eq!(all.len(), 2);
}

#[test]
fn test_check_device_recalls() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let manufacturer = Address::generate(&env);
    let device_id = register_device_helper(&env, &client);

    let no_recalls = client.check_device_recalls(&device_id);
    assert_eq!(no_recalls.len(), 0);

    let mut device_ids: Vec<u64> = Vec::new(&env);
    device_ids.push_back(device_id);

    client.issue_device_recall(
        &manufacturer,
        &device_ids.clone(),
        &String::from_str(&env, "Component failure"),
        &Symbol::new(&env, "HIGH"),
        &1750000000u64,
        &String::from_str(&env, "Return to manufacturer"),
    );
    client.issue_device_recall(
        &manufacturer,
        &device_ids.clone(),
        &String::from_str(&env, "Labeling error"),
        &Symbol::new(&env, "LOW"),
        &1760000000u64,
        &String::from_str(&env, "Update records"),
    );

    let recalls = client.check_device_recalls(&device_id);
    assert_eq!(recalls.len(), 2);

    let r = recalls.get(0).unwrap();
    assert_eq!(r.recall_id, 1);
    assert_eq!(r.severity, Symbol::new(&env, "HIGH"));
}

#[test]
fn test_check_device_recalls_no_results_for_unknown_device() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalDeviceRegistry);
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let recalls = client.check_device_recalls(&999u64);
    assert_eq!(recalls.len(), 0);
}
