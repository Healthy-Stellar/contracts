#![cfg(test)]

use super::*;
use soroban_sdk::{Address, BytesN, Env, String, Symbol};
use soroban_sdk::testutils::Ledger as _;

#[test]
fn test_prescription_lifecycle_invariants() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    env.mock_all_auths();

    // Test prescription issuance
    let req = IssueRequest {
        medication_name: String::from_str(&env, "TestMedication"),
        ndc_code: String::from_str(&env, "12345-678-90"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 30,
        days_supply: 30,
        refills_allowed: 3,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: env.ledger().timestamp() + (30 * 24 * 60 * 60),
        substitution_allowed: true,
        pharmacy_id: Some(pharmacy.clone()),
    };

    let prescription_id = client.issue_prescription(&provider, &patient, &req);

    // Test dispensing with proper validation
    let dispense_req = DispenseRequest {
        prescription_id,
        quantity: 15,
        lot: String::from_str(&env, "LOT123"),
        expires_at: env.ledger().timestamp() + (90 * 24 * 60 * 60),
        ndc_code: String::from_str(&env, "12345-678-90"),
    };

    client.dispense_prescription(&dispense_req, &pharmacy);

    // Test partial dispensing
    let dispense_req2 = DispenseRequest {
        prescription_id,
        quantity: 15,
        lot: String::from_str(&env, "LOT124"),
        expires_at: env.ledger().timestamp() + (90 * 24 * 60 * 60),
        ndc_code: String::from_str(&env, "12345-678-90"),
    };

    client.dispense_prescription(&dispense_req2, &pharmacy);

    // Verify prescription is fully dispensed
    let prescription: Prescription = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&prescription_id).unwrap()
    });
    assert_eq!(prescription.quantity_dispensed, 30);
    assert!(matches!(prescription.status, PrescriptionStatus::Dispensed));
}

#[test]
fn test_prescription_transfer_ownership_verification() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy1 = Address::generate(&env);
    let pharmacy2 = Address::generate(&env);
    let unauthorized_pharmacy = Address::generate(&env);

    env.mock_all_auths();

    // Issue prescription
    let req = IssueRequest {
        medication_name: String::from_str(&env, "TestMedication"),
        ndc_code: String::from_str(&env, "12345-678-90"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 30,
        days_supply: 30,
        refills_allowed: 2,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: env.ledger().timestamp() + (30 * 24 * 60 * 60),
        substitution_allowed: true,
        pharmacy_id: Some(pharmacy1.clone()),
    };

    let prescription_id = client.issue_prescription(&provider, &patient, &req);

    // Test successful transfer
    let transfer_req = TransferRequest {
        prescription_id,
        to_pharmacy: pharmacy2.clone(),
        transfer_reason: String::from_str(&env, "Patient relocation"),
        urgency: Symbol::new(&env, "normal"),
    };

    client.transfer_prescription(&transfer_req, &pharmacy1);
    client.accept_transfer(&prescription_id, &pharmacy2);

    // Test unauthorized transfer fails
    let unauthorized_transfer = TransferRequest {
        prescription_id,
        to_pharmacy: unauthorized_pharmacy,
        transfer_reason: String::from_str(&env, "Unauthorized attempt"),
        urgency: Symbol::new(&env, "normal"),
    };

    let result = client.try_transfer_prescription(&unauthorized_transfer, &pharmacy1);
    assert_eq!(result, Err(Ok(Error::PharmacyNotAuthorized)));
}

#[test]
fn test_controlled_substance_transfer_limits() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy1 = Address::generate(&env);
    let pharmacy2 = Address::generate(&env);
    let pharmacy3 = Address::generate(&env);

    env.mock_all_auths();

    // Issue controlled substance prescription
    let req = IssueRequest {
        medication_name: String::from_str(&env, "Oxycodone"),
        ndc_code: String::from_str(&env, "54321-876-09"),
        dosage: String::from_str(&env, "5mg"),
        quantity: 30,
        days_supply: 15,
        refills_allowed: 1,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: true,
        schedule: Some(2), // Schedule II
        valid_until: env.ledger().timestamp() + (30 * 24 * 60 * 60),
        substitution_allowed: false,
        pharmacy_id: Some(pharmacy1.clone()),
    };

    let prescription_id = client.issue_prescription(&provider, &patient, &req);

    // First transfer should succeed
    let transfer_req1 = TransferRequest {
        prescription_id,
        to_pharmacy: pharmacy2.clone(),
        transfer_reason: String::from_str(&env, "Patient request"),
        urgency: Symbol::new(&env, "normal"),
    };

    client.transfer_prescription(&transfer_req1, &pharmacy1);
    client.accept_transfer(&prescription_id, &pharmacy2);

    // Second transfer should fail for controlled substance
    let transfer_req2 = TransferRequest {
        prescription_id,
        to_pharmacy: pharmacy3,
        transfer_reason: String::from_str(&env, "Second transfer"),
        urgency: Symbol::new(&env, "normal"),
    };

    let result = client.try_transfer_prescription(&transfer_req2, &pharmacy2);
    assert_eq!(result, Err(Ok(Error::ControlledSubstanceViolation)));
}

