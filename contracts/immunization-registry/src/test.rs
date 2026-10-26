#![cfg(test)]
#![allow(deprecated)]

use super::*;
use soroban_sdk::{contract, contractimpl, testutils::Address as _, testutils::Events, testutils::Ledger, Address, BytesN, Env, IntoVal, String, Symbol};

// Minimal mock provider registry used by the tests. It exposes the same
// `initialize` / `register_provider` surface the tests rely on so the
// immunization registry can be exercised in isolation.
#[contract]
pub struct ProviderRegistry;

#[contractimpl]
impl ProviderRegistry {
    pub fn initialize(_env: Env, _admin: Address) {}

    #[allow(clippy::too_many_arguments)]
    pub fn register_provider(
        _env: Env,
        _admin: Address,
        _provider: Address,
        _name: String,
        _specialty: String,
        _license: String,
        _license_hash: BytesN<32>,
        _issuer: Address,
        _issuer_hash: BytesN<32>,
        _expires_at: u64,
        _credential_hash: BytesN<32>,
    ) {
    }
}

fn setup() -> (
    Env,
    ImmunizationRegistryClient<'static>,
    ProviderRegistryClient<'static>,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1_700_000_000);

    let admin = Address::generate(&env);
    let registry_id = env.register(ProviderRegistry, ());
    let registry = ProviderRegistryClient::new(&env, &registry_id);
    registry.initialize(&admin);

    let regulator = Address::generate(&env);
    let contract_id = env.register(ImmunizationRegistry, (&regulator, &registry_id));
    let client = ImmunizationRegistryClient::new(&env, &contract_id);

    (env, client, registry, admin, regulator)
}

fn register_provider(
    env: &Env,
    registry: &ProviderRegistryClient<'static>,
    admin: &Address,
    provider: &Address,
) {
    let issuer = Address::generate(env);
    registry.register_provider(
        admin,
        provider,
        &String::from_str(env, "Dr. Provider"),
        &String::from_str(env, "General"),
        &String::from_str(env, "LIC-001"),
        &BytesN::from_array(env, &[1u8; 32]),
        &issuer,
        &BytesN::from_array(env, &[2u8; 32]),
        &u64::MAX,
        &BytesN::from_array(env, &[3u8; 32]),
    );
}

#[test]
fn test_record_immunization() {
    let (env, client, registry, admin, _regulator) = setup();

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    register_provider(&env, &registry, &admin, &provider_id);

    let id = client.record_immunization(&VaccineRecord {
        patient_id: patient_id.clone(),
        provider_id: provider_id.clone(),
        vaccine_name: String::from_str(&env, "Hepatitis B"),
        cvx_code: String::from_str(&env, "CVX_43"),
        lot_number: String::from_str(&env, "LOT_12345"),
        manufacturer: String::from_str(&env, "SANOFI"),
        administration_date: 1690000000,
        expiration_date: 1790000000,
        dose_number: 1,
        route: Symbol::new(&env, "IM"),
        site: Symbol::new(&env, "DELTOID"),
    });

    assert_eq!(id, 1);

    let history = client.get_immunization_history(&patient_id, &patient_id);
    assert_eq!(history.len(), 1);
    let record = history.get(0).unwrap();
    assert_eq!(record.patient_id, patient_id);
    assert_eq!(record.provider_id, provider_id);
    assert_eq!(record.vaccine_name, String::from_str(&env, "Hepatitis B"));
    assert_eq!(record.cvx_code, String::from_str(&env, "CVX_43"));
}

#[test]
fn test_record_immunization_rejects_unregistered_provider() {
    let (env, client, _registry, _admin, _regulator) = setup();

    let patient_id = Address::generate(&env);
    let unregistered = Address::generate(&env);

    let res = client.try_record_immunization(&VaccineRecord {
        patient_id: patient_id.clone(),
        provider_id: unregistered.clone(),
        vaccine_name: String::from_str(&env, "Hepatitis B"),
        cvx_code: String::from_str(&env, "CVX_43"),
        lot_number: String::from_str(&env, "LOT_12345"),
        manufacturer: String::from_str(&env, "SANOFI"),
        administration_date: 1690000000,
        expiration_date: 1790000000,
        dose_number: 1,
        route: Symbol::new(&env, "IM"),
        site: Symbol::new(&env, "DELTOID"),
    });
    assert!(res.is_err());
}

