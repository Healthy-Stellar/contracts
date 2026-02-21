#![cfg(test)]

use super::*;
// Note the inclusion of 'Ledger' and 'Address' as traits here
use soroban_sdk::{
    Address, BytesN, Env, String, Symbol,
    testutils::{Address as _, Ledger as _},
    vec,
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

#[test]
fn test_multi_drug_interactions_with_severity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let med_new = String::from_str(&env, "11111-0001");
    let med_current_1 = String::from_str(&env, "22222-0002");
    let med_current_2 = String::from_str(&env, "33333-0003");

    client.register_medication(
        &med_new,
        &String::from_str(&env, "Warfarin"),
        &vec![&env, String::from_str(&env, "Coumadin")],
        &Symbol::new(&env, "anticoag"),
        &BytesN::from_array(&env, &[1u8; 32]),
    );
    client.register_medication(
        &med_current_1,
        &String::from_str(&env, "Aspirin"),
        &vec![&env],
        &Symbol::new(&env, "nsaid"),
        &BytesN::from_array(&env, &[2u8; 32]),
    );
    client.register_medication(
        &med_current_2,
        &String::from_str(&env, "Omeprazole"),
        &vec![&env, String::from_str(&env, "Prilosec")],
        &Symbol::new(&env, "ppi"),
        &BytesN::from_array(&env, &[3u8; 32]),
    );

    client.add_interaction(
        &med_new,
        &med_current_1,
        &Symbol::new(&env, "major"),
        &Symbol::new(&env, "pk"),
        &String::from_str(&env, "Increased bleeding risk"),
        &String::from_str(&env, "Avoid combination or monitor INR closely"),
    );
    client.add_interaction(
        &med_new,
        &med_current_2,
        &Symbol::new(&env, "minor"),
        &Symbol::new(&env, "absorp"),
        &String::from_str(&env, "Slight change in absorption"),
        &String::from_str(&env, "Space administration by 2 hours"),
    );

    let current = vec![&env, med_current_1.clone(), med_current_2.clone()];
    let warnings = client.check_interactions(&patient, &med_new, &current);

    assert_eq!(warnings.len(), 2);

    let major = warnings.get(0).unwrap();
    assert_eq!(major.severity, Symbol::new(&env, "major"));
    assert!(major.documentation_required);

    let minor = warnings.get(1).unwrap();
    assert_eq!(minor.severity, Symbol::new(&env, "minor"));
    assert!(!minor.documentation_required);
}

#[test]
fn test_drug_allergy_and_contraindications() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let patient = Address::generate(&env);
    let med = String::from_str(&env, "44444-1000");

    client.register_medication(
        &med,
        &String::from_str(&env, "Penicillin"),
        &vec![&env, String::from_str(&env, "Pen-V")],
        &Symbol::new(&env, "abx"),
        &BytesN::from_array(&env, &[4u8; 32]),
    );

    client.set_patient_allergies(&patient, &vec![&env, String::from_str(&env, "Penicillin")]);

    let allergy = client.check_allergy_interaction(&patient, &med);
    assert_eq!(allergy.len(), 1);
    let warning = allergy.get(0).unwrap();
    assert_eq!(warning.severity, Symbol::new(&env, "contraindicated"));
    assert_eq!(warning.interaction_type, Symbol::new(&env, "allergy"));
    assert!(warning.documentation_required);

    client.set_patient_conditions(&patient, &vec![&env, String::from_str(&env, "pregnancy")]);
    client.set_medication_contraindications(
        &med,
        &vec![
            &env,
            String::from_str(&env, "pregnancy"),
            String::from_str(&env, "renal_failure"),
        ],
    );

    let found = client.get_contraindications(
        &patient,
        &med,
        &vec![&env, String::from_str(&env, "renal_failure")],
    );

    assert_eq!(found.len(), 2);
}

#[test]
fn test_override_interaction_warning_requires_justification() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);

    let med1 = String::from_str(&env, "55555-0001");
    let med2 = String::from_str(&env, "55555-0002");

    client.register_medication(
        &med1,
        &String::from_str(&env, "Drug A"),
        &vec![&env],
        &Symbol::new(&env, "classa"),
        &BytesN::from_array(&env, &[5u8; 32]),
    );
    client.register_medication(
        &med2,
        &String::from_str(&env, "Drug B"),
        &vec![&env],
        &Symbol::new(&env, "classb"),
        &BytesN::from_array(&env, &[6u8; 32]),
    );

    client.add_interaction(
        &med1,
        &med2,
        &Symbol::new(&env, "contraindicated"),
        &Symbol::new(&env, "pd"),
        &String::from_str(&env, "Severe adverse reaction"),
        &String::from_str(&env, "Do not co-administer"),
    );

    let err = client.try_override_interaction_warning(
        &provider,
        &patient,
        &med1,
        &1u64,
        &String::from_str(&env, ""),
    );
    assert_eq!(err, Err(Ok(Error::MissingOverrideReason)));

    client.override_interaction_warning(
        &provider,
        &patient,
        &med1,
        &1u64,
        &String::from_str(&env, "Benefit outweighs risk with monitoring"),
    );
}

#[test]
fn test_invalid_severity_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let med1 = String::from_str(&env, "99999-0001");
    let med2 = String::from_str(&env, "99999-0002");

    client.register_medication(
        &med1,
        &String::from_str(&env, "Drug X"),
        &vec![&env],
        &Symbol::new(&env, "classx"),
        &BytesN::from_array(&env, &[7u8; 32]),
    );
    client.register_medication(
        &med2,
        &String::from_str(&env, "Drug Y"),
        &vec![&env],
        &Symbol::new(&env, "classy"),
        &BytesN::from_array(&env, &[8u8; 32]),
    );

    let result = client.try_add_interaction(
        &med1,
        &med2,
        &Symbol::new(&env, "critical"),
        &Symbol::new(&env, "pk"),
        &String::from_str(&env, "Unknown"),
        &String::from_str(&env, "Unknown"),
    );

    assert_eq!(result, Err(Ok(Error::InvalidSeverity)));
}
