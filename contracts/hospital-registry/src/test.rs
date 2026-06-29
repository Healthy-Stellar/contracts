#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, BytesN, Env, String, Vec};

fn dummy_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn register_hospital_with_anchor(
    env: &Env,
    client: &HospitalRegistryClient<'_>,
    hospital_wallet: &Address,
) {
    let issuer = Address::generate(env);
    client.register_hospital(
        hospital_wallet,
        &String::from_str(env, "General Hospital"),
        &String::from_str(env, "123 Main St, New York, NY"),
        &String::from_str(env, "Services: ER, Surgery, Cardiology"),
        &issuer,
        &dummy_hash(env, 1),
        &dummy_hash(env, 2),
        &4_100_000_000_u64,
        &dummy_hash(env, 3),
    );
}

#[test]
fn test_register_hospital() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_hospital_with_anchor(&env, &client, &hospital_wallet);

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
    assert_eq!(hospital.credential.credential_hash, dummy_hash(&env, 1));
}

#[test]
fn test_update_hospital() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_hospital_with_anchor(&env, &client, &hospital_wallet);

    client.update_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Services: ER, ICU, Pediatrics, Oncology"),
    );

    let hospital = client.get_hospital(&hospital_wallet);
    assert_eq!(
        hospital.metadata,
        String::from_str(&env, "Services: ER, ICU, Pediatrics, Oncology")
    );
    assert_eq!(hospital.name, String::from_str(&env, "General Hospital"));
}

#[test]
fn test_duplicate_registration() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_hospital_with_anchor(&env, &client, &hospital_wallet);

    let issuer = Address::generate(&env);
    let result = client.try_register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Test Hospital"),
        &String::from_str(&env, "Test Location"),
        &String::from_str(&env, "Test Metadata"),
        &issuer,
        &dummy_hash(&env, 4),
        &dummy_hash(&env, 5),
        &4_100_000_000_u64,
        &dummy_hash(&env, 6),
    );

    assert_eq!(result, Err(Ok(ContractError::HospitalAlreadyRegistered)));
}

#[test]
fn test_get_nonexistent_hospital() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);

    let result = client.try_get_hospital(&hospital_wallet);
    assert_eq!(result, Err(Ok(ContractError::HospitalNotFound)));
}

#[test]
fn test_update_nonexistent_hospital() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    let result = client.try_update_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Updated Metadata"),
    );
    assert_eq!(result, Err(Ok(ContractError::HospitalNotFound)));
}

#[test]
fn test_expired_hospital_credential_disables_membership() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 100);

    client.register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "City Hospital"),
        &String::from_str(&env, "456 Oak Ave"),
        &String::from_str(&env, "General Services"),
        &issuer,
        &dummy_hash(&env, 1),
        &dummy_hash(&env, 2),
        &150_u64,
        &dummy_hash(&env, 3),
    );
    assert!(client.is_hospital_active(&hospital_wallet));

    env.ledger().with_mut(|li| li.timestamp = 151);
    assert!(!client.is_hospital_active(&hospital_wallet));

    let result = client.try_update_hospital(
        &hospital_wallet,
        &String::from_str(&env, "Should fail"),
    );
    assert_eq!(result, Err(Ok(ContractError::CredentialExpired)));
}

#[test]
fn test_hospital_config_flow() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_hospital_with_anchor(&env, &client, &hospital_wallet);

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

#[test]
fn test_update_departments_exceeds_limit() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_hospital_with_anchor(&env, &client, &hospital_wallet);

    // Initialise an empty config so get_hospital_config succeeds inside update_departments
    client.set_hospital_config(&hospital_wallet, &HospitalConfig {
        departments: Vec::new(&env),
        locations: Vec::new(&env),
        equipment: Vec::new(&env),
        policies: Vec::new(&env),
        alerts: Vec::new(&env),
        insurance_providers: Vec::new(&env),
        billing: BillingConfig {
            currency: String::from_str(&env, "USD"),
            payment_terms: String::from_str(&env, "Net 30"),
            tax_id: String::from_str(&env, "TAX-001"),
        },
        emergency_protocols: Vec::new(&env),
    });

    let mut departments: Vec<Department> = Vec::new(&env);
    for i in 0..=MAX_DEPARTMENTS {
        departments.push_back(Department {
            name: String::from_str(&env, "Dept"),
            head: String::from_str(&env, "Head"),
            contact: String::from_str(&env, "contact@hospital.org"),
        });
        let _ = i;
    }

    let result = client.try_update_departments(&hospital_wallet, &departments);
    assert_eq!(result, Err(Ok(ContractError::ConfigLimitExceeded)));
}