#[test]
fn test_refill_lifecycle_management() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    env.mock_all_auths();

    // Issue prescription with refills
    let req = IssueRequest {
        medication_name: String::from_str(&env, "TestMedication"),
        ndc_code: String::from_str(&env, "12345-678-90"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 30,
        days_supply: 30,
        refills_allowed: 3,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: env.ledger().timestamp() + (90 * 24 * 60 * 60),
        substitution_allowed: true,
        pharmacy_id: Some(pharmacy.clone()),
    };

    let prescription_id = client.issue_prescription(&provider, &patient, &req);

    // Dispense initial fill
    let dispense_req = DispenseRequest {
        prescription_id,
        quantity: 30,
        lot: String::from_str(&env, "LOT123"),
        expires_at: env.ledger().timestamp() + (90 * 24 * 60 * 60),
        ndc_code: String::from_str(&env, "12345-678-90"),
    };

    client.dispense_prescription(&dispense_req, &pharmacy);

    // Test refill
    client.refill_prescription(&prescription_id, &pharmacy, &provider);

    // Verify refill state
    let prescription: Prescription = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&prescription_id).unwrap()
    });
    assert_eq!(prescription.refills_remaining, 2);
    assert_eq!(prescription.refills_used, 1);
    assert_eq!(prescription.quantity_dispensed, 0);
    assert!(matches!(prescription.status, PrescriptionStatus::Active));

    // Test refill limit exceeded
    client.refill_prescription(&prescription_id, &pharmacy, &provider);
    client.refill_prescription(&prescription_id, &pharmacy, &provider);

    let result = client.try_refill_prescription(&prescription_id, &pharmacy, &provider);
    assert_eq!(result, Err(Ok(Error::RefillExceeded)));
}

#[test]
fn test_prescription_cancellation_safety() {
    let env = Env::default();
    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    env.mock_all_auths();

    // Issue prescription
    let req = IssueRequest {
        medication_name: String::from_str(&env, "TestMedication"),
        ndc_code: String::from_str(&env, "12345-678-90"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 30,
        days_supply: 30,
        refills_allowed: 2,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: env.ledger().timestamp() + (30 * 24 * 60 * 60),
        substitution_allowed: true,
        pharmacy_id: Some(pharmacy.clone()),
    };

    let prescription_id = client.issue_prescription(&provider, &patient, &req);

    // Partially dispense
    let dispense_req = DispenseRequest {
        prescription_id,
        quantity: 15,
        lot: String::from_str(&env, "LOT123"),
        expires_at: env.ledger().timestamp() + (90 * 24 * 60 * 60),
        ndc_code: String::from_str(&env, "12345-678-90"),
    };

    client.dispense_prescription(&dispense_req, &pharmacy);

    // Test normal cancellation fails after partial dispense
    let result = client.try_cancel_prescription(
        &prescription_id,
        &provider,
        &String::from_str(&env, "Change of mind"),
    );
    assert_eq!(result, Err(Ok(Error::InvalidStatusTransition)));

    // Test safety-related cancellation succeeds
    client.cancel_prescription(
        &prescription_id,
        &provider,
        &String::from_str(&env, "safety_concern"),
    );

    let prescription: Prescription = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&prescription_id).unwrap()
    });
    assert!(matches!(prescription.status, PrescriptionStatus::Cancelled));
}

// -----------------------------------------------------------------------
// #324 – Edge-case timestamp tests
// -----------------------------------------------------------------------

