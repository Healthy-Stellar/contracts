#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, vec as svec, Env};

#[test]
fn test_order_imaging_study() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    env.mock_all_auths();

    let study_type = Symbol::new(&env, "CT");
    let body_part = String::from_str(&env, "Chest");
    let clinical_indication = String::from_str(&env, "Rule out pulmonary embolism");
    let priority = Symbol::new(&env, "URGENT");

    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &study_type,
        &body_part,
        &true,
        &clinical_indication,
        &priority,
    );

    assert_eq!(order_id, 1);

    // Verify order was created
    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.provider_id, provider);
    assert_eq!(order.patient_id, patient);
    assert_eq!(order.study_type, study_type);
    assert_eq!(order.contrast_required, true);
    assert_eq!(order.status, Symbol::new(&env, "ORDERED"));
}

#[test]
fn test_multiple_imaging_orders() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    env.mock_all_auths();

    // Order 1: CT Scan
    let order_id1 = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Abdomen"),
        &true,
        &String::from_str(&env, "Abdominal pain"),
        &Symbol::new(&env, "ROUTINE"),
    );

    // Order 2: X-Ray
    let order_id2 = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "XRAY"),
        &String::from_str(&env, "Chest"),
        &false,
        &String::from_str(&env, "Cough"),
        &Symbol::new(&env, "ROUTINE"),
    );

    assert_eq!(order_id1, 1);
    assert_eq!(order_id2, 2);

    // Verify patient has both orders
    let patient_orders = client.get_patient_orders(&patient);
    assert_eq!(patient_orders.len(), 2);
}

#[test]
fn test_schedule_imaging() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    env.mock_all_auths();

    // Create order
    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "MRI"),
        &String::from_str(&env, "Brain"),
        &true,
        &String::from_str(&env, "Headaches"),
        &Symbol::new(&env, "ROUTINE"),
    );

    // Schedule imaging
    let scheduled_time = env.ledger().timestamp() + 86400; // Tomorrow
    let prep_hash = BytesN::from_array(&env, &[1u8; 32]);

    client.schedule_imaging(&order_id, &imaging_center, &scheduled_time, &prep_hash);

    // Verify schedule
    let schedule = client.get_imaging_schedule(&order_id).unwrap();
    assert_eq!(schedule.imaging_center, imaging_center);
    assert_eq!(schedule.scheduled_time, scheduled_time);

    // Verify order status updated
    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.status, Symbol::new(&env, "SCHEDULED"));
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_schedule_imaging_already_scheduled() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    env.mock_all_auths();

    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "XRAY"),
        &String::from_str(&env, "Knee"),
        &false,
        &String::from_str(&env, "Pain"),
        &Symbol::new(&env, "ROUTINE"),
    );

    let scheduled_time = env.ledger().timestamp() + 3600;
    let prep_hash = BytesN::from_array(&env, &[1u8; 32]);

    // First schedule succeeds
    client.schedule_imaging(&order_id, &imaging_center, &scheduled_time, &prep_hash);

    // Second schedule fails
    client.schedule_imaging(&order_id, &imaging_center, &scheduled_time, &prep_hash);
}

#[test]
fn test_upload_images() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    env.mock_all_auths();

    // Create and schedule order
    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Chest"),
        &true,
        &String::from_str(&env, "Trauma"),
        &Symbol::new(&env, "STAT"),
    );

    let scheduled_time = env.ledger().timestamp() + 1800;
    let prep_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.schedule_imaging(&order_id, &imaging_center, &scheduled_time, &prep_hash);

    // Upload images
    let dicom_hash = BytesN::from_array(&env, &[42u8; 32]);
    let image_count = 150;
    let study_date = env.ledger().timestamp();

    client.upload_images(
        &order_id,
        &imaging_center,
        &dicom_hash,
        &image_count,
        &study_date,
    );

    // Verify images uploaded
    let images = client.get_dicom_images(&order_id).unwrap();
    assert_eq!(images.dicom_hash, dicom_hash);
    assert_eq!(images.image_count, 150);

    // Verify order status updated
    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.status, Symbol::new(&env, "IN_PROGRESS"));
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_upload_images_already_uploaded() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    env.mock_all_auths();

    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "XRAY"),
        &String::from_str(&env, "Hand"),
        &false,
        &String::from_str(&env, "Fracture"),
        &Symbol::new(&env, "URGENT"),
    );

    let dicom_hash = BytesN::from_array(&env, &[10u8; 32]);
    let study_date = env.ledger().timestamp();

    // First upload succeeds
    client.upload_images(&order_id, &imaging_center, &dicom_hash, &5, &study_date);

    // Second upload fails
    client.upload_images(&order_id, &imaging_center, &dicom_hash, &5, &study_date);
}