#[test]
fn test_update_locations_exceeds_limit() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_hospital_with_anchor(&env, &client, &hospital_wallet);

    client.set_hospital_config(&hospital_wallet, &HospitalConfig {
        departments: Vec::new(&env),
        locations: Vec::new(&env),
        equipment: Vec::new(&env),
        policies: Vec::new(&env),
        alerts: Vec::new(&env),
        insurance_providers: Vec::new(&env),
        billing: BillingConfig {
            currency: String::from_str(&env, "USD"),
            payment_terms: String::from_str(&env, "Net 30"),
            tax_id: String::from_str(&env, "TAX-001"),
        },
        emergency_protocols: Vec::new(&env),
    });

    let mut locations: Vec<Location> = Vec::new(&env);
    for i in 0..=MAX_LOCATIONS {
        locations.push_back(Location {
            name: String::from_str(&env, "Loc"),
            address: String::from_str(&env, "Addr"),
            metadata: String::from_str(&env, ""),
        });
        let _ = i;
    }

    let result = client.try_update_locations(&hospital_wallet, &locations);
    assert_eq!(result, Err(Ok(ContractError::ConfigLimitExceeded)));
}

#[test]
fn test_update_equipment_exceeds_limit() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_hospital_with_anchor(&env, &client, &hospital_wallet);

    client.set_hospital_config(&hospital_wallet, &HospitalConfig {
        departments: Vec::new(&env),
        locations: Vec::new(&env),
        equipment: Vec::new(&env),
        policies: Vec::new(&env),
        alerts: Vec::new(&env),
        insurance_providers: Vec::new(&env),
        billing: BillingConfig {
            currency: String::from_str(&env, "USD"),
            payment_terms: String::from_str(&env, "Net 30"),
            tax_id: String::from_str(&env, "TAX-001"),
        },
        emergency_protocols: Vec::new(&env),
    });

    let mut equipment: Vec<EquipmentResource> = Vec::new(&env);
    for i in 0..=MAX_EQUIPMENT {
        equipment.push_back(EquipmentResource {
            name: String::from_str(&env, "Item"),
            quantity: 1,
            status: String::from_str(&env, "operational"),
            metadata: String::from_str(&env, ""),
        });
        let _ = i;
    }

    let result = client.try_update_equipment(&hospital_wallet, &equipment);
    assert_eq!(result, Err(Ok(ContractError::ConfigLimitExceeded)));
}

#[test]
fn test_set_hospital_config_exceeds_limits() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    env.mock_all_auths();

    register_hospital_with_anchor(&env, &client, &hospital_wallet);

    let mut locations: Vec<Location> = Vec::new(&env);
    for i in 0..=MAX_LOCATIONS {
        locations.push_back(Location {
            name: String::from_str(&env, "Loc"),
            address: String::from_str(&env, "Addr"),
            metadata: String::from_str(&env, ""),
        });
        let _ = i;
    }

    let result = client.try_set_hospital_config(&hospital_wallet, &HospitalConfig {
        departments: Vec::new(&env),
        locations,
        equipment: Vec::new(&env),
        policies: Vec::new(&env),
        alerts: Vec::new(&env),
        insurance_providers: Vec::new(&env),
        billing: BillingConfig {
            currency: String::from_str(&env, "USD"),
            payment_terms: String::from_str(&env, "Net 30"),
            tax_id: String::from_str(&env, "TAX-001"),
        },
        emergency_protocols: Vec::new(&env),
    });
    assert_eq!(result, Err(Ok(ContractError::ConfigLimitExceeded)));
}