/// valid_until = 0 when ledger timestamp = 0 must be rejected immediately.
/// must_be_future requires valid_until > current_timestamp, so 0 > 0 is false.
#[test]
fn test_valid_until_zero_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    // Default ledger timestamp is 0.
    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = IssueRequest {
        medication_name: String::from_str(&env, "TestMed"),
        ndc_code: String::from_str(&env, "00000-0001"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 10,
        days_supply: 10,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: 0, // same as ledger timestamp — not in the future
        substitution_allowed: true,
        pharmacy_id: None,
    };

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::InvalidValidityWindow)));
}

/// valid_until = u64::MAX is outside the 1-year validity window and must be rejected.
#[test]
fn test_valid_until_max_u64_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = IssueRequest {
        medication_name: String::from_str(&env, "TestMed"),
        ndc_code: String::from_str(&env, "00000-0002"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 10,
        days_supply: 10,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: u64::MAX, // far exceeds MAX_VALIDITY_WINDOW_SECS (1 year)
        substitution_allowed: true,
        pharmacy_id: None,
    };

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::InvalidValidityWindow)));
}

/// valid_until exactly at MAX_VALIDITY_WINDOW_SECS (1 year from timestamp = 0) is accepted.
/// valid_until one second beyond is rejected.
#[test]
fn test_valid_until_at_and_beyond_window_limit() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // Exactly 1 year — within_validity_window checks end - start > max_secs,
    // so equality is accepted.
    let at_limit = shared::temporal::MAX_VALIDITY_WINDOW_SECS; // 31_536_000
    let req_ok = IssueRequest {
        medication_name: String::from_str(&env, "TestMedA"),
        ndc_code: String::from_str(&env, "00000-0003"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 10,
        days_supply: 10,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: at_limit,
        substitution_allowed: true,
        pharmacy_id: None,
    };
    let id = client.issue_prescription(&provider, &patient, &req_ok);
    // Prescription was created successfully — fetch it and verify
    let p: Prescription = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&id).unwrap()
    });
    assert_eq!(p.valid_until, at_limit);

    // One second beyond the limit must be rejected.
    let req_over = IssueRequest {
        medication_name: String::from_str(&env, "TestMedB"),
        ndc_code: String::from_str(&env, "00000-0004"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 10,
        days_supply: 10,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: at_limit + 1,
        substitution_allowed: true,
        pharmacy_id: None,
    };
    let result = client.try_issue_prescription(&provider, &patient, &req_over);
    assert_eq!(result, Err(Ok(Error::InvalidValidityWindow)));
}

/// When the ledger timestamp is within 30 days of u64::MAX, the refill
/// validity extension (timestamp + 30 days) would overflow. The checked_add
/// guard must return InvalidValidityWindow instead of panicking.
#[test]
fn test_refill_timestamp_near_max_u64_returns_error() {
    let env = Env::default();
    env.mock_all_auths();

    // Place the ledger 100 seconds before u64::MAX so that adding 30 days overflows.
    let near_max: u64 = u64::MAX - 100;
    env.ledger().set_timestamp(near_max);

    let contract_id = env.register_contract(None, PrescriptionContract);
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    // valid_until = near_max + 50 (50 s ahead, well within the 1-year window)
    let valid_until = near_max + 50;
    let req = IssueRequest {
        medication_name: String::from_str(&env, "TestMed"),
        ndc_code: String::from_str(&env, "00000-0005"),
        dosage: String::from_str(&env, "5mg"),
        quantity: 30,
        days_supply: 30,
        refills_allowed: 3,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until,
        substitution_allowed: true,
        pharmacy_id: Some(pharmacy.clone()),
    };

    let prescription_id = client.issue_prescription(&provider, &patient, &req);

    // Fully dispense to reach Dispensed state (required for refill).
    let dispense_req = DispenseRequest {
        prescription_id,
        quantity: 30,
        lot: String::from_str(&env, "LOT001"),
        expires_at: valid_until,
        ndc_code: String::from_str(&env, "00000-0005"),
    };
    client.dispense_prescription(&dispense_req, &pharmacy);

    // Refill triggers: timestamp + 30_days which overflows u64::MAX.
    // The checked_add guard must surface InvalidValidityWindow.
    let result = client.try_refill_prescription(&prescription_id, &pharmacy, &provider);
    assert_eq!(result, Err(Ok(Error::InvalidValidityWindow)));
}
