#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, BytesN, Env, String, Vec};

/// Generate a dummy hash filled with a specific byte value.
/// Local helper mirroring the pattern used by other contracts (e.g.
/// provider-registry); `shared::test_utils` is `#[cfg(test)]`-gated and
/// cannot be imported from another crate's test build.
fn dummy_hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}


fn register_hospital_with_anchor(
    env: &Env,
    client: &HospitalRegistryClient<'_>,
    hospital_wallet: &Address,
) {
    let admin = Address::generate(env);
    if client.get_admin().is_none() {
        client.initialize_admin(&admin);
    }

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
fn test_set_admin_cannot_bootstrap_without_initialize() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let attacker = Address::generate(&env);
    env.mock_all_auths();

    // set_admin must never be usable as a bootstrap path: with no admin
    // initialized yet, an attacker cannot self-elect as admin.
    let result = client.try_set_admin(&attacker, &attacker);
    assert_eq!(result, Err(Ok(ContractError::NotAuthorized)));
    assert!(client.get_admin().is_none());
}

#[test]
fn test_set_admin_requires_existing_admin() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    env.mock_all_auths();

    client.initialize_admin(&admin);

    // A non-admin caller cannot rotate the admin.
    let result = client.try_set_admin(&attacker, &attacker);
    assert_eq!(result, Err(Ok(ContractError::NotAuthorized)));
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // The current admin can still rotate to a new admin.
    let new_admin = Address::generate(&env);
    client.set_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), Some(new_admin));
}

#[test]
fn test_register_hospital_requires_admin() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    let issuer = Address::generate(&env);
    env.mock_all_auths();

    let result = client.try_register_hospital(
        &hospital_wallet,
        &String::from_str(&env, "General Hospital"),
        &String::from_str(&env, "123 Main St, New York, NY"),
        &String::from_str(&env, "Services: ER, Surgery, Cardiology"),
        &issuer,
        &dummy_hash(&env, 1),
        &dummy_hash(&env, 2),
        &4_100_000_000_u64,
        &dummy_hash(&env, 3),
    );

    assert_eq!(result, Err(Ok(ContractError::NotAuthorized)));
}

#[test]
fn test_register_hospital() {
    let env = Env::default();
    let contract_id = env.register(HospitalRegistry, ());
    let client = HospitalRegistryClient::new(&env, &contract_id);

    let hospital_wallet = Address::generate(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    client.initialize_admin(&admin);
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
        alert_type: String::from_str(&env, "emergency"),
        enabled: true,
        threshold: 1,
    });

    client.set_hospital_config(
        &hospital_wallet,
        &departments,
        &locations,
        &equipment,
        &policies,
        &channels,
        &alerts,
    );

    let config = client.get_hospital_config(&hospital_wallet);
    assert_eq!(config.departments.len(), 1);
    assert_eq!(config.locations.len(), 1);
    assert_eq!(config.equipment.len(), 1);
    assert_eq!(config.policies.len(), 1);
    assert_eq!(config.communication_channels.len(), 2);
    assert_eq!(config.alert_settings.len(), 1);
}
