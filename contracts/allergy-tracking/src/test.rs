#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Symbol, Vec,
};

fn create_test_env() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    // Set ledger timestamp to a reasonable value for testing
    env.ledger().with_mut(|li| {
        li.timestamp = 10000; // Set to 10000 so test timestamps work
    });

    let contract_id = env.register(AllergyTrackingContract, ());
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let admin = Address::generate(&env);

    (env, contract_id, patient, provider, admin)
}

#[test]
fn test_record_allergy_success() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reaction_types = Vec::new(&env);
    reaction_types.push_back(String::from_str(&env, "rash"));
    reaction_types.push_back(String::from_str(&env, "itching"));

    let allergy_id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Penicillin"),
        &Symbol::new(&env, "medication"),
        &reaction_types,
        &Symbol::new(&env, "moderate"),
        &Some(1000u64),
        &true,
    );

    assert_eq!(allergy_id, 0);

    let allergy = client.get_allergy(&allergy_id);
    assert_eq!(allergy.allergen, String::from_str(&env, "Penicillin"));
    assert_eq!(allergy.severity, Severity::Moderate);
    assert!(allergy.verified);
    assert_eq!(allergy.status, AllergyStatus::Active);
}

#[test]
fn test_record_multiple_allergies() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions1 = Vec::new(&env);
    reactions1.push_back(String::from_str(&env, "anaphylaxis"));

    let allergy_id1 = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Peanuts"),
        &Symbol::new(&env, "food"),
        &reactions1,
        &Symbol::new(&env, "life_threatening"),
        &None,
        &true,
    );

    let mut reactions2 = Vec::new(&env);
    reactions2.push_back(String::from_str(&env, "sneezing"));
    reactions2.push_back(String::from_str(&env, "watery eyes"));

    let allergy_id2 = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Pollen"),
        &Symbol::new(&env, "environmental"),
        &reactions2,
        &Symbol::new(&env, "mild"),
        &Some(2000u64),
        &true, // Changed to true so it's Active, not Suspected
    );

    assert_eq!(allergy_id1, 0);
    assert_eq!(allergy_id2, 1);

    let active_allergies = client.get_active_allergies(&patient, &provider);
    assert_eq!(active_allergies.len(), 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")] // Error::DuplicateAllergy = 7
fn test_duplicate_allergy_prevention() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Penicillin"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "moderate"),
        &None,
        &true,
    );

    // Try to record the same allergy again - should panic
    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Penicillin"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "moderate"),
        &None,
        &true,
    );
}

#[test]
fn test_update_allergy_severity() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "mild rash"));

    let allergy_id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Latex"),
        &Symbol::new(&env, "other"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );

    // Update severity
    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &Symbol::new(&env, "severe"),
        &String::from_str(&env, "Patient had severe reaction during procedure"),
    );

    let allergy = client.get_allergy(&allergy_id);
    assert_eq!(allergy.severity, Severity::Severe);

    // Check severity history
    let history = client.get_severity_history(&allergy_id);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().old_severity, Severity::Mild);
    assert_eq!(history.get(0).unwrap().new_severity, Severity::Severe);
}

#[test]
fn test_resolve_allergy() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "hives"));

    let allergy_id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Shellfish"),
        &Symbol::new(&env, "food"),
        &reactions,
        &Symbol::new(&env, "moderate"),
        &None,
        &false,
    );

    // Resolve the allergy
    client.resolve_allergy(
        &allergy_id,
        &provider,
        &5000u64,
        &String::from_str(&env, "False positive - patient tolerated shellfish"),
    );

    let allergy = client.get_allergy(&allergy_id);
    assert_eq!(allergy.status, AllergyStatus::Resolved);
    assert_eq!(allergy.resolution_date, Some(5000u64));

    // Active allergies should not include resolved ones
    let active_allergies = client.get_active_allergies(&patient, &provider);
    assert_eq!(active_allergies.len(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")] // Error::AlreadyResolved = 5
fn test_cannot_update_resolved_allergy() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "nausea"));

    let allergy_id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Aspirin"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );

    // Resolve the allergy
    client.resolve_allergy(
        &allergy_id,
        &provider,
        &3000u64,
        &String::from_str(&env, "Resolved"),
    );

    // Try to update severity of resolved allergy - should panic
    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &Symbol::new(&env, "severe"),
        &String::from_str(&env, "Should fail"),
    );
}

