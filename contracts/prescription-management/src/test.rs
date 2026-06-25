#![cfg(test)]

use super::*;
// Note the inclusion of 'Ledger' and 'Address' as traits here
use soroban_sdk::{
    Address, BytesN, Env, String, Symbol,
    testutils::{Address as _, Ledger as _},
    vec,
};

#[path = "test_enhanced.rs"]
mod test_enhanced;

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
        pharmacy_id: Some(pharmacy.clone()),
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
    };

    let prescription_id = client.issue_prescription(&provider, &patient, &request);
    assert_eq!(prescription_id, 0);

    // Test Dispensing
    let dispense = DispenseRequest {
        prescription_id,
        quantity: 10,
        lot: String::from_str(&env, "LOT123"),
        expires_at: 2000,
        ndc_code: String::from_str(&env, "0501-1234-01"),
    };
    client.dispense_prescription(&dispense, &pharmacy);

    // Test Transfer
    let new_pharmacy = Address::generate(&env);
    let transfer = TransferRequest {
        prescription_id,
        to_pharmacy: new_pharmacy,
        transfer_reason: String::from_str(&env, "patient_request"),
        urgency: Symbol::new(&env, "routine"),
    };
    client.transfer_prescription(&transfer, &pharmacy);
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
        pharmacy_id: Some(pharmacy.clone()),
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
    };

    let id = client.issue_prescription(&provider, &patient, &request);

    // This now works because Ledger trait is in scope
    env.ledger().with_mut(|li| {
        li.timestamp = 501;
    });

    let dispense = DispenseRequest {
        prescription_id: id,
        quantity: 10,
        lot: String::from_str(&env, "LOT999"),
        expires_at: 2000,
        ndc_code: String::from_str(&env, "123"),
    };
    client.dispense_prescription(&dispense, &pharmacy);
}

// ── #478: MAX_ACTIVE_PRESCRIPTIONS cap ──────────────────────────────────────

fn max_cap_request(env: &Env, pharmacy: &Address, valid_until: u64) -> IssueRequest {
    IssueRequest {
        medication_name: String::from_str(env, "Amoxicillin"),
        ndc_code: String::from_str(env, "0501-1234-01"),
        dosage: String::from_str(env, "500mg"),
        quantity: 30,
        days_supply: 10,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(env, &[0u8; 32]),
        is_controlled: false,
        schedule: None,
        valid_until,
        substitution_allowed: true,
        pharmacy_id: Some(pharmacy.clone()),
        bypass_allergy_check: false,
        dea_number: None,
    }
}

#[test]
fn test_issuing_beyond_max_active_prescriptions_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    for _ in 0..MAX_ACTIVE_PRESCRIPTIONS {
        let req = max_cap_request(&env, &pharmacy, 10_000_000);
        client.issue_prescription(&provider, &patient, &req);
    }

    // The (MAX_ACTIVE_PRESCRIPTIONS + 1)th prescription for the same patient is rejected.
    let req = max_cap_request(&env, &pharmacy, 10_000_000);
    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::TooManyActivePrescriptions)));

    // A different patient is unaffected by this patient's cap.
    let other_patient = Address::generate(&env);
    let req = max_cap_request(&env, &pharmacy, 10_000_000);
    client.issue_prescription(&provider, &other_patient, &req);
}

#[test]
fn test_issuing_succeeds_after_a_prescription_is_fully_dispensed() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    let mut first_id = 0u64;
    for i in 0..MAX_ACTIVE_PRESCRIPTIONS {
        let req = max_cap_request(&env, &pharmacy, 10_000_000);
        let id = client.issue_prescription(&provider, &patient, &req);
        if i == 0 {
            first_id = id;
        }
    }

    // At the cap: the next issuance is rejected.
    let req = max_cap_request(&env, &pharmacy, 10_000_000);
    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::TooManyActivePrescriptions)));

    // Fully dispense one existing prescription, freeing a slot.
    client.dispense_prescription(
        &DispenseRequest {
            prescription_id: first_id,
            quantity: 30,
            lot: String::from_str(&env, "LOT-FULL"),
            expires_at: 20_000_000,
            ndc_code: String::from_str(&env, "0501-1234-01"),
        },
        &pharmacy,
    );

    // A new prescription can now be issued for the same patient.
    let req = max_cap_request(&env, &pharmacy, 10_000_000);
    client.issue_prescription(&provider, &patient, &req);
}

