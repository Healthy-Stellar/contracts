#![cfg(test)]
#![allow(deprecated)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};

#[test]
fn test_full_claim_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalClaimsSystem);
    let client = MedicalClaimsSystemClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let insurance_admin = Address::generate(&env);

    let mut services = Vec::new(&env);
    services.push_back(ServiceLine {
        procedure_code: String::from_str(&env, "99213"),
        modifier: None,
        quantity: 1,
        charge_amount: 15000, // $150.00
        diagnosis_pointers: Vec::new(&env),
    });

    // 1. Submit Claim
    let claim_id = client.submit_claim(
        &provider_id,
        &patient_id,
        &12345,      // policy
        &1690000000, // date
        &services,
        &Vec::new(&env),                     // diagnoses
        &BytesN::from_array(&env, &[0; 32]), // hash
        &15000,
    );
    assert_eq!(claim_id, 1);

    // 2. Adjudicate Claim
    let mut approved_lines = Vec::new(&env);
    approved_lines.push_back(1);

    client.adjudicate_claim(
        &claim_id,
        &insurance_admin,
        &approved_lines,
        &Vec::new(&env), // no denials
        &10000,          // Approved $100.00
        &2000,           // Patient owes $20.00
    );

    // 3. Process Insurance Payment
    client.process_payment(
        &claim_id,
        &insurance_admin,
        &8000, // Ins pays $80.00 (100 - 20)
        &1690100000,
        &String::from_str(&env, "REF_123"),
    );

    // 4. Apply Patient Payment
    client.apply_patient_payment(&claim_id, &patient_id, &2000, &1690200000);

    // State cannot be verified directly without getters, but operations shouldn't panic.
    // If we try to appeal a Paid claim, it should fail
    let res = client.try_appeal_denial(
        &claim_id,
        &provider_id,
        &1,
        &BytesN::from_array(&env, &[0; 32]),
    );
    assert!(res.is_err()); // InvalidStateTransition
}

#[test]
fn test_appeal_workflow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, MedicalClaimsSystem);
    let client = MedicalClaimsSystemClient::new(&env, &contract_id);

    let provider_id = Address::generate(&env);
    let patient_id = Address::generate(&env);
    let insurance_admin = Address::generate(&env);

    let mut services = Vec::new(&env);
    services.push_back(ServiceLine {
        procedure_code: String::from_str(&env, "99214"),
        modifier: None,
        quantity: 1,
        charge_amount: 25000,
        diagnosis_pointers: Vec::new(&env),
    });

    let claim_id = client.submit_claim(
        &provider_id,
        &patient_id,
        &12345,
        &1690000000,
        &services,
        &Vec::new(&env),
        &BytesN::from_array(&env, &[1; 32]),
        &25000,
    );

    // Adjudicate: Deny
    let mut denials = Vec::new(&env);
    denials.push_back(DenialInfo {
        line_number: 1,
        denial_code: String::from_str(&env, "CO-50"),
        denial_reason: String::from_str(&env, "Not deemed medically necessary"),
        is_appealable: true,
    });

    client.adjudicate_claim(
        &claim_id,
        &insurance_admin,
        &Vec::new(&env), // none approved
        &denials,
        &0,
        &0,
    );

    // Appeal level 1
    client.appeal_denial(
        &claim_id,
        &provider_id,
        &1,
        &BytesN::from_array(&env, &[2; 32]),
    );

    // Try invalid appeal levels
    let res1 = client.try_appeal_denial(
        &claim_id,
        &provider_id,
        &1, // already at level 1
        &BytesN::from_array(&env, &[2; 32]),
    );
    assert!(res1.is_err()); // InvalidStateTransition or InvalidAppealLevel

    // Re-adjudicate after appeal
    client.adjudicate_claim(
        &claim_id,
        &insurance_admin,
        &Vec::new(&env),
        &denials,
        &0,
        &0,
    );

    // Appeal level 2
    client.appeal_denial(
        &claim_id,
        &provider_id,
        &2,
        &BytesN::from_array(&env, &[3; 32]),
    );

    // Re-adjudicate
    client.adjudicate_claim(
        &claim_id,
        &insurance_admin,
        &Vec::new(&env),
        &denials,
        &0,
        &0,
    );

    // Appeal level 3
    client.appeal_denial(
        &claim_id,
        &provider_id,
        &3,
        &BytesN::from_array(&env, &[4; 32]),
    );
}
