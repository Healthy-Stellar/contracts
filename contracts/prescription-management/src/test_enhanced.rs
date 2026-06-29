#![cfg(test)]

use super::*;
use soroban_sdk::{Address, BytesN, Env, String, Symbol};
use soroban_sdk::testutils::Ledger as _;

#[test]
fn test_prescription_lifecycle_invariants() {
    let env = Env::default();
    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
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
    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
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
    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        // AB1234563: A=registrant type, B=last-name initial, check digit 3 ✓
        dea_number: Some(String::from_str(&env, "AB1234563")),
        bypass_reason_hash: None,
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
    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
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
    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
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
    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
    };

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::InvalidValidityWindow)));
}

/// valid_until = u64::MAX is outside the 1-year validity window and must be rejected.
#[test]
fn test_valid_until_max_u64_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
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

    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
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

    let contract_id = env.register(PrescriptionContract, ());
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
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
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

// ── DEA number validation tests ───────────────────────────────────────────────
//
// Coverage:
//   1. Controlled substance with valid DEA number → accepted
//   2. Controlled substance with no DEA number → ControlledSubstanceViolation
//   3. Controlled substance with malformed DEA (wrong length) → ControlledSubstanceViolation
//   4. Controlled substance with invalid first letter → ControlledSubstanceViolation
//   5. Controlled substance with bad check digit → ControlledSubstanceViolation
//   6. Non-controlled prescription with dea_number: None → accepted
//   7. Non-controlled prescription with dea_number: Some(...) → accepted (ignored)
//   8. Controlled substance with schedule: None → accepted even without DEA number

/// Helper: build a minimal IssueRequest for DEA tests.
/// Valid DEA "AB1234563": A=registrant type, B=last-name initial.
/// Check: (1+3+5) + 2*(2+4+6) = 9 + 24 = 33 → units digit 3 = d7 ✓
fn make_dea_test_req(
    env: &Env,
    provider: &Address,
    patient: &Address,
    is_controlled: bool,
    schedule: Option<u32>,
    dea_number: Option<String>,
) -> IssueRequest {
    IssueRequest {
        medication_name: String::from_str(env, "Hydrocodone"),
        ndc_code: String::from_str(env, "00406-0369-01"),
        dosage: String::from_str(env, "5mg"),
        quantity: 30,
        days_supply: 30,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(env, &[0u8; 32]),
        is_controlled,
        schedule,
        valid_until: env.ledger().timestamp() + 86_400,
        substitution_allowed: false,
        pharmacy_id: None,
        bypass_allergy_check: false,
        dea_number,
        bypass_reason_hash: None,
    }
}

/// Controlled + schedule + valid DEA → prescription issued successfully.
#[test]
fn test_controlled_with_valid_dea_number_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = make_dea_test_req(
        &env,
        &provider,
        &patient,
        true,
        Some(2),
        Some(String::from_str(&env, "AB1234563")),
    );

    // Must not panic or return an error
    let id = client.issue_prescription(&provider, &patient, &req);
    assert!(id < u64::MAX);
}

/// Controlled + schedule, but dea_number: None → ControlledSubstanceViolation.
#[test]
fn test_controlled_missing_dea_number_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = make_dea_test_req(&env, &provider, &patient, true, Some(2), None);

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::ControlledSubstanceViolation)));
}

/// Controlled + schedule, DEA number is only 8 characters (wrong length).
#[test]
fn test_controlled_dea_number_wrong_length_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // 8 chars instead of 9
    let req = make_dea_test_req(
        &env,
        &provider,
        &patient,
        true,
        Some(3),
        Some(String::from_str(&env, "AB123456")),
    );

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::ControlledSubstanceViolation)));
}

/// Controlled + schedule, DEA number is 10 characters (too long).
#[test]
fn test_controlled_dea_number_too_long_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // 10 chars
    let req = make_dea_test_req(
        &env,
        &provider,
        &patient,
        true,
        Some(4),
        Some(String::from_str(&env, "AB12345630")),
    );

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::ControlledSubstanceViolation)));
}

/// Controlled + schedule, first letter is a digit (not a valid registrant type).
#[test]
fn test_controlled_dea_number_first_char_not_letter_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // Starts with digit '1' — invalid
    let req = make_dea_test_req(
        &env,
        &provider,
        &patient,
        true,
        Some(2),
        Some(String::from_str(&env, "1B1234563")),
    );

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::ControlledSubstanceViolation)));
}

/// Controlled + schedule, DEA number has correct structure but wrong check digit.
/// "AB1234560" — check should be 3, not 0.
#[test]
fn test_controlled_dea_number_bad_check_digit_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = make_dea_test_req(
        &env,
        &provider,
        &patient,
        true,
        Some(2),
        Some(String::from_str(&env, "AB1234560")), // check digit 0, should be 3
    );

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::ControlledSubstanceViolation)));
}