#[test]
fn test_issuing_succeeds_after_a_prescription_expires() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let provider = Address::generate(&env);
    let patient = Address::generate(&env);
    let pharmacy = Address::generate(&env);

    // One prescription with a short validity window; the rest valid for a long time.
    let short_req = max_cap_request(&env, &pharmacy, 100);
    client.issue_prescription(&provider, &patient, &short_req);
    for _ in 1..MAX_ACTIVE_PRESCRIPTIONS {
        let req = max_cap_request(&env, &pharmacy, 10_000_000);
        client.issue_prescription(&provider, &patient, &req);
    }

    // At the cap: the next issuance is rejected.
    let req = max_cap_request(&env, &pharmacy, 10_000_000);
    let result = client.try_issue_prescription(&provider, &patient, &req);
    assert_eq!(result, Err(Ok(Error::TooManyActivePrescriptions)));

    // Advance past the short prescription's valid_until (expiry is exclusive: expired at >=).
    env.ledger().with_mut(|li| {
        li.timestamp = 100;
    });

    // A new prescription can now be issued: the expired one no longer counts.
    let req = max_cap_request(&env, &pharmacy, 10_000_000);
    client.issue_prescription(&provider, &patient, &req);
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

#[test]
fn test_registry_governance_authorizes_writers_and_blocks_legacy_updates() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let writer = Address::generate(&env);
    let outsider = Address::generate(&env);
    let med = String::from_str(&env, "12345-0001");

    client.initialize_registry_governance(&admin);
    client.add_registry_writer(&admin, &writer);

    let legacy = client.try_register_medication(
        &med,
        &String::from_str(&env, "Governed Drug"),
        &vec![&env],
        &Symbol::new(&env, "classg"),
        &BytesN::from_array(&env, &[9u8; 32]),
    );
    assert_eq!(legacy, Err(Ok(Error::RegistryGoverned)));

    let unauthorized = client.try_register_medication_by(
        &outsider,
        &med,
        &String::from_str(&env, "Governed Drug"),
        &vec![&env],
        &Symbol::new(&env, "classg"),
        &BytesN::from_array(&env, &[9u8; 32]),
    );
    assert_eq!(unauthorized, Err(Ok(Error::Unauthorized)));

    client.register_medication_by(
        &writer,
        &med,
        &String::from_str(&env, "Governed Drug"),
        &vec![&env],
        &Symbol::new(&env, "classg"),
        &BytesN::from_array(&env, &[9u8; 32]),
    );
}

#[test]
fn test_high_impact_interaction_requires_proposal_and_snapshot_is_versioned() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let writer = Address::generate(&env);
    let med1 = String::from_str(&env, "55555-1001");
    let med2 = String::from_str(&env, "55555-1002");

    client.initialize_registry_governance(&admin);
    client.add_registry_writer(&admin, &writer);
    client.register_medication_by(
        &writer,
        &med1,
        &String::from_str(&env, "Drug One"),
        &vec![&env],
        &Symbol::new(&env, "classa"),
        &BytesN::from_array(&env, &[1u8; 32]),
    );
    client.register_medication_by(
        &writer,
        &med2,
        &String::from_str(&env, "Drug Two"),
        &vec![&env],
        &Symbol::new(&env, "classb"),
        &BytesN::from_array(&env, &[2u8; 32]),
    );

    let direct = client.try_add_interaction_by(
        &writer,
        &med1,
        &med2,
        &Symbol::new(&env, "major"),
        &Symbol::new(&env, "pk"),
        &String::from_str(&env, "High impact interaction"),
        &String::from_str(&env, "Admin approval required"),
    );
    assert_eq!(direct, Err(Ok(Error::HighImpactRequiresProposal)));

    let proposal_id = client.propose_interaction_update(
        &writer,
        &med1,
        &med2,
        &Symbol::new(&env, "major"),
        &Symbol::new(&env, "pk"),
        &String::from_str(&env, "High impact interaction"),
        &String::from_str(&env, "Admin approval required"),
    );
    client.approve_registry_proposal(&admin, &proposal_id);

    let snapshot_version = client.create_catalog_snapshot(&admin);
    let snapshot = client.get_catalog_snapshot(&snapshot_version);
    assert_eq!(snapshot.version, 1);
    assert_eq!(snapshot.medication_count, 2);
    assert_eq!(snapshot.interaction_count, 1);

    let duplicate_approval = client.try_approve_registry_proposal(&admin, &proposal_id);
    assert_eq!(duplicate_approval, Err(Ok(Error::ProposalAlreadyFinalized)));
}

// ── UTC midnight boundary tests (#381) ───────────────────────────────────────
//
// valid_until is an EXCLUSIVE upper bound:
//   timestamp <  valid_until  → prescription is valid
//   timestamp == valid_until  → prescription is expired
//   timestamp >  valid_until  → prescription is expired
//
// Midnight boundaries used below are real UTC epoch values:
//   2024-01-01 00:00:00 UTC = 1_704_067_200
//   2024-06-15 00:00:00 UTC = 1_718_409_600

const MIDNIGHT_JAN1: u64 = 1_704_067_200;
const MIDNIGHT_JUN15: u64 = 1_718_409_600;