#[test]
fn test_submit_preliminary_report() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    let radiologist = Address::generate(&env);
    env.mock_all_auths();

    // Create order and upload images
    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Head"),
        &true,
        &String::from_str(&env, "Stroke workup"),
        &Symbol::new(&env, "STAT"),
    );

    let dicom_hash = BytesN::from_array(&env, &[20u8; 32]);
    client.upload_images(
        &order_id,
        &imaging_center,
        &dicom_hash,
        &200,
        &env.ledger().timestamp(),
    );

    // Submit preliminary report
    let report_hash = BytesN::from_array(&env, &[30u8; 32]);
    client.submit_preliminary_report(&order_id, &radiologist, &report_hash, &true);

    // Verify report
    let report = client.get_preliminary_report(&order_id).unwrap();
    assert_eq!(report.radiologist_id, radiologist);
    assert_eq!(report.urgent_findings, true);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_submit_preliminary_report_without_images() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let radiologist = Address::generate(&env);
    env.mock_all_auths();

    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "MRI"),
        &String::from_str(&env, "Spine"),
        &false,
        &String::from_str(&env, "Back pain"),
        &Symbol::new(&env, "ROUTINE"),
    );

    // Try to submit report without images
    let report_hash = BytesN::from_array(&env, &[40u8; 32]);
    client.submit_preliminary_report(&order_id, &radiologist, &report_hash, &false);
}

#[test]
fn test_submit_final_report() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    let radiologist = Address::generate(&env);
    env.mock_all_auths();

    // Create order and upload images
    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "MAMMO"),
        &String::from_str(&env, "Bilateral"),
        &false,
        &String::from_str(&env, "Screening"),
        &Symbol::new(&env, "ROUTINE"),
    );

    let dicom_hash = BytesN::from_array(&env, &[50u8; 32]);
    client.upload_images(
        &order_id,
        &imaging_center,
        &dicom_hash,
        &4,
        &env.ledger().timestamp(),
    );

    // Submit final report
    let final_hash = BytesN::from_array(&env, &[60u8; 32]);
    let impression = String::from_str(&env, "No evidence of malignancy. BI-RADS 1.");

    client.submit_final_report(&order_id, &radiologist, &final_hash, &impression);

    // Verify final report
    let report = client.get_final_report(&order_id).unwrap();
    assert_eq!(report.radiologist_id, radiologist);
    assert_eq!(report.impression, impression);

    // Verify order completed
    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.status, Symbol::new(&env, "COMPLETED"));
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_submit_final_report_already_exists() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    let radiologist = Address::generate(&env);
    env.mock_all_auths();

    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "ULTRASOUND"),
        &String::from_str(&env, "Abdomen"),
        &false,
        &String::from_str(&env, "RUQ pain"),
        &Symbol::new(&env, "URGENT"),
    );

    let dicom_hash = BytesN::from_array(&env, &[70u8; 32]);
    client.upload_images(
        &order_id,
        &imaging_center,
        &dicom_hash,
        &50,
        &env.ledger().timestamp(),
    );

    let final_hash = BytesN::from_array(&env, &[80u8; 32]);
    let impression = String::from_str(&env, "Normal study");

    // First submission succeeds
    client.submit_final_report(&order_id, &radiologist, &final_hash, &impression);

    // Second submission fails
    client.submit_final_report(&order_id, &radiologist, &final_hash, &impression);
}