#[test]
fn test_check_drug_allergy_interaction_direct_match() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "anaphylaxis"));

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Amoxicillin"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "life_threatening"),
        &None,
        &true,
    );

    // Check for interaction with the same drug
    let warnings =
        client.check_drug_allergy_interaction(&patient, &String::from_str(&env, "Amoxicillin"));

    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings.get(0).unwrap().allergen,
        String::from_str(&env, "Amoxicillin")
    );
    assert_eq!(warnings.get(0).unwrap().severity, Severity::LifeThreatening);
}

#[test]
fn test_check_drug_allergy_interaction_no_match() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Penicillin"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "moderate"),
        &None,
        &true,
    );

    // Check for interaction with a different drug
    let warnings =
        client.check_drug_allergy_interaction(&patient, &String::from_str(&env, "Ibuprofen"));

    assert_eq!(warnings.len(), 0);
}

#[test]
fn test_cross_sensitivity_checking() {
    let (env, contract_id, patient, provider, admin) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    // Register cross-sensitivity between Penicillin and Amoxicillin
    client.register_cross_sensitivity(
        &admin,
        &String::from_str(&env, "Penicillin"),
        &String::from_str(&env, "Amoxicillin"),
    );

    // Record allergy to Penicillin
    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "severe rash"));

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Penicillin"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "severe"),
        &None,
        &true,
    );

    // Check for interaction with Amoxicillin (cross-sensitive)
    let warnings =
        client.check_drug_allergy_interaction(&patient, &String::from_str(&env, "Amoxicillin"));

    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings.get(0).unwrap().allergen,
        String::from_str(&env, "Penicillin")
    );
}

#[test]
fn test_multiple_severity_updates() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "itching"));

    let allergy_id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Sulfa drugs"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );

    // First update
    env.ledger().with_mut(|li| li.timestamp = 1000);
    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &Symbol::new(&env, "moderate"),
        &String::from_str(&env, "Increased reaction severity"),
    );

    // Second update
    env.ledger().with_mut(|li| li.timestamp = 2000);
    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &Symbol::new(&env, "severe"),
        &String::from_str(&env, "Patient hospitalized"),
    );

    let history = client.get_severity_history(&allergy_id);
    assert_eq!(history.len(), 2);
    assert_eq!(history.get(0).unwrap().new_severity, Severity::Moderate);
    assert_eq!(history.get(1).unwrap().new_severity, Severity::Severe);
}

#[test]
fn test_get_active_allergies_filters_resolved() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    // Record three allergies
    let id1 = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Allergy1"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Allergy2"),
        &Symbol::new(&env, "food"),
        &reactions,
        &Symbol::new(&env, "moderate"),
        &None,
        &true,
    );

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Allergy3"),
        &Symbol::new(&env, "environmental"),
        &reactions,
        &Symbol::new(&env, "severe"),
        &None,
        &true,
    );

    // Resolve one allergy
    client.resolve_allergy(
        &id1,
        &provider,
        &4000u64,
        &String::from_str(&env, "Resolved"),
    );

    // Should only return 2 active allergies
    let active = client.get_active_allergies(&patient, &provider);
    assert_eq!(active.len(), 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // Error::InvalidSeverity = 3
fn test_invalid_severity_symbol() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Test"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "invalid_severity"),
        &None,
        &true,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")] // Error::InvalidAllergenType = 4
fn test_invalid_allergen_type_symbol() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Test"),
        &Symbol::new(&env, "invalid_type"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")] // Error::AllergyNotFound = 1
fn test_allergy_not_found() {
    let (env, contract_id, _, _, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    client.get_allergy(&999);
}

#[test]
fn test_comprehensive_workflow() {
    let (env, contract_id, patient, provider, admin) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    // Setup cross-sensitivities
    client.register_cross_sensitivity(
        &admin,
        &String::from_str(&env, "Penicillin"),
        &String::from_str(&env, "Ampicillin"),
    );

    // Record initial allergy
    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "hives"));
    reactions.push_back(String::from_str(&env, "swelling"));

    let allergy_id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "Penicillin"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "moderate"),
        &Some(1000u64),
        &true,
    );

    // Update severity after new reaction
    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &Symbol::new(&env, "severe"),
        &String::from_str(&env, "Patient had severe reaction"),
    );

    // Check for drug interactions
    let warnings1 =
        client.check_drug_allergy_interaction(&patient, &String::from_str(&env, "Penicillin"));
    assert_eq!(warnings1.len(), 1);

    let warnings2 =
        client.check_drug_allergy_interaction(&patient, &String::from_str(&env, "Ampicillin"));
    assert_eq!(warnings2.len(), 1);

    // Verify active allergies
    let active = client.get_active_allergies(&patient, &provider);
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap().severity, Severity::Severe);

    // Check history
    let history = client.get_severity_history(&allergy_id);
    assert_eq!(history.len(), 1);
}

