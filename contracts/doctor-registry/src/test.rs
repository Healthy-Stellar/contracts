#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_create_doctor_profile() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. John Smith"),
        &String::from_str(&env, "Cardiology"),
        &institution_wallet,
    );

    let profile = client.get_doctor_profile(&doctor_wallet);

    assert_eq!(profile.name, String::from_str(&env, "Dr. John Smith"));
    assert_eq!(profile.specialization, String::from_str(&env, "Cardiology"));
    assert_eq!(profile.institution_wallet, institution_wallet);
    assert_eq!(profile.metadata, String::from_str(&env, ""));
    assert_eq!(profile.active, true);
    assert_eq!(profile.revoked_at, None);
}

#[test]
fn test_update_doctor_profile() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Jane Doe"),
        &String::from_str(&env, "Neurology"),
        &institution_wallet,
    );

    client.update_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Pediatric Neurology"),
        &String::from_str(&env, "Board Certified, 15 years experience"),
    );

    let profile = client.get_doctor_profile(&doctor_wallet);

    assert_eq!(
        profile.specialization,
        String::from_str(&env, "Pediatric Neurology")
    );
    assert_eq!(
        profile.metadata,
        String::from_str(&env, "Board Certified, 15 years experience")
    );
    assert_eq!(profile.name, String::from_str(&env, "Dr. Jane Doe"));
}

#[test]
fn test_duplicate_profile_creation() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Test"),
        &String::from_str(&env, "General Medicine"),
        &institution_wallet,
    );

    // Attempt to create again — must return DuplicateProfile typed error
    let result = client.try_create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Test"),
        &String::from_str(&env, "General Medicine"),
        &institution_wallet,
    );

    assert_eq!(result, Err(Ok(Error::DuplicateProfile)));
}

#[test]
fn test_get_nonexistent_profile() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let doctor_wallet = Address::generate(&env);

    let result = client.try_get_doctor_profile(&doctor_wallet);
    assert_eq!(result, Err(Ok(Error::ProfileNotFound)));
}

#[test]
fn test_update_nonexistent_profile() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_update_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Cardiology"),
        &String::from_str(&env, "Updated info"),
    );

    assert_eq!(result, Err(Ok(Error::ProfileNotFound)));
}

#[test]
fn test_multiple_doctors() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor1_wallet = Address::generate(&env);
    let doctor2_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor1_wallet,
        &String::from_str(&env, "Dr. Alice"),
        &String::from_str(&env, "Oncology"),
        &institution_wallet,
    );

    client.create_doctor_profile(
        &admin,
        &doctor2_wallet,
        &String::from_str(&env, "Dr. Bob"),
        &String::from_str(&env, "Orthopedics"),
        &institution_wallet,
    );

    let profile1 = client.get_doctor_profile(&doctor1_wallet);
    let profile2 = client.get_doctor_profile(&doctor2_wallet);

    assert_eq!(profile1.name, String::from_str(&env, "Dr. Alice"));
    assert_eq!(profile1.specialization, String::from_str(&env, "Oncology"));

    assert_eq!(profile2.name, String::from_str(&env, "Dr. Bob"));
    assert_eq!(
        profile2.specialization,
        String::from_str(&env, "Orthopedics")
    );
}

#[test]
fn test_double_initialize_fails() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_initialize(&admin);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_create_without_initialize_fails() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let non_admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    // Contract not initialized — no admin stored, so require_admin returns Unauthorized
    let result = client.try_create_doctor_profile(
        &non_admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Impostor"),
        &String::from_str(&env, "Fake Specialty"),
        &institution_wallet,
    );

    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_non_admin_cannot_create_profile() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();

    client.initialize(&admin);

    // Attacker passes their own address as registrar — stored admin is different
    let result = client.try_create_doctor_profile(
        &attacker,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Impostor"),
        &String::from_str(&env, "Fake Specialty"),
        &institution_wallet,
    );

    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_deactivate_and_reactivate_doctor_lifecycle() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Gregory House"),
        &String::from_str(&env, "Diagnostic Medicine"),
        &institution_wallet,
    );

    // Initial state: active == true, revoked_at == None, is_active == true
    let profile = client.get_doctor_profile(&doctor_wallet);
    assert_eq!(profile.active, true);
    assert_eq!(profile.revoked_at, None);
    assert_eq!(client.is_active(&doctor_wallet), true);

    // Deactivate doctor profile
    client.deactivate_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "suspended"),
    );

    let deactivated_profile = client.get_doctor_profile(&doctor_wallet);
    assert_eq!(deactivated_profile.active, false);
    assert_eq!(deactivated_profile.revoked_at, Some(env.ledger().timestamp()));
    assert_eq!(client.is_active(&doctor_wallet), false);

    // Reactivate doctor profile
    client.reactivate_doctor_profile(&admin, &doctor_wallet);

    let reactivated_profile = client.get_doctor_profile(&doctor_wallet);
    assert_eq!(reactivated_profile.active, true);
    assert_eq!(client.is_active(&doctor_wallet), true);
}

#[test]
fn test_deactivate_inactive_profile_rejected() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Remy Hadley"),
        &String::from_str(&env, "Immunology"),
        &institution_wallet,
    );

    client.deactivate_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "suspended"),
    );

    // Deactivating an already-inactive profile must be rejected and must not
    // overwrite the original revocation timestamp.
    let first_revoked_at = client.get_doctor_profile(&doctor_wallet).revoked_at;
    let res = client.try_deactivate_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "expired"),
    );
    assert_eq!(res, Err(Ok(Error::AlreadyInactive)));
    assert_eq!(
        client.get_doctor_profile(&doctor_wallet).revoked_at,
        first_revoked_at
    );
}