#[test]
fn test_request_peer_review() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let radiologist1 = Address::generate(&env);
    let radiologist2 = Address::generate(&env);
    env.mock_all_auths();

    // Create order
    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "PET"),
        &String::from_str(&env, "Whole body"),
        &false,
        &String::from_str(&env, "Cancer staging"),
        &Symbol::new(&env, "ROUTINE"),
    );

    // Request peer review
    client.request_peer_review(&order_id, &radiologist1, &radiologist2);

    // Verify peer review request
    let peer_review = client.get_peer_review(&order_id).unwrap();
    assert_eq!(peer_review.requesting_radiologist, radiologist1);
    assert_eq!(peer_review.peer_radiologist, radiologist2);
    assert_eq!(peer_review.status, Symbol::new(&env, "PENDING"));
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_request_peer_review_already_exists() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let radiologist1 = Address::generate(&env);
    let radiologist2 = Address::generate(&env);
    env.mock_all_auths();

    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Chest"),
        &true,
        &String::from_str(&env, "Complex case"),
        &Symbol::new(&env, "ROUTINE"),
    );

    // First request succeeds
    client.request_peer_review(&order_id, &radiologist1, &radiologist2);

    // Second request fails
    client.request_peer_review(&order_id, &radiologist1, &radiologist2);
}

#[test]
fn test_get_patient_orders() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    env.mock_all_auths();

    // Create multiple orders for same patient
    let order_id1 = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "XRAY"),
        &String::from_str(&env, "Chest"),
        &false,
        &String::from_str(&env, "Cough"),
        &Symbol::new(&env, "ROUTINE"),
    );

    let order_id2 = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Abdomen"),
        &true,
        &String::from_str(&env, "Pain"),
        &Symbol::new(&env, "URGENT"),
    );

    // Get patient orders
    let orders = client.get_patient_orders(&patient);
    assert_eq!(orders.len(), 2);
    assert_eq!(orders.get(0).unwrap(), order_id1);
    assert_eq!(orders.get(1).unwrap(), order_id2);
}

#[test]
fn test_get_provider_orders() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient1 = Address::generate(&env);
    let patient2 = Address::generate(&env);
    env.mock_all_auths();

    // Create orders from same provider
    let order_id1 = client.order_imaging_study(
        &provider,
        &patient1,
        &Symbol::new(&env, "MRI"),
        &String::from_str(&env, "Brain"),
        &true,
        &String::from_str(&env, "Headache"),
        &Symbol::new(&env, "ROUTINE"),
    );

    let order_id2 = client.order_imaging_study(
        &provider,
        &patient2,
        &Symbol::new(&env, "XRAY"),
        &String::from_str(&env, "Knee"),
        &false,
        &String::from_str(&env, "Injury"),
        &Symbol::new(&env, "URGENT"),
    );

    // Get provider orders
    let orders = client.get_provider_orders(&provider);
    assert_eq!(orders.len(), 2);
    assert_eq!(orders.get(0).unwrap(), order_id1);
    assert_eq!(orders.get(1).unwrap(), order_id2);
}

#[test]
fn test_complete_imaging_workflow() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    let radiologist = Address::generate(&env);
    let peer_radiologist = Address::generate(&env);
    env.mock_all_auths();

    // 1. Order imaging study
    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Chest/Abdomen/Pelvis"),
        &true,
        &String::from_str(&env, "Cancer staging"),
        &Symbol::new(&env, "URGENT"),
    );

    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.status, Symbol::new(&env, "ORDERED"));

    // 2. Schedule imaging
    let scheduled_time = env.ledger().timestamp() + 7200;
    let prep_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.schedule_imaging(&order_id, &imaging_center, &scheduled_time, &prep_hash);

    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.status, Symbol::new(&env, "SCHEDULED"));

    // 3. Upload images
    let dicom_hash = BytesN::from_array(&env, &[2u8; 32]);
    client.upload_images(
        &order_id,
        &imaging_center,
        &dicom_hash,
        &500,
        &env.ledger().timestamp(),
    );

    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.status, Symbol::new(&env, "IN_PROGRESS"));

    // 4. Submit preliminary report with urgent findings
    let prelim_hash = BytesN::from_array(&env, &[3u8; 32]);
    client.submit_preliminary_report(&order_id, &radiologist, &prelim_hash, &true);

    let prelim = client.get_preliminary_report(&order_id).unwrap();
    assert_eq!(prelim.urgent_findings, true);

    // 5. Request peer review
    client.request_peer_review(&order_id, &radiologist, &peer_radiologist);

    let peer_review = client.get_peer_review(&order_id).unwrap();
    assert_eq!(peer_review.status, Symbol::new(&env, "PENDING"));

    // 6. Submit final report
    let final_hash = BytesN::from_array(&env, &[4u8; 32]);
    let impression = String::from_str(
        &env,
        "Multiple pulmonary nodules. Recommend follow-up CT in 3 months.",
    );
    client.submit_final_report(&order_id, &radiologist, &final_hash, &impression);

    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.status, Symbol::new(&env, "COMPLETED"));

    let final_report = client.get_final_report(&order_id).unwrap();
    assert_eq!(final_report.radiologist_id, radiologist);
}