// ==================== NEW VALIDATION TESTS ====================

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // Error::InvalidAllergen = 8
fn test_empty_allergen_rejected() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, ""), // Empty allergen
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")] // Error::AllergenTooLong = 9
fn test_long_allergen_rejected() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    // Create allergen name longer than MAX_ALLERGEN_LENGTH (100)
    let long_allergen = "A".repeat(101);

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, &long_allergen),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")] // Error::InvalidTimestamp = 10
fn test_zero_timestamp_rejected() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "TestAllergen"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &Some(0u64), // Zero timestamp
        &true,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")] // Error::InvalidTimestamp = 10
fn test_future_timestamp_rejected() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    // Ledger timestamp is 10000, try to use 20000
    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "TestAllergen"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &Some(20000u64), // Future timestamp
        &true,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // Error::ReasonTooLong = 11
fn test_long_reason_rejected() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    let allergy_id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "TestAllergen"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );

    // Create reason longer than MAX_REASON_LENGTH (500)
    let long_reason = "A".repeat(501);

    client.update_allergy_severity(
        &allergy_id,
        &provider,
        &Symbol::new(&env, "severe"),
        &String::from_str(&env, &long_reason),
    );
}

#[test]
fn test_valid_allergen_length_accepted() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    // Test minimum length (1 character)
    let allergy_id1 = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "A"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );
    assert_eq!(allergy_id1, 0);

    // Test maximum length (100 characters)
    let max_allergen = "B".repeat(100);
    let allergy_id2 = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, &max_allergen),
        &Symbol::new(&env, "food"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );
    assert_eq!(allergy_id2, 1);
}

#[test]
fn test_delete_record_soft_deletes_and_blocks_get_record() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "rash"));

    let record_id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "SoftDeleteDrug"),
        &Symbol::new(&env, "medication"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );

    client.delete_record(&record_id, &provider);

    let deleted = client.try_get_record(&record_id);
    assert!(matches!(deleted, Err(Ok(Error::AllergyNotFound))));
}

#[test]
fn test_get_all_records_excludes_deleted_by_default() {
    let (env, contract_id, patient, provider, _) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    let id1 = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "KeepMe"),
        &Symbol::new(&env, "food"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );

    client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "DeleteMe"),
        &Symbol::new(&env, "food"),
        &reactions,
        &Symbol::new(&env, "moderate"),
        &None,
        &true,
    );

    client.delete_record(&1u64, &patient);

    let visible = client.get_all_records(&patient, &provider, &false);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible.get(0).unwrap().allergy_id, id1);
}

#[test]
fn test_include_deleted_requires_admin() {
    let (env, contract_id, patient, provider, admin) = create_test_env();
    let client = AllergyTrackingContractClient::new(&env, &contract_id);

    client.initialize(&admin);

    let mut reactions = Vec::new(&env);
    reactions.push_back(String::from_str(&env, "reaction"));

    let id = client.record_allergy(
        &patient,
        &provider,
        &String::from_str(&env, "AdminView"),
        &Symbol::new(&env, "other"),
        &reactions,
        &Symbol::new(&env, "mild"),
        &None,
        &true,
    );
    client.delete_record(&id, &provider);

    let non_admin_attempt = client.try_get_all_records(&patient, &provider, &true);
    assert_eq!(non_admin_attempt, Err(Ok(Error::Unauthorized)));

    let admin_view = client.get_all_records(&patient, &admin, &true);
    assert_eq!(admin_view.len(), 1);
    assert!(admin_view.get(0).unwrap().is_deleted);
}
