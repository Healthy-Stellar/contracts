#![cfg(test)]
#![allow(deprecated)]

extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Symbol, Vec};

fn dummy_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

fn setup_client(env: &Env) -> MedicalDeviceRegistryClient<'_> {
    let regulator = Address::generate(env);
    let contract_id = env.register(MedicalDeviceRegistry, (&regulator,));
    MedicalDeviceRegistryClient::new(env, &contract_id)
}

fn get_udi(client: &MedicalDeviceRegistryClient, device_id: u64) -> String {
    client.get_device_record(&device_id).device_udi
}

fn register_device_helper(
    env: &Env,
    client: &MedicalDeviceRegistryClient,
    manufacturer_id: &Address,
) -> u64 {
    if !client.is_manufacturer_allowlisted(manufacturer_id) {
        client.allowlist_manufacturer(&client.get_regulator(), manufacturer_id);
    }
    let count = client.get_device_count();
    let udi_str = std::format!("UDI-12345-{count}");
    client.register_device(
        manufacturer_id,
        &String::from_str(env, &udi_str),
        &Symbol::new(env, "IMPLANT"),
        &String::from_str(env, "MedCorp"),
        &String::from_str(env, "MC-100"),
        &String::from_str(env, "LOT-001"),
        &1690000000u64,
        &None,
        &dummy_hash(env),
        &DeviceExtras { warranty_expiration_date: None, maintenance_interval_days: None },
    )
}

#[test]
fn test_register_device() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);
    let manufacturer = Address::generate(&env);

    let id = register_device_helper(&env, &client, &manufacturer);
    assert_eq!(id, 1);

    let id2 = register_device_helper(&env, &client, &manufacturer);
    assert_eq!(id2, 2);
}

#[test]
fn test_register_device_with_expiration() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);
    let manufacturer = Address::generate(&env);
    client.allowlist_manufacturer(&client.get_regulator(), &manufacturer);

    let id = client.register_device(
        &manufacturer,
        &String::from_str(&env, "UDI-99999-XYZ"),
        &Symbol::new(&env, "DME"),
        &String::from_str(&env, "DeviceMaker"),
        &String::from_str(&env, "DM-500"),
        &String::from_str(&env, "LOT-999"),
        &1690000000u64,
        &Some(1800000000u64),
        &dummy_hash(&env),
        &DeviceExtras { warranty_expiration_date: None, maintenance_interval_days: None },
    );
    assert_eq!(id, 1);
}

#[test]
fn test_implant_device() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);

    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
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

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    let res = client.try_implant_device(
        &patient_id,
        &String::from_str(&env, "NONEXISTENT-UDI"),
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

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let manufacturer = Address::generate(&env);
    let device_id = register_device_helper(&env, &client, &manufacturer);

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

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let technician = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);
    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
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

    let contract_id = env.register(MedicalDeviceRegistry, (&Address::generate(&env),));
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

    let client = setup_client(&env);

    let manufacturer = Address::generate(&env);
    let device_id = register_device_helper(&env, &client, &manufacturer);

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

    let client = setup_client(&env);

    let patient1 = Address::generate(&env);
    let patient2 = Address::generate(&env);
    let provider = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);

    client.implant_device(
        &patient1,
        &get_udi(&client, device_id),
        &provider,
        &1700000000u64,
        &String::from_str(&env, "Left Knee"),
        &dummy_hash(&env),
    );
    client.implant_device(
        &patient2,
        &get_udi(&client, device_id),
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

    let affected = client.notify_affected_patients(&manufacturer, &recall_id, &1750100000u64);
    assert_eq!(affected.len(), 2);
    assert!(affected.contains(patient1));
    assert!(affected.contains(patient2));
}

