#![cfg(test)]

use super::*;
// Note the inclusion of 'Ledger' and 'Address' as traits here
use soroban_sdk::{
    Address, BytesN, Env, String,
    testutils::{Address as _, Ledger as _},
};

#[test]
fn test_prescription_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    // Updated from register_contract to register
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    let request = IssueRequest {
        medication_name: String::from_str(&env, "Amoxicillin"),
        ndc_code: String::from_str(&env, "0501-1234-01"),
        dosage: String::from_str(&env, "500mg"),
        quantity: 30,
        days_supply: 10,
        refills_allowed: 2,
        instructions_hash: BytesN::from_array(&env, &[0u8; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: 1000,
        substitution_allowed: true,
    };

    let prescription_id = client.issue_prescription(&provider, &patient, &request);
    assert_eq!(prescription_id, 0);

    // Test Dispensing
    client.dispense_prescription(
        &prescription_id,
        &pharmacy,
        &30,
        &String::from_str(&env, "LOT123"),
    );

    // Test Transfer
    let new_pharmacy = Address::generate(&env);
    client.transfer_prescription(&prescription_id, &pharmacy, &new_pharmacy);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")] // Error::Expired = 1
fn test_fail_expired_prescription() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    let request = IssueRequest {
        medication_name: String::from_str(&env, "Advil"),
        ndc_code: String::from_str(&env, "123"),
        dosage: String::from_str(&env, "200mg"),
        quantity: 10,
        days_supply: 5,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(&env, &[0u8; 32]),
        is_controlled: false,
        schedule: None,
        valid_until: 500,
        substitution_allowed: true,
    };

    let id = client.issue_prescription(&provider, &patient, &request);

    // This now works because Ledger trait is in scope
    env.ledger().with_mut(|li| {
        li.timestamp = 501;
    });

    client.dispense_prescription(&id, &pharmacy, &10, &String::from_str(&env, "LOT999"));
}