fn make_client(env: &Env) -> (PrescriptionContractClient<'static>, Address, Address, Address) {
    let contract_id = env.register(PrescriptionContract, ());
    let client = PrescriptionContractClient::new(env, &contract_id);
    let provider = Address::generate(env);
    let patient = Address::generate(env);
    let pharmacy = Address::generate(env);
    (client, provider, patient, pharmacy)
}

fn issue_at(
    env: &Env,
    client: &PrescriptionContractClient,
    provider: &Address,
    patient: &Address,
    pharmacy: &Address,
    valid_until: u64,
) -> u64 {
    // Set ledger to 1 second before valid_until so must_be_future and
    // within_validity_window (1-year cap) both pass during issuance.
    env.ledger().with_mut(|li| li.timestamp = valid_until - 1);
    let req = IssueRequest {
        medication_name: String::from_str(env, "TestDrug"),
        ndc_code: String::from_str(env, "00000-0001"),
        dosage: String::from_str(env, "10mg"),
        quantity: 10,
        days_supply: 30,
        refills_allowed: 0,
        instructions_hash: BytesN::from_array(env, &[0u8; 32]),
        is_controlled: false,
        schedule: None,
        valid_until,
        substitution_allowed: true,
        pharmacy_id: Some(pharmacy.clone()),
        bypass_allergy_check: false,
        dea_number: None,
        bypass_reason_hash: None,
    };
    client.issue_prescription(provider, patient, &req)
}

/// One second before midnight: prescription must be valid.
#[test]
fn test_dispense_one_second_before_midnight_is_valid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, provider, patient, pharmacy) = make_client(&env);
    let id = issue_at(&env, &client, &provider, &patient, &pharmacy, MIDNIGHT_JAN1);

    env.ledger().with_mut(|li| li.timestamp = MIDNIGHT_JAN1 - 1);
    let req = DispenseRequest {
        prescription_id: id,
        quantity: 1,
        lot: String::from_str(&env, "LOT1"),
        expires_at: MIDNIGHT_JAN1 + 86400,
        ndc_code: String::from_str(&env, "00000-0001"),
    };
    client.dispense_prescription(&req, &pharmacy); // must not panic
}

/// Exactly at midnight (timestamp == valid_until): prescription must be expired.
#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_dispense_exactly_at_midnight_is_expired() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, provider, patient, pharmacy) = make_client(&env);
    let id = issue_at(&env, &client, &provider, &patient, &pharmacy, MIDNIGHT_JAN1);

    env.ledger().with_mut(|li| li.timestamp = MIDNIGHT_JAN1);
    let req = DispenseRequest {
        prescription_id: id,
        quantity: 1,
        lot: String::from_str(&env, "LOT1"),
        expires_at: MIDNIGHT_JAN1 + 86400,
        ndc_code: String::from_str(&env, "00000-0001"),
    };
    client.dispense_prescription(&req, &pharmacy);
}

/// One second after midnight: prescription must be expired.
#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_dispense_one_second_after_midnight_is_expired() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, provider, patient, pharmacy) = make_client(&env);
    let id = issue_at(&env, &client, &provider, &patient, &pharmacy, MIDNIGHT_JAN1);

    env.ledger().with_mut(|li| li.timestamp = MIDNIGHT_JAN1 + 1);
    let req = DispenseRequest {
        prescription_id: id,
        quantity: 1,
        lot: String::from_str(&env, "LOT1"),
        expires_at: MIDNIGHT_JAN1 + 86400,
        ndc_code: String::from_str(&env, "00000-0001"),
    };
    client.dispense_prescription(&req, &pharmacy);
}

/// Transfer: exactly at midnight is expired.
#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_transfer_exactly_at_midnight_is_expired() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, provider, patient, pharmacy) = make_client(&env);
    let id = issue_at(&env, &client, &provider, &patient, &pharmacy, MIDNIGHT_JUN15);

    env.ledger().with_mut(|li| li.timestamp = MIDNIGHT_JUN15);
    let req = TransferRequest {
        prescription_id: id,
        to_pharmacy: Address::generate(&env),
        transfer_reason: String::from_str(&env, "patient_request"),
        urgency: Symbol::new(&env, "routine"),
    };
    client.transfer_prescription(&req, &pharmacy);
}

/// Transfer: one second before midnight is valid.
#[test]
fn test_transfer_one_second_before_midnight_is_valid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, provider, patient, pharmacy) = make_client(&env);
    let id = issue_at(&env, &client, &provider, &patient, &pharmacy, MIDNIGHT_JUN15);

    env.ledger().with_mut(|li| li.timestamp = MIDNIGHT_JUN15 - 1);
    let req = TransferRequest {
        prescription_id: id,
        to_pharmacy: Address::generate(&env),
        transfer_reason: String::from_str(&env, "patient_request"),
        urgency: Symbol::new(&env, "routine"),
    };
    client.transfer_prescription(&req, &pharmacy); // must not panic
}