#[test]
fn test_notify_affected_patients_excludes_removed() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient1 = Address::generate(&env);
    let patient2 = Address::generate(&env);
    let provider = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);

    let implant_id1 = client.implant_device(
        &patient1,
        &get_udi(&client, device_id),
        &provider,
        &1700000000u64,
        &String::from_str(&env, "Chest"),
        &dummy_hash(&env),
    );
    client.implant_device(
        &patient2,
        &get_udi(&client, device_id),
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

    let affected = client.notify_affected_patients(&manufacturer, &recall_id, &1750100000u64);
    assert_eq!(affected.len(), 1);
    assert!(affected.contains(patient2));
}

#[test]
fn test_notify_affected_patients_recall_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let res = client.try_notify_affected_patients(&Address::generate(&env), &999u64, &1750100000u64);
    assert!(res.is_err());
}

#[test]
fn test_notify_affected_patients_rejects_unauthorized_caller() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let manufacturer = Address::generate(&env);
    let stranger = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);
    client.implant_device(
        &patient,
        &get_udi(&client, device_id),
        &provider,
        &1700000000u64,
        &String::from_str(&env, "Left Knee"),
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

    // A caller who is neither the recall issuer nor the configured regulator
    // must not be able to enumerate affected patients.
    let res = client.try_notify_affected_patients(&stranger, &recall_id, &1750100000u64);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_notify_affected_patients_allows_configured_regulator() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let contract_id = env.register(MedicalDeviceRegistry, (&regulator,));
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);
    client.implant_device(
        &patient,
        &get_udi(&client, device_id),
        &provider,
        &1700000000u64,
        &String::from_str(&env, "Left Knee"),
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

    // The configured regulator has oversight of all recalls, even ones it
    // did not personally issue.
    let affected = client.notify_affected_patients(&regulator, &recall_id, &1750100000u64);
    assert_eq!(affected.len(), 1);
    assert!(affected.contains(patient));
}

#[test]
fn test_remove_implant() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);
    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
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

    let client = setup_client(&env);

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

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);
    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
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
fn test_remove_implant_rejects_unauthorized_provider() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let other_provider = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);
    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Shoulder"),
        &dummy_hash(&env),
    );

    // A provider who did not perform the implant attempts removal.
    let res = client.try_remove_implant(
        &implant_id,
        &other_provider,
        &1750000000u64,
        &String::from_str(&env, "Unauthorized removal attempt"),
        &None,
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_falsely_removed_implant_cannot_suppress_recall_notification() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let other_provider = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);
    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Chest"),
        &dummy_hash(&env),
    );

    // Unauthorized removal attempt must fail and not affect the implant.
    let res = client.try_remove_implant(
        &implant_id,
        &other_provider,
        &1740000000u64,
        &String::from_str(&env, "Malicious removal"),
        &None,
    );
    assert!(res.is_err());

    let mut device_ids: Vec<u64> = Vec::new(&env);
    device_ids.push_back(device_id);

    let recall_id = client.issue_device_recall(
        &manufacturer,
        &device_ids,
        &String::from_str(&env, "Battery failure risk"),
        &Symbol::new(&env, "HIGH"),
        &1750000000u64,
        &String::from_str(&env, "Immediate replacement required"),
    );

    // The patient must still be surfaced for recall notification.
    let affected = client.notify_affected_patients(&manufacturer, &recall_id, &1750100000u64);
    assert_eq!(affected.len(), 1);
    assert!(affected.contains(patient_id));
}

#[test]
fn test_track_device_performance_no_complications() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);
    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
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

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let manufacturer = Address::generate(&env);
    let device_id = register_device_helper(&env, &client, &manufacturer);
    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
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

    let contract_id = env.register(MedicalDeviceRegistry, (&Address::generate(&env),));
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

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let requester = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);

    let implant_id1 = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Hip"),
        &dummy_hash(&env),
    );
    client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
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

    let client = setup_client(&env);

    let manufacturer = Address::generate(&env);
    let device_id = register_device_helper(&env, &client, &manufacturer);

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

    let client = setup_client(&env);

    let recalls = client.check_device_recalls(&999u64);
    assert_eq!(recalls.len(), 0);
}

