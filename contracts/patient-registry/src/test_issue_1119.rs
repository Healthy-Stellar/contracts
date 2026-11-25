#![cfg(test)]

//! #1119: `get_authorized_doctors` is gated to the patient / guardian, and
//! `grant_access` / `revoke_access` emit structured audit events.

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, Map, Symbol, TryFromVal, Val,
};

fn setup(env: &Env) -> MedicalRegistryClient<'_> {
    let contract_id = env.register(MedicalRegistry, ());
    let client = MedicalRegistryClient::new(env, &contract_id);
    client.initialize(
        &Address::generate(env),
        &Address::generate(env),
        &Address::generate(env),
    );
    client
}

/// Find the data map of the last event whose first topic is `name`.
fn find_event(env: &Env, name: &str) -> Option<Map<Symbol, Val>> {
    let topic = Symbol::new(env, name);
    let mut found = None;
    for (_contract_id, topics, data) in env.events().all().iter() {
        if let Some(first) = topics.get(0) {
            if let Ok(sym) = Symbol::try_from_val(env, &first) {
                if sym == topic {
                    found = Map::<Symbol, Val>::try_from_val(env, &data).ok();
                }
            }
        }
    }
    found
}

fn field<T: TryFromVal<Env, Val>>(env: &Env, data: &Map<Symbol, Val>, key: &str) -> T {
    let v = data.get(Symbol::new(env, key)).expect("missing event field");
    T::try_from_val(env, &v).ok().expect("bad event field type")
}

#[test]
fn test_get_authorized_doctors_patient_can_read() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.grant_access(&patient, &patient, &doctor, &0u64);

    let doctors = client.get_authorized_doctors(&patient, &patient);
    assert_eq!(doctors.len(), 1);
    assert_eq!(doctors.get(0).unwrap(), doctor);
}

#[test]
fn test_get_authorized_doctors_guardian_can_read() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.assign_guardian(&patient, &guardian);
    client.grant_access(&patient, &patient, &doctor, &0u64);

    assert_eq!(client.get_authorized_doctors(&patient, &guardian).len(), 1);
}

#[test]
fn test_get_authorized_doctors_rejects_third_party() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.grant_access(&patient, &patient, &doctor, &0u64);

    let res = client.try_get_authorized_doctors(&patient, &stranger);
    assert_eq!(res, Err(Ok(ContractError::NotAuthorized)));

    // Even an authorized doctor cannot enumerate the rest of the care team.
    let res = client.try_get_authorized_doctors(&patient, &doctor);
    assert_eq!(res, Err(Ok(ContractError::NotAuthorized)));
}

#[test]
fn test_get_authorized_doctors_requires_caller_auth() {
    let env = Env::default();
    let client = setup(&env);
    let patient = Address::generate(&env);

    // No auths mocked: the patient's own call must still be authorized.
    let res = client.try_get_authorized_doctors(&patient, &patient);
    assert!(res.is_err());
}

#[test]
fn test_grant_access_emits_access_granted_event() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env);
    let patient = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.grant_access(&patient, &patient, &doctor, &0u64);

    let data = find_event(&env, "access_granted").expect("access_granted not emitted");
    let provider: Address = field(&env, &data, "provider");
    let role: Symbol = field(&env, &data, "actor_role");
    let op_id: u64 = field(&env, &data, "op_id");
    assert_eq!(provider, doctor);
    assert_eq!(role, Symbol::new(&env, "patient"));
    assert_eq!(op_id, 1);
}

#[test]
fn test_revoke_access_emits_access_revoked_event_with_guardian_role() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env);
    let patient = Address::generate(&env);
    let guardian = Address::generate(&env);
    let doctor = Address::generate(&env);

    client.assign_guardian(&patient, &guardian);
    client.grant_access(&patient, &guardian, &doctor, &0u64);
    client.revoke_access(&patient, &guardian, &doctor, &0u64);

    let data = find_event(&env, "access_revoked").expect("access_revoked not emitted");
    let provider: Address = field(&env, &data, "provider");
    let role: Symbol = field(&env, &data, "actor_role");
    let op_id: u64 = field(&env, &data, "op_id");
    assert_eq!(provider, doctor);
    assert_eq!(role, Symbol::new(&env, "guardian"));
    // grant = op 1, revoke = op 2
    assert_eq!(op_id, 2);
}