#[test]
fn test_record_adverse_event() {
    let (env, client, registry, admin, regulator) = setup();

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    register_provider(&env, &registry, &admin, &provider_id);

    let id = client.record_immunization(&VaccineRecord {
        patient_id: patient_id.clone(),
        provider_id: provider_id.clone(),
        vaccine_name: String::from_str(&env, "Hepatitis B"),
        cvx_code: String::from_str(&env, "CVX_43"),
        lot_number: String::from_str(&env, "LOT_12345"),
        manufacturer: String::from_str(&env, "SANOFI"),
        administration_date: 1690000000,
        expiration_date: 1790000000,
        dose_number: 1,
        route: Symbol::new(&env, "IM"),
        site: Symbol::new(&env, "DELTOID"),
    });

    // Provider can report.
    client.record_adverse_event(
        &id,
        &provider_id,
        &String::from_str(&env, "Slight fever"),
        &Symbol::new(&env, "MILD"),
        &1690086400,
    );

    // Patient can report.
    client.record_adverse_event(
        &id,
        &patient_id,
        &String::from_str(&env, "Arm soreness"),
        &Symbol::new(&env, "MILD"),
        &1690086400,
    );

    // Regulator can report.
    client.record_adverse_event(
        &id,
        &regulator,
        &String::from_str(&env, "Regulatory follow-up"),
        &Symbol::new(&env, "LOW"),
        &1690086400,
    );

    // Arbitrary outsider is rejected.
    let outsider = Address::generate(&env);
    let res = client.try_record_adverse_event(
        &id,
        &outsider,
        &String::from_str(&env, "Unauthorized"),
        &Symbol::new(&env, "NONE"),
        &1690086400,
    );
    assert!(res.is_err());

    // Non-existent immunization ID is rejected.
    let res2 = client.try_record_adverse_event(
        &999,
        &provider_id,
        &String::from_str(&env, "NA"),
        &Symbol::new(&env, "NONE"),
        &1690086400,
    );
    assert!(res2.is_err());
}

#[test]
fn test_vaccine_series_and_due() {
    let (env, client, registry, admin, _regulator) = setup();

    let patient_id = Address::generate(&env);
    let provider_id = Address::generate(&env);
    register_provider(&env, &registry, &admin, &provider_id);

    // Register a 3-dose series
    client.register_vaccine_series(
        &patient_id,
        &String::from_str(&env, "Hepatitis B"),
        &String::from_str(&env, "CVX_43"),
        &3,
        &BytesN::from_array(&env, &[0; 32]),
    );

    // Initially due.
    let due = client.check_due_vaccines(&patient_id, &patient_id, &1690000000);
    assert_eq!(due.len(), 1);

    // Record one dose.
    client.record_immunization(&VaccineRecord {
        patient_id: patient_id.clone(),
        provider_id: provider_id.clone(),
        vaccine_name: String::from_str(&env, "Hepatitis B"),
        cvx_code: String::from_str(&env, "CVX_43"),
        lot_number: String::from_str(&env, "LOT_12345"),
        manufacturer: String::from_str(&env, "SANOFI"),
        administration_date: 1690000000,
        expiration_date: 1790000000,
        dose_number: 1,
        route: Symbol::new(&env, "IM"),
        site: Symbol::new(&env, "DELTOID"),
    });

    // Still due (need 3).
    let due2 = client.check_due_vaccines(&patient_id, &patient_id, &1695000000);
    assert_eq!(due2.len(), 1);

    // Record two more doses.
    client.record_immunization(&VaccineRecord {
        patient_id: patient_id.clone(),
        provider_id: provider_id.clone(),
        vaccine_name: String::from_str(&env, "Hepatitis B"),
        cvx_code: String::from_str(&env, "CVX_43"),
        lot_number: String::from_str(&env, "LOT_12346"),
        manufacturer: String::from_str(&env, "SANOFI"),
        administration_date: 1692000000,
        expiration_date: 1790000000,
        dose_number: 2,
        route: Symbol::new(&env, "IM"),
        site: Symbol::new(&env, "DELTOID"),
    });
    client.record_immunization(&VaccineRecord {
        patient_id: patient_id.clone(),
        provider_id: provider_id.clone(),
        vaccine_name: String::from_str(&env, "Hepatitis B"),
        cvx_code: String::from_str(&env, "CVX_43"),
        lot_number: String::from_str(&env, "LOT_12347"),
        manufacturer: String::from_str(&env, "SANOFI"),
        administration_date: 1698000000,
        expiration_date: 1790000000,
        dose_number: 3,
        route: Symbol::new(&env, "IM"),
        site: Symbol::new(&env, "DELTOID"),
    });

    // Now not due.
    let due3 = client.check_due_vaccines(&patient_id, &patient_id, &1700000000);
    assert_eq!(due3.len(), 0);
}