#[test]
fn test_recall_rejected_for_unrelated_manufacturer() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);
    let manufacturer = Address::generate(&env);
    let spoofing_manufacturer = Address::generate(&env);
    let device_id = register_device_helper(&env, &client, &manufacturer);

    let mut device_ids: Vec<u64> = Vec::new(&env);
    device_ids.push_back(device_id);

    let result = client.try_issue_device_recall(
        &spoofing_manufacturer,
        &device_ids,
        &String::from_str(&env, "Spoofed recall"),
        &Symbol::new(&env, "HIGH"),
        &1750000000u64,
        &String::from_str(&env, "Ignore"),
    );
    assert_eq!(result, Err(Ok(Error::NotAuthorized)));
}

#[test]
fn test_regulator_emergency_recall_records_scope() {
    let env = Env::default();
    env.mock_all_auths();

    let regulator = Address::generate(&env);
    let contract_id = env.register(MedicalDeviceRegistry, (&regulator,));
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let manufacturer_a = Address::generate(&env);
    let manufacturer_b = Address::generate(&env);
    let device_a = register_device_helper(&env, &client, &manufacturer_a);
    let device_b = register_device_helper(&env, &client, &manufacturer_b);

    let mut device_ids: Vec<u64> = Vec::new(&env);
    device_ids.push_back(device_a);
    device_ids.push_back(device_b);

    let recall_id = client.issue_regulator_recall(
        &regulator,
        &device_ids,
        &String::from_str(&env, "Cross-manufacturer safety event"),
        &Symbol::new(&env, "CRITICAL"),
        &1750000000u64,
        &String::from_str(&env, "Immediate quarantine"),
        &String::from_str(&env, "national-device-hold"),
    );

    let recalls = client.check_device_recalls(&device_a);
    let recall = recalls.get(0).unwrap();
    assert_eq!(recall_id, recall.recall_id);
    assert_eq!(recall.issuer, regulator);
    assert_eq!(recall.issuer_role, Symbol::new(&env, "reg"));
    assert_eq!(
        recall.emergency_scope,
        Some(String::from_str(&env, "national-device-hold"))
    );
}

#[test]
fn test_track_device_performance_patient_mismatch() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient_a = Address::generate(&env);
    let patient_b = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    let device_id = register_device_helper(&env, &client, &manufacturer);

    // Implant device for patient_a
    let implant_id = client.implant_device(
        &patient_a,
        &get_udi(&client, device_id),
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Left Hip"),
        &dummy_hash(&env),
    );

    // Patient B attempts to file performance report for Patient A's implant
    let result = client.try_track_device_performance(
        &implant_id,
        &patient_b,
        &dummy_hash(&env),
        &1700086400u64,
        &None,
    );

    // Should fail with NotAuthorized error
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_schedule_next_maintenance_requires_auth() {
    let env = Env::default();
    // env.mock_all_auths() is NOT called, so auth will fail
    let contract_id = env.register(MedicalDeviceRegistry, (&Address::generate(&env),));
    let client = MedicalDeviceRegistryClient::new(&env, &contract_id);

    let manufacturer = Address::generate(&env);

    client.schedule_next_maintenance(&1u64, &1710000000u64, &manufacturer);
}

#[test]
fn test_schedule_next_maintenance_rejects_unauthorized_caller() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let manufacturer = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.allowlist_manufacturer(&client.get_regulator(), &manufacturer);

    let device_id = client.register_device(
        &manufacturer,
        &String::from_str(&env, "UDI-55555-DME"),
        &Symbol::new(&env, "DME"),
        &String::from_str(&env, "MedCorp"),
        &String::from_str(&env, "MC-200"),
        &String::from_str(&env, "LOT-002"),
        &1690000000u64,
        &None,
        &dummy_hash(&env),
        &DeviceExtras {
            warranty_expiration_date: None,
            maintenance_interval_days: Some(90u64),
        },
    );

    // A caller who is neither the manufacturer nor a technician with an
    // existing maintenance-history entry for this device must not be able
    // to rewrite the maintenance schedule.
    let res = client.try_schedule_next_maintenance(&device_id, &1710000000u64, &stranger);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotAuthorized);
}