#[test]
fn test_multi_modality_support() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    env.mock_all_auths();

    // Test different modalities
    let modalities = svec![
        &env,
        Symbol::new(&env, "XRAY"),
        Symbol::new(&env, "CT"),
        Symbol::new(&env, "MRI"),
        Symbol::new(&env, "ULTRASOUND"),
        Symbol::new(&env, "PET"),
        Symbol::new(&env, "MAMMO"),
    ];

    for i in 0..modalities.len() {
        let modality = modalities.get(i).unwrap();
        let order_id = client.order_imaging_study(
            &provider,
            &patient,
            &modality,
            &String::from_str(&env, "Test body part"),
            &false,
            &String::from_str(&env, "Test indication"),
            &Symbol::new(&env, "ROUTINE"),
        );

        let order = client.get_imaging_order(&order_id).unwrap();
        assert_eq!(order.study_type, modality);
    }
}

#[test]
fn test_priority_levels() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    env.mock_all_auths();

    // Test different priority levels
    let priorities = svec![
        &env,
        Symbol::new(&env, "STAT"),
        Symbol::new(&env, "URGENT"),
        Symbol::new(&env, "ROUTINE"),
    ];

    for i in 0..priorities.len() {
        let priority = priorities.get(i).unwrap();
        let order_id = client.order_imaging_study(
            &provider,
            &patient,
            &Symbol::new(&env, "XRAY"),
            &String::from_str(&env, "Chest"),
            &false,
            &String::from_str(&env, "Test"),
            &priority,
        );

        let order = client.get_imaging_order(&order_id).unwrap();
        assert_eq!(order.priority, priority);
    }
}

#[test]
fn test_urgent_findings_notification() {
    let env = Env::default();
    let contract_id = env.register(ImagingRadiology, ());
    let client = ImagingRadiologyClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let imaging_center = Address::generate(&env);
    let radiologist = Address::generate(&env);
    env.mock_all_auths();

    // Create order with STAT priority
    let order_id = client.order_imaging_study(
        &provider,
        &patient,
        &Symbol::new(&env, "CT"),
        &String::from_str(&env, "Head"),
        &true,
        &String::from_str(&env, "Acute stroke"),
        &Symbol::new(&env, "STAT"),
    );

    // Upload images
    let dicom_hash = BytesN::from_array(&env, &[100u8; 32]);
    client.upload_images(
        &order_id,
        &imaging_center,
        &dicom_hash,
        &150,
        &env.ledger().timestamp(),
    );

    // Submit preliminary report with urgent findings
    let report_hash = BytesN::from_array(&env, &[101u8; 32]);
    client.submit_preliminary_report(&order_id, &radiologist, &report_hash, &true);

    // Verify urgent findings flag
    let prelim = client.get_preliminary_report(&order_id).unwrap();
    assert_eq!(prelim.urgent_findings, true);

    // Verify order priority
    let order = client.get_imaging_order(&order_id).unwrap();
    assert_eq!(order.priority, Symbol::new(&env, "STAT"));
}