/// Non-controlled prescription with dea_number: None → accepted without error.
#[test]
fn test_non_controlled_none_dea_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = make_dea_test_req(&env, &provider, &patient, false, None, None);

    let id = client.issue_prescription(&provider, &patient, &req);
    assert!(id < u64::MAX);
}

/// Non-controlled prescription may optionally carry a DEA number without error.
#[test]
fn test_non_controlled_with_dea_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = make_dea_test_req(
        &env,
        &provider,
        &patient,
        false,
        None,
        Some(String::from_str(&env, "AB1234563")),
    );

    let id = client.issue_prescription(&provider, &patient, &req);
    assert!(id < u64::MAX);
}

/// Controlled prescription with schedule: None skips DEA validation entirely.
/// (No schedule means it is not a scheduled controlled substance.)
#[test]
fn test_controlled_no_schedule_skips_dea_check() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // is_controlled true but schedule None — DEA check is not triggered
    let req = make_dea_test_req(&env, &provider, &patient, true, None, None);

    let id = client.issue_prescription(&provider, &patient, &req);
    assert!(id < u64::MAX);
}

/// Second valid DEA number to confirm the check-digit algorithm with a
/// different set of digits: "BC2345671"
/// odd  = 2+4+6 = 12, even = 3+5+7 = 15
/// total = 12 + 2*15 = 42 → units digit 2 = d7 ✓
#[test]
fn test_controlled_alternative_valid_dea_number_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = make_dea_test_req(
        &env,
        &provider,
        &patient,
        true,
        Some(3),
        Some(String::from_str(&env, "BC2345672")),
    );

    let id = client.issue_prescription(&provider, &patient, &req);
    assert!(id < u64::MAX);
}

// ── #480: rate-limit tests ────────────────────────────────────────────────────

#[test]
fn test_rate_limit_exceeded_after_default_cap() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    // Set a tiny limit so we can exceed it quickly
    client.configure_allergy_check(&admin, &admin, &false);
    client.set_provider_limit(&admin, &provider, &2);

    let make_req = |n: u8| -> IssueRequest {
        IssueRequest {
            medication_name: String::from_str(&env, "Drug"),
            ndc_code: String::from_str(&env, "00000-1111"),
            dosage: String::from_str(&env, "10mg"),
            quantity: 10,
            days_supply: 10,
            refills_allowed: 0,
            instructions_hash: BytesN::from_array(&env, &[n; 32]),
            is_controlled: false,
            schedule: None,
            valid_until: env.ledger().timestamp() + 86_400,
            substitution_allowed: true,
            pharmacy_id: None,
            bypass_allergy_check: false,
            dea_number: None,
            bypass_reason_hash: None,
        }
    };

    client.issue_prescription(&provider, &patient, &make_req(1));
    client.issue_prescription(&provider, &patient, &make_req(2));
    // 3rd must be rejected
    let result = client.try_issue_prescription(&provider, &patient, &make_req(3));
    assert_eq!(result, Err(Ok(Error::RateLimitExceeded)));
}

#[test]
fn test_rate_limit_resets_after_window() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    client.configure_allergy_check(&admin, &admin, &false);
    client.set_provider_limit(&admin, &provider, &1);

    let make_req = |ts: u64| -> IssueRequest {
        IssueRequest {
            medication_name: String::from_str(&env, "Drug"),
            ndc_code: String::from_str(&env, "00000-2222"),
            dosage: String::from_str(&env, "10mg"),
            quantity: 10,
            days_supply: 10,
            refills_allowed: 0,
            instructions_hash: BytesN::from_array(&env, &[0; 32]),
            is_controlled: false,
            schedule: None,
            valid_until: ts + 86_400,
            substitution_allowed: true,
            pharmacy_id: None,
            bypass_allergy_check: false,
            dea_number: None,
            bypass_reason_hash: None,
        }
    };

    env.ledger().with_mut(|li| li.timestamp = 1_000);
    client.issue_prescription(&provider, &patient, &make_req(1_000));

    // Still in the same window — must be rejected
    let result = client.try_issue_prescription(&provider, &patient, &make_req(1_000));
    assert_eq!(result, Err(Ok(Error::RateLimitExceeded)));

    // Advance past the 24-hour window
    let new_ts = 1_000 + RATE_LIMIT_WINDOW_SECS + 1;
    env.ledger().with_mut(|li| li.timestamp = new_ts);
    // Window reset — should succeed now
    client.issue_prescription(&provider, &patient, &make_req(new_ts));
}