#[test]
fn test_schedule_next_maintenance_allows_manufacturer() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let manufacturer = Address::generate(&env);
    client.allowlist_manufacturer(&client.get_regulator(), &manufacturer);

    let device_id = client.register_device(
        &manufacturer,
        &String::from_str(&env, "UDI-66666-DME"),
        &Symbol::new(&env, "DME"),
        &String::from_str(&env, "MedCorp"),
        &String::from_str(&env, "MC-300"),
        &String::from_str(&env, "LOT-003"),
        &1690000000u64,
        &None,
        &dummy_hash(&env),
        &DeviceExtras {
            warranty_expiration_date: None,
            maintenance_interval_days: Some(90u64),
        },
    );

    let completed_date = 1710000000u64;
    client.schedule_next_maintenance(&device_id, &completed_date, &manufacturer);

    assert!(client.check_maintenance_due(&device_id, &(completed_date + 90 * 86400)));
    assert!(!client.check_maintenance_due(&device_id, &(completed_date + 86400)));
}

#[test]
fn test_schedule_next_maintenance_allows_technician_with_history() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    let technician = Address::generate(&env);
    let manufacturer = Address::generate(&env);
    client.allowlist_manufacturer(&client.get_regulator(), &manufacturer);

    let device_id = client.register_device(
        &manufacturer,
        &String::from_str(&env, "UDI-77777-IMP"),
        &Symbol::new(&env, "IMPLANT"),
        &String::from_str(&env, "MedCorp"),
        &String::from_str(&env, "MC-400"),
        &String::from_str(&env, "LOT-004"),
        &1690000000u64,
        &None,
        &dummy_hash(&env),
        &DeviceExtras {
            warranty_expiration_date: None,
            maintenance_interval_days: Some(90u64),
        },
    );

    let implant_id = client.implant_device(
        &patient_id,
        &get_udi(&client, device_id),
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Left Hip"),
        &dummy_hash(&env),
    );

    // Technician records maintenance for this device's implant first,
    // establishing their authorization via the device's maintenance history.
    client.record_device_maintenance(
        &implant_id,
        &1710000000u64,
        &Symbol::new(&env, "CALIBRATION"),
        &technician,
        &dummy_hash(&env),
    );

    let completed_date = 1710000000u64;
    client.schedule_next_maintenance(&device_id, &completed_date, &technician);

    assert!(client.check_maintenance_due(&device_id, &(completed_date + 90 * 86400)));
}

#[test]
fn test_unallowlisted_manufacturer_cannot_register_device() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);
    let manufacturer = Address::generate(&env);

    let res = client.try_register_device(
        &manufacturer,
        &String::from_str(&env, "UDI-UNAUTH-1"),
        &Symbol::new(&env, "IMPLANT"),
        &String::from_str(&env, "FakeCorp"),
        &String::from_str(&env, "FC-1"),
        &String::from_str(&env, "LOT-01"),
        &1690000000u64,
        &None,
        &dummy_hash(&env),
        &DeviceExtras { warranty_expiration_date: None, maintenance_interval_days: None },
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::ManufacturerNotAuthorized);
}