#[test]
fn test_status_history_accumulates() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Eric Foreman"),
        &String::from_str(&env, "Neurology"),
        &institution_wallet,
    );

    // Creation records the initial active status.
    let history = client.get_status_history(&doctor_wallet);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().status, true);

    client.deactivate_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "suspended"),
    );
    client.reactivate_doctor_profile(&admin, &doctor_wallet);
    client.deactivate_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "voluntary"),
    );

    let history = client.get_status_history(&doctor_wallet);
    assert_eq!(history.len(), 4);

    let first_deact = history.get(1).unwrap();
    assert_eq!(first_deact.status, false);
    assert_eq!(first_deact.reason, String::from_str(&env, "suspended"));
    assert_eq!(first_deact.by, admin);

    let react = history.get(2).unwrap();
    assert_eq!(react.status, true);
    assert_eq!(react.by, admin);

    let second_deact = history.get(3).unwrap();
    assert_eq!(second_deact.status, false);
    assert_eq!(second_deact.reason, String::from_str(&env, "voluntary"));
    assert_eq!(second_deact.by, admin);

    // Reactivation must not erase the revocation history.
    assert!(client.get_doctor_profile(&doctor_wallet).revoked_at.is_some());
}

#[test]
fn test_deactivate_requires_reason() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Allison Cameron"),
        &String::from_str(&env, "Immunology"),
        &institution_wallet,
    );

    let res = client.try_deactivate_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, ""),
    );
    assert_eq!(res, Err(Ok(Error::ReasonRequired)));
    assert_eq!(client.is_active(&doctor_wallet), true);
}

#[test]
fn test_deactivation_event_carries_registrar_and_reason() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Robert Chase"),
        &String::from_str(&env, "Surgery"),
        &institution_wallet,
    );

    let reason = String::from_str(&env, "expired");
    client.deactivate_doctor_profile(&admin, &doctor_wallet, &reason);

    let events = env.events().all();
    let mut found = false;
    for (_contract, topics, data) in events.iter() {
        let _ = topics;
        let decoded: Result<(Address, String), _> = data.try_into_val(&env);
        if let Ok((by, why)) = decoded {
            if by == admin && why == reason {
                found = true;
            }
        }
    }
    assert!(found, "deactivation event must carry registrar and reason");
}

#[test]
fn test_update_profile_name_and_institution() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);
    let new_institution_wallet = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Lawrence Kutner"),
        &String::from_str(&env, "Sports Medicine"),
        &institution_wallet,
    );

    client.update_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Lawrence Kutner-Chase"),
        &new_institution_wallet,
    );

    let profile = client.get_doctor_profile(&doctor_wallet);
    assert_eq!(
        profile.name,
        String::from_str(&env, "Dr. Lawrence Kutner-Chase")
    );
    assert_eq!(profile.institution_wallet, new_institution_wallet);

    // Non-admin cannot update the profile.
    let attacker = Address::generate(&env);
    let res = client.try_update_profile(
        &attacker,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Impostor"),
        &new_institution_wallet,
    );
    assert_eq!(res, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_two_step_admin_rotation() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let attacker = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    // Only the current admin may propose a successor.
    let res = client.try_propose_admin(&attacker, &new_admin);
    assert_eq!(res, Err(Ok(Error::Unauthorized)));

    client.propose_admin(&admin, &new_admin);

    // Only the proposed successor may accept.
    let res = client.try_accept_admin(&attacker);
    assert_eq!(res, Err(Ok(Error::Unauthorized)));

    client.accept_admin(&new_admin);

    // Old admin no longer has authority.
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);
    let res = client.try_create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Impostor"),
        &String::from_str(&env, "Fake Specialty"),
        &institution_wallet,
    );
    assert_eq!(res, Err(Ok(Error::Unauthorized)));

    // New admin does.
    client.create_doctor_profile(
        &new_admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Chris Taub"),
        &String::from_str(&env, "Plastic Surgery"),
        &institution_wallet,
    );
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn test_deactivate_and_reactivate_unauthorized() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let doctor_wallet = Address::generate(&env);
    let institution_wallet = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    client.create_doctor_profile(
        &admin,
        &doctor_wallet,
        &String::from_str(&env, "Dr. Wilson"),
        &String::from_str(&env, "Oncology"),
        &institution_wallet,
    );

    // Non-admin cannot deactivate
    let res_deact = client.try_deactivate_doctor_profile(
        &attacker,
        &doctor_wallet,
        &String::from_str(&env, "suspended"),
    );
    assert_eq!(res_deact, Err(Ok(Error::Unauthorized)));

    // Non-admin cannot reactivate
    let res_react = client.try_reactivate_doctor_profile(&attacker, &doctor_wallet);
    assert_eq!(res_react, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_deactivate_and_reactivate_nonexistent_profile_fails() {
    let env = Env::default();
    let contract_id = env.register(DoctorRegistry, ());
    let client = DoctorRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let unknown_doctor = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    let res_deact = client.try_deactivate_doctor_profile(
        &admin,
        &unknown_doctor,
        &String::from_str(&env, "suspended"),
    );
    assert_eq!(res_deact, Err(Ok(Error::ProfileNotFound)));

    let res_react = client.try_reactivate_doctor_profile(&admin, &unknown_doctor);
    assert_eq!(res_react, Err(Ok(Error::ProfileNotFound)));

    assert_eq!(client.is_active(&unknown_doctor), false);
}