#[test]
fn test_admin_set_provider_limit_works() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    client.configure_allergy_check(&admin, &admin, &false);
    // Default limit is 100; set to 3
    client.set_provider_limit(&admin, &provider, &3);

    let make_req = |n: u8| -> IssueRequest {
        IssueRequest {
            medication_name: String::from_str(&env, "Drug"),
            ndc_code: String::from_str(&env, "00000-3333"),
            dosage: String::from_str(&env, "10mg"),
            quantity: 10,
            days_supply: 10,
            refills_allowed: 0,
            instructions_hash: BytesN::from_array(&env, &[n; 32]),
            is_controlled: false,
            schedule: None,
            valid_until: env.ledger().timestamp() + 86_400,
            substitution_allowed: true,
            pharmacy_id: None,
            bypass_allergy_check: false,
            dea_number: None,
            bypass_reason_hash: None,
        }
    };

    client.issue_prescription(&provider, &patient, &make_req(1));
    client.issue_prescription(&provider, &patient, &make_req(2));
    client.issue_prescription(&provider, &patient, &make_req(3));
    let result = client.try_issue_prescription(&provider, &patient, &make_req(4));
    assert_eq!(result, Err(Ok(Error::RateLimitExceeded)));
}

// ── #481: allergy bypass audit log tests ─────────────────────────────────────

#[test]
fn test_bypass_with_missing_reason_hash_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = IssueRequest {
        medication_name: String::from_str(&env, "Drug"),
        ndc_code: String::from_str(&env, "00000-4444"),
        dosage: String::from_str(&env, "10mg"),
        quantity: 10,
        days_supply: 10,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: env.ledger().timestamp() + 86_400,
        substitution_allowed: true,
        pharmacy_id: None,
        bypass_allergy_check: true,
        dea_number: None,
        bypass_reason_hash: None, // missing — must be rejected
    };

    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::MissingOverrideReason)));
}

#[test]
fn test_non_bypass_prescription_unaffected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let req = IssueRequest {
        medication_name: String::from_str(&env, "NormalDrug"),
        ndc_code: String::from_str(&env, "00000-5555"),
        dosage: String::from_str(&env, "5mg"),
        quantity: 10,
        days_supply: 10,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(&env, &[0; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: env.ledger().timestamp() + 86_400,
        substitution_allowed: true,
        pharmacy_id: None,
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None, // not required when bypass=false
    };

    // Must succeed without error
    let id = client.issue_prescription(&provider, &patient, &req);
    assert!(id < u64::MAX);
}

// ── #482: prescription template tests ────────────────────────────────────────

#[test]
fn test_create_and_issue_from_template() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let template = PrescriptionTemplate {
        medication_name: String::from_str(&env, "Metformin"),
        ndc_code: String::from_str(&env, "00093-7267-01"),
        dosage: String::from_str(&env, "500mg twice daily"),
        quantity: 60,
        days_supply: 30,
        refills_allowed: 11,
        instructions_hash: BytesN::from_array(&env, &[1u8; 32]),
        is_controlled: false,
        schedule: None,
        substitution_allowed: true,
        pharmacy_id: None,
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
    };

    let template_id = client.create_template(&provider, &template);

    let valid_until = env.ledger().timestamp() + 86_400 * 30;
    let rx_id = client.issue_from_template(&provider, &patient, &template_id, &valid_until);

    // Prescription was stored
    let rx: Prescription = env.as_contract(&contract_id, || {
        env.storage().persistent().get(&rx_id).unwrap()
    });
    assert_eq!(rx.medication_name, String::from_str(&env, "Metformin"));
    assert_eq!(rx.quantity, 60);
    assert_eq!(rx.refills_allowed, 11);
    assert_eq!(rx.valid_until, valid_until);
    assert!(matches!(rx.status, PrescriptionStatus::Issued));
}

#[test]
fn test_non_owning_provider_cannot_use_template() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let other = Address::generate(&env);
    let patient = Address::generate(&env);

    let template = PrescriptionTemplate {
        medication_name: String::from_str(&env, "Atorvastatin"),
        ndc_code: String::from_str(&env, "00071-0156-23"),
        dosage: String::from_str(&env, "20mg"),
        quantity: 30,
        days_supply: 30,
        refills_allowed: 5,
        instructions_hash: BytesN::from_array(&env, &[2u8; 32]),
        is_controlled: false,
        schedule: None,
        substitution_allowed: true,
        pharmacy_id: None,
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
    };

    let template_id = client.create_template(&owner, &template);
    let valid_until = env.ledger().timestamp() + 86_400;

    let result = client.try_issue_from_template(&other, &patient, &template_id, &valid_until);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}