#[test]
fn test_duplicate_udi_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);
    let regulator = client.get_regulator();
    let manufacturer_a = Address::generate(&env);
    let manufacturer_b = Address::generate(&env);

    client.allowlist_manufacturer(&regulator, &manufacturer_a);
    client.allowlist_manufacturer(&regulator, &manufacturer_b);

    let udi = String::from_str(&env, "UDI-GLOBAL-UNIQUE-1");

    let id1 = client.register_device(
        &manufacturer_a,
        &udi,
        &Symbol::new(&env, "IMPLANT"),
        &String::from_str(&env, "RealCorp"),
        &String::from_str(&env, "RC-1"),
        &String::from_str(&env, "LOT-01"),
        &1690000000u64,
        &None,
        &dummy_hash(&env),
        &DeviceExtras { warranty_expiration_date: None, maintenance_interval_days: None },
    );
    assert_eq!(id1, 1);

    // Attempting to register the same UDI (even from another manufacturer) must fail
    let res = client.try_register_device(
        &manufacturer_b,
        &udi,
        &Symbol::new(&env, "IMPLANT"),
        &String::from_str(&env, "ShadowCorp"),
        &String::from_str(&env, "SC-1"),
        &String::from_str(&env, "LOT-02"),
        &1690000000u64,
        &None,
        &dummy_hash(&env),
        &DeviceExtras { warranty_expiration_date: None, maintenance_interval_days: None },
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::DuplicateUdi);
}

#[test]
fn test_implant_device_resolves_udi_correctly() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);
    let regulator = client.get_regulator();
    let manufacturer = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);

    client.allowlist_manufacturer(&regulator, &manufacturer);

    let udi = String::from_str(&env, "UDI-PACEMAKER-42");
    let dev_id = client.register_device(
        &manufacturer,
        &udi,
        &Symbol::new(&env, "PACEMAKER"),
        &String::from_str(&env, "HeartTech"),
        &String::from_str(&env, "HT-10"),
        &String::from_str(&env, "LOT-42"),
        &1690000000u64,
        &None,
        &dummy_hash(&env),
        &DeviceExtras { warranty_expiration_date: None, maintenance_interval_days: None },
    );

    let implant_id = client.implant_device(
        &patient_id,
        &udi,
        &provider_id,
        &1700000000u64,
        &String::from_str(&env, "Chest"),
        &dummy_hash(&env),
    );
    assert_eq!(implant_id, 1);

    let requester = Address::generate(&env);
    let implants = client.get_patient_implants(&patient_id, &requester, &true);
    assert_eq!(implants.len(), 1);
    assert_eq!(implants.get(0).unwrap().device_id, dev_id);
}

#[test]
fn test_maintenance_schedule_checked_arithmetic_overflow() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);
    let regulator = client.get_regulator();
    let manufacturer = Address::generate(&env);
    client.allowlist_manufacturer(&regulator, &manufacturer);

    // Huge interval that overflows interval * 86400
    let res = client.try_register_device(
        &manufacturer,
        &String::from_str(&env, "UDI-OVERFLOW-1"),
        &Symbol::new(&env, "DME"),
        &String::from_str(&env, "MedCorp"),
        &String::from_str(&env, "MC-1"),
        &String::from_str(&env, "LOT-1"),
        &1690000000u64,
        &None,
        &dummy_hash(&env),
        &DeviceExtras {
            warranty_expiration_date: None,
            maintenance_interval_days: Some(u64::MAX),
        },
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::ArithmeticOverflow);
}

#[test]
fn test_regulator_allowlist_management() {
    let env = Env::default();
    env.mock_all_auths();

    let client = setup_client(&env);
    let regulator = client.get_regulator();
    let non_regulator = Address::generate(&env);
    let manufacturer = Address::generate(&env);

    assert!(!client.is_manufacturer_allowlisted(&manufacturer));

    // Non-regulator cannot allowlist
    let unauth_res = client.try_allowlist_manufacturer(&non_regulator, &manufacturer);
    assert_eq!(unauth_res.unwrap_err().unwrap(), Error::NotAuthorized);

    // Regulator can allowlist
    client.allowlist_manufacturer(&regulator, &manufacturer);
    assert!(client.is_manufacturer_allowlisted(&manufacturer));

    // Non-regulator cannot revoke
    let unauth_revoke = client.try_revoke_manufacturer(&non_regulator, &manufacturer);
    assert_eq!(unauth_revoke.unwrap_err().unwrap(), Error::NotAuthorized);

    // Regulator can revoke
    client.revoke_manufacturer(&regulator, &manufacturer);
    assert!(!client.is_manufacturer_allowlisted(&manufacturer));
}

