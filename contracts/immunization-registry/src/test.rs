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

    pub fn is_provider(env: Env, provider: Address) -> bool {
        env.storage().instance().has(&provider)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register_provider(
        env: Env,
        _admin: Address,
        provider: Address,
        _name: String,
        _specialty: String,
        _license: String,
        _license_hash: BytesN<32>,
        _issuer: Address,
        _issuer_hash: BytesN<32>,
        _expires_at: u64,
        _credential_hash: BytesN<32>,
    ) {
        env.storage().instance().set(&provider, &true);
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
    }, &None);

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
    }, &None);
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
    }, &None);

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
    }, &None);

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
    }, &None);
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
    }, &None);

    // Now not due.
    let due3 = client.check_due_vaccines(&patient_id, &patient_id, &1700000000);
    assert_eq!(due3.len(), 0);
}

// ─── #1112: per-lot index, dose validation, lot normalization ───────────────

fn dose(
    env: &Env,
    patient_id: &Address,
    provider_id: &Address,
    lot: &str,
    dose_number: u32,
    administration_date: u64,
) -> VaccineRecord {
    VaccineRecord {
        patient_id: patient_id.clone(),
        provider_id: provider_id.clone(),
        vaccine_name: String::from_str(env, "COVID-19 mRNA"),
        cvx_code: String::from_str(env, "CVX_208"),
        lot_number: String::from_str(env, lot),
        manufacturer: String::from_str(env, "PFIZER"),
        administration_date,
        expiration_date: 1_790_000_000,
        dose_number,
        route: Symbol::new(env, "IM"),
        site: Symbol::new(env, "DELTOID"),
    }
}

#[test]
fn test_duplicate_dose_rejected() {
    let (env, client, registry, admin, _regulator) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    let other_clinic = Address::generate(&env);
    register_provider(&env, &registry, &admin, &provider);
    register_provider(&env, &registry, &admin, &other_clinic);

    client.record_immunization(&dose(&env, &patient, &provider, "EL9261", 1, 1_690_000_000), &None);

    // A second clinic recording the same dose is rejected, even with an override.
    let res = client.try_record_immunization(
        &dose(&env, &patient, &other_clinic, "EL9261", 1, 1_690_000_100),
        &Some(String::from_str(&env, "catch-up")),
    );
    assert_eq!(res, Err(Ok(Error::DuplicateDose)));
}

#[test]
fn test_out_of_sequence_dose_requires_override() {
    let (env, client, registry, admin, _regulator) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    register_provider(&env, &registry, &admin, &provider);

    // Dose 3 with no dose 1 or 2.
    let res = client.try_record_immunization(
        &dose(&env, &patient, &provider, "EL9261", 3, 1_690_000_000),
        &None,
    );
    assert_eq!(res, Err(Ok(Error::OutOfSequenceDose)));

    // An empty override reason does not count.
    let res = client.try_record_immunization(
        &dose(&env, &patient, &provider, "EL9261", 3, 1_690_000_000),
        &Some(String::from_str(&env, "")),
    );
    assert_eq!(res, Err(Ok(Error::OutOfSequenceDose)));

    // With a reason (e.g. doses 1-2 given abroad) it is accepted.
    client.record_immunization(
        &dose(&env, &patient, &provider, "EL9261", 3, 1_690_000_000),
        &Some(String::from_str(&env, "doses 1-2 administered abroad")),
    );

    // Next dose must follow on from 3.
    client.record_immunization(&dose(&env, &patient, &provider, "EL9262", 4, 1_695_000_000), &None);
}

#[test]
fn test_dose_before_previous_dose_rejected() {
    let (env, client, registry, admin, _regulator) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    register_provider(&env, &registry, &admin, &provider);

    client.record_immunization(&dose(&env, &patient, &provider, "EL9261", 1, 1_695_000_000), &None);
    let res = client.try_record_immunization(
        &dose(&env, &patient, &provider, "EL9261", 2, 1_690_000_000),
        &None,
    );
    assert_eq!(res, Err(Ok(Error::OutOfSequenceDose)));
}

#[test]
fn test_lot_numbers_are_normalized_and_patients_paged() {
    let (env, client, registry, admin, regulator) = setup();
    let provider = Address::generate(&env);
    register_provider(&env, &registry, &admin, &provider);

    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    let p3 = Address::generate(&env);

    client.record_immunization(&dose(&env, &p1, &provider, "EL9261", 1, 1_690_000_000), &None);
    client.record_immunization(&dose(&env, &p2, &provider, "el9261 ", 1, 1_690_000_000), &None);
    // p1 gets dose 2 from the same lot: must not appear twice.
    client.record_immunization(&dose(&env, &p1, &provider, " El9261", 2, 1_692_000_000), &None);
    client.record_immunization(&dose(&env, &p3, &provider, "EL9261", 1, 1_690_000_000), &None);

    let lot = String::from_str(&env, "el9261");
    assert_eq!(client.get_lot_dose_count(&lot, &regulator), 4);
    assert_eq!(client.get_lot_patient_count(&lot, &regulator), 3);

    let page1 = client.get_patients_by_lot(&lot, &regulator, &0, &2);
    let page2 = client.get_patients_by_lot(&lot, &regulator, &2, &2);
    assert_eq!(page1.len(), 2);
    assert_eq!(page2.len(), 1);
    assert_eq!(page1.get(0).unwrap(), p1);
    assert_eq!(page1.get(1).unwrap(), p2);
    assert_eq!(page2.get(0).unwrap(), p3);

    let history = client.get_immunization_history(&p2, &p2);
    assert_eq!(
        history.get(0).unwrap().lot_number,
        String::from_str(&env, "EL9261")
    );
}

#[test]
fn test_get_patients_by_lot_regulator_only() {
    let (env, client, _registry, _admin, _regulator) = setup();
    let outsider = Address::generate(&env);
    let res = client.try_get_patients_by_lot(&String::from_str(&env, "EL9261"), &outsider, &0, &10);
    assert_eq!(res, Err(Ok(Error::NotAuthorized)));
}

#[test]
fn test_blank_lot_number_rejected() {
    let (env, client, registry, admin, _regulator) = setup();
    let patient = Address::generate(&env);
    let provider = Address::generate(&env);
    register_provider(&env, &registry, &admin, &provider);

    let res = client.try_record_immunization(
        &dose(&env, &patient, &provider, "   ", 1, 1_690_000_000),
        &None,
    );
    assert_eq!(res, Err(Ok(Error::InvalidLotNumber)));
}
