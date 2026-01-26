#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

#[test]
fn test_register_hospital() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "General Hospital"),
        &String::from_str(&env, "123 Main St, New York, NY"),
        &String::from_str(&env, "Services: ER, Surgery, Cardiology"),
    );

    let hospital = client.get_hospital(&hospital_wallet);

    assert_eq!(hospital.name, String::from_str(&env, "General Hospital"));
    assert_eq!(
        hospital.location,
        String::from_str(&env, "123 Main St, New York, NY")
    );
    assert_eq!(
        hospital.metadata,
        String::from_str(&env, "Services: ER, Surgery, Cardiology")
    );
}

#[test]
fn test_update_hospital() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "456 Oak Ave"),
        &String::from_str(&env, "General Services"),
    );

    client.update_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Services: ER, ICU, Pediatrics, Oncology"),
    );

    let hospital = client.get_hospital(&hospital_wallet);

    assert_eq!(
        hospital.metadata,
        String::from_str(&env, "Services: ER, ICU, Pediatrics, Oncology")
    );
    assert_eq!(hospital.name, String::from_str(&env, "City Hospital"));
}

#[test]
#[should_panic(expected = "Hospital already registered")]
fn test_duplicate_registration() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Test Hospital"),
        &String::from_str(&env, "Test Location"),
        &String::from_str(&env, "Test Metadata"),
    );

    // Attempt to register again
    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Test Hospital"),
        &String::from_str(&env, "Test Location"),
        &String::from_str(&env, "Test Metadata"),
    );
}

#[test]
#[should_panic(expected = "Hospital not found")]
fn test_get_nonexistent_hospital() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    client.get_hospital(&hospital_wallet);
}

#[test]
#[should_panic(expected = "Hospital not found")]
fn test_update_nonexistent_hospital() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.update_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Updated Metadata"),
    );
}

#[test]
fn test_hospital_config_flow() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HospitalRegistry);
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Regional Medical Center"),
        &String::from_str(&env, "789 Pine Rd"),
        &String::from_str(&env, "Accredited, trauma level II"),
    );

    let mut departments: Vec<Department> = Vec::new(&env);
    departments.push_back(Department {
        name: String::from_str(&env, "Emergency"),
        head: String::from_str(&env, "Dr. Smith"),
        contact: String::from_str(&env, "er@rmc.org"),
    });

    let mut locations: Vec<Location> = Vec::new(&env);
    locations.push_back(Location {
        name: String::from_str(&env, "Main Campus"),
        address: String::from_str(&env, "789 Pine Rd"),
        metadata: String::from_str(&env, "24/7"),
    });

    let mut equipment: Vec<EquipmentResource> = Vec::new(&env);
    equipment.push_back(EquipmentResource {
        name: String::from_str(&env, "MRI"),
        quantity: 2,
        status: String::from_str(&env, "operational"),
        metadata: String::from_str(&env, "Siemens Aera"),
    });

    let mut policies: Vec<PolicyProcedure> = Vec::new(&env);
    policies.push_back(PolicyProcedure {
        title: String::from_str(&env, "Infection Control"),
        version: String::from_str(&env, "v3"),
        details: String::from_str(&env, "Hand hygiene and PPE policy"),
    });

    let mut channels: Vec<String> = Vec::new(&env);
    channels.push_back(String::from_str(&env, "sms"));
    channels.push_back(String::from_str(&env, "email"));

    let mut alerts: Vec<AlertSetting> = Vec::new(&env);
    alerts.push_back(AlertSetting {
        alert_type: String::from_str(&env, "code_blue"),
        enabled: true,
        channels,
        escalation_contact: String::from_str(&env, "+1-555-0100"),
    });

    let mut plan_codes: Vec<String> = Vec::new(&env);
    plan_codes.push_back(String::from_str(&env, "HMO-101"));
    plan_codes.push_back(String::from_str(&env, "PPO-202"));

    let mut insurance_providers: Vec<InsuranceProviderConfig> = Vec::new(&env);
    insurance_providers.push_back(InsuranceProviderConfig {
        provider_name: String::from_str(&env, "Acme Health"),
        plan_codes,
        billing_contact: String::from_str(&env, "billing@acmehealth.com"),
        metadata: String::from_str(&env, "EDI enabled"),
    });

    let billing = BillingConfig {
        currency: String::from_str(&env, "USD"),
        payment_terms: String::from_str(&env, "Net 30"),
        tax_id: String::from_str(&env, "TAX-001"),
    };

    let mut protocols: Vec<EmergencyProtocol> = Vec::new(&env);
    protocols.push_back(EmergencyProtocol {
        protocol_name: String::from_str(&env, "Fire"),
        description: String::from_str(&env, "Evacuate wing A"),
        last_updated: 1700000000,
        contact: String::from_str(&env, "safety@rmc.org"),
    });

    let config = HospitalConfig {
        departments: departments.clone(),
        locations: locations.clone(),
        equipment: equipment.clone(),
        policies: policies.clone(),
        alerts: alerts.clone(),
        insurance_providers: insurance_providers.clone(),
        billing: billing.clone(),
        emergency_protocols: protocols.clone(),
    };

    client.set_hospital_config(&hospital_wallet, &config);

    let stored = client.get_hospital_config(&hospital_wallet);
    assert_eq!(stored.departments, departments);
    assert_eq!(stored.locations, locations);
    assert_eq!(stored.equipment, equipment);
    assert_eq!(stored.policies, policies);
    assert_eq!(stored.alerts, alerts);
    assert_eq!(stored.insurance_providers, insurance_providers);
    assert_eq!(stored.billing, billing);
    assert_eq!(stored.emergency_protocols, protocols);

    let mut updated_departments: Vec<Department> = Vec::new(&env);
    updated_departments.push_back(Department {
        name: String::from_str(&env, "Cardiology"),
        head: String::from_str(&env, "Dr. Lee"),
        contact: String::from_str(&env, "cardio@rmc.org"),
    });

    client.update_departments(&hospital_wallet, &updated_departments);
    let stored_after = client.get_hospital_config(&hospital_wallet);
    assert_eq!(stored_after.departments, updated_departments);
}
