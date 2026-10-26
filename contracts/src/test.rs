#[cfg(test)]
mod tests {
    use crate::{
        safe_increment, safe_increment_ns, safe_increment_persistent, safe_increment_persistent_ns,
        AppointmentScheduling, AppointmentSchedulingClient, AppointmentStatus, DataKey, Error,
        HealthcareRegistry, HealthcareRegistryClient,
    };

    use soroban_sdk::{
        testutils::{Address as _, MockAuth, MockAuthInvoke},
        Address, Env, IntoVal, String, Vec,
    };

    fn setup_registry_test(
        env: &Env,
    ) -> (Address, HealthcareRegistryClient<'static>, Address, Address) {
        let admin = Address::generate(env);
        let contract_id = env.register(HealthcareRegistry, (&admin,));
        let client = HealthcareRegistryClient::new(env, &contract_id);

        let institution = Address::generate(env);

        env.mock_all_auths();

        (contract_id, client, admin, institution)
    }

    fn setup_test(env: &Env) -> (HealthcareRegistryClient<'static>, Address, Address) {
        let (_, client, admin, institution) = setup_registry_test(env);
        (client, admin, institution)
    }

    fn setup_appointment_test(
        env: &Env,
    ) -> (AppointmentSchedulingClient<'static>, Address, Address) {
        let contract_id = env.register(AppointmentScheduling, ());
        let client = AppointmentSchedulingClient::new(env, &contract_id);

        let patient = Address::generate(env);
        let doctor = Address::generate(env);

        (client, patient, doctor)
    }

    fn stored_admin(env: &Env, contract_id: &Address) -> Address {
        env.as_contract(contract_id, || {
            env.storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::Admin)
                .unwrap()
        })
    }

    fn stored_pending_admin(env: &Env, contract_id: &Address) -> Option<Address> {
        env.as_contract(contract_id, || {
            env.storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::PendingAdmin)
        })
    }

    #[test]
    fn test_register_and_get() {
        let env = Env::default();
        let (client, _, inst_addr) = setup_test(&env);

        let name = String::from_str(&env, "General Hospital");
        let license = String::from_str(&env, "LIC-123");
        let meta = String::from_str(&env, "{}");

        env.mock_all_auths();
        client.register_institution(&inst_addr, &name, &license, &meta);

        let data = client.get_institution(&inst_addr);
        assert_eq!(data.name, name);
    }

    #[test]
    fn test_duplicate_registration_fails() {
        let env = Env::default();
        let (client, _, inst_addr) = setup_test(&env);
        env.mock_all_auths();

        let name = String::from_str(&env, "Clinic A");
        client.register_institution(&inst_addr, &name, &name, &name);
        let result = client.try_register_institution(&inst_addr, &name, &name, &name);
        assert_eq!(result, Err(Ok(Error::AlreadyRegistered)));
    }

    #[test]
    fn test_verification_by_admin() {
        let env = Env::default();
        let (client, admin, inst_addr) = setup_test(&env);
        env.mock_all_auths();

        let name = String::from_str(&env, "Clinic A");
        client.register_institution(&inst_addr, &name, &name, &name);

        client.verify_institution(&admin, &inst_addr);

        let data = client.get_institution(&inst_addr);
        assert!(data.is_verified);
    }

    #[test]
    fn test_unauthorized_verification_fails() {
        let env = Env::default();
        let (client, _, inst_addr) = setup_test(&env);
        let fake_admin = Address::generate(&env);
        env.mock_all_auths();

        let name = String::from_str(&env, "Clinic A");
        client.register_institution(&inst_addr, &name, &name, &name);

        let result = client.try_verify_institution(&fake_admin, &inst_addr);
        assert_eq!(result, Err(Ok(Error::NotAuthorized)));
    }

    #[test]
    fn test_propose_and_accept_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, _client, _admin, _) = setup_registry_test(&env);
        let new_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            HealthcareRegistry::propose_admin(env.clone(), new_admin.clone()).unwrap();
        });

        assert_eq!(
            stored_pending_admin(&env, &contract_id),
            Some(new_admin.clone())
        );

        env.as_contract(&contract_id, || {
            HealthcareRegistry::accept_admin(env.clone()).unwrap();
        });

        assert_eq!(stored_admin(&env, &contract_id), new_admin.clone());
        assert_eq!(stored_pending_admin(&env, &contract_id), None);
    }

    #[test]
    fn test_cancel_admin_transfer() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, _client, admin, _) = setup_registry_test(&env);
        let new_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            HealthcareRegistry::propose_admin(env.clone(), new_admin.clone()).unwrap();
        });

        env.as_contract(&contract_id, || {
            HealthcareRegistry::cancel_admin_transfer(env.clone()).unwrap();
        });

        assert_eq!(stored_admin(&env, &contract_id), admin.clone());
        assert_eq!(stored_pending_admin(&env, &contract_id), None);
    }

    #[test]
    fn test_unauthorized_propose_rejected() {
        let env = Env::default();
        let (contract_id, client, admin, _) = setup_registry_test(&env);
        let attacker = Address::generate(&env);
        let new_admin = Address::generate(&env);

        let result = client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "propose_admin",
                    args: (&new_admin,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_propose_admin(&new_admin);

        assert!(result.is_err());
        assert_eq!(stored_admin(&env, &contract_id), admin);
        assert_eq!(stored_pending_admin(&env, &contract_id), None);
    }

    #[test]
    fn test_unauthorized_accept_rejected() {
        let env = Env::default();
        let (contract_id, client, admin, _) = setup_registry_test(&env);
        let new_admin = Address::generate(&env);
        let attacker = Address::generate(&env);

        client
            .mock_auths(&[MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "propose_admin",
                    args: (&new_admin,).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .propose_admin(&new_admin);

        let result = client
            .mock_auths(&[MockAuth {
                address: &attacker,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "accept_admin",
                    args: ().into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_accept_admin();

        assert!(result.is_err());
        assert_eq!(stored_admin(&env, &contract_id), admin);
        assert_eq!(stored_pending_admin(&env, &contract_id), Some(new_admin));
    }

    #[test]
    fn test_update_metadata() {
        let env = Env::default();
        let (client, _, inst_addr) = setup_test(&env);
        env.mock_all_auths();

        client.register_institution(
            &inst_addr,
            &String::from_str(&env, "H"),
            &String::from_str(&env, "1"),
            &String::from_str(&env, "old"),
        );

        let new_meta = String::from_str(&env, "new_metadata");
        client.update_institution(&inst_addr, &new_meta);

        let data = client.get_institution(&inst_addr);
        assert_eq!(data.metadata, new_meta);
    }

    // Appointment Scheduling Tests
    #[test]
    fn test_create_appointment() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        env.mock_all_auths();

        let datetime = 1640995200; // 2022-01-01 00:00:00 UTC
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        assert_eq!(appointment_id, 1);

        let patient_appointments = client.get_appointments(&patient);
        assert_eq!(patient_appointments.len(), 1);

        let appointment = &patient_appointments.get(0).unwrap();
        assert_eq!(appointment.patient, patient);
        assert_eq!(appointment.doctor, doctor);
        assert_eq!(appointment.datetime, datetime);
        assert!(matches!(appointment.status, AppointmentStatus::Scheduled));
    }

    #[test]
    fn test_cancel_appointment() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        env.mock_all_auths();

        let datetime = 1640995200;
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        client.cancel_appointment(&patient, &appointment_id);

        let patient_appointments = client.get_appointments(&patient);
        let appointment = &patient_appointments.get(0).unwrap();
        assert!(matches!(appointment.status, AppointmentStatus::Canceled));
    }

    #[test]
    fn test_unauthorized_cancel_appointment() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        let unauthorized_user = Address::generate(&env);
        env.mock_all_auths();

        let datetime = 1640995200;
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        let result = client.try_cancel_appointment(&unauthorized_user, &appointment_id);
        assert_eq!(result, Err(Ok(Error::UnauthorizedAppointmentAction)));
    }

    #[test]
    fn test_cancel_completed_appointment_fails() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        env.mock_all_auths();

        let datetime = 1640995200;
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        client.complete_appointment(&doctor, &appointment_id);
        let result = client.try_cancel_appointment(&patient, &appointment_id);
        assert_eq!(result, Err(Ok(Error::InvalidAppointmentStatus)));
    }

    #[test]
    fn test_complete_appointment() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        env.mock_all_auths();

        let datetime = 1640995200;
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        client.complete_appointment(&doctor, &appointment_id);

        let doctor_appointments = client.get_appointments(&doctor);
        let appointment = &doctor_appointments.get(0).unwrap();
        assert!(matches!(appointment.status, AppointmentStatus::Completed));
    }

    #[test]
    fn test_unauthorized_complete_appointment() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        let unauthorized_user = Address::generate(&env);
        env.mock_all_auths();

        let datetime = 1640995200;
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        let result = client.try_complete_appointment(&unauthorized_user, &appointment_id);
        assert_eq!(result, Err(Ok(Error::UnauthorizedAppointmentAction)));
    }

    #[test]
    fn test_get_appointments_for_user() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        let patient2 = Address::generate(&env);
        env.mock_all_auths();

        let datetime1 = 1640995200;
        let datetime2 = 1641081600; // Next day

        // Create appointments for patient with doctor
        let appointment_id1 = client.create_appointment(&patient, &doctor, &datetime1);
        let appointment_id2 = client.create_appointment(&patient, &doctor, &datetime2);

        // Create appointment for patient2 with doctor
        env.mock_all_auths();
        let appointment_id3 = client.create_appointment(&patient2, &doctor, &datetime1);

        // Check patient's appointments
        let patient_appointments = client.get_appointments(&patient);
        assert_eq!(patient_appointments.len(), 2);

        let mut appointment_ids = Vec::new(&env);
        for appt in patient_appointments.iter() {
            appointment_ids.push_back(appt.id);
        }
        assert!(appointment_ids.contains(appointment_id1));
        assert!(appointment_ids.contains(appointment_id2));
        assert!(!appointment_ids.contains(appointment_id3));

        // Check doctor's appointments
        let doctor_appointments = client.get_appointments(&doctor);
        assert_eq!(doctor_appointments.len(), 3);

        let mut doctor_appointment_ids = Vec::new(&env);
        for appt in doctor_appointments.iter() {
            doctor_appointment_ids.push_back(appt.id);
        }
        assert!(doctor_appointment_ids.contains(appointment_id1));
        assert!(doctor_appointment_ids.contains(appointment_id2));
        assert!(doctor_appointment_ids.contains(appointment_id3));
    }

    #[test]
    fn test_multiple_appointments_workflow() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        env.mock_all_auths();

        // Create multiple appointments
        let datetime1 = 1640995200;
        let datetime2 = 1641081600;
        let datetime3 = 1641168000;

        let id1 = client.create_appointment(&patient, &doctor, &datetime1);
        let id2 = client.create_appointment(&patient, &doctor, &datetime2);
        let _id3 = client.create_appointment(&patient, &doctor, &datetime3);

        // Cancel one
        client.cancel_appointment(&patient, &id2);

        // Complete one
        client.complete_appointment(&doctor, &id1);

        // Check final state
        let appointments = client.get_appointments(&patient);
        assert_eq!(appointments.len(), 3);

        let mut scheduled_count = 0;
        let mut canceled_count = 0;
        let mut completed_count = 0;

        for appointment in appointments.iter() {
            match appointment.status {
                AppointmentStatus::Scheduled => scheduled_count += 1,
                AppointmentStatus::Canceled => canceled_count += 1,
                AppointmentStatus::Completed => completed_count += 1,
            }
        }

        assert_eq!(scheduled_count, 1); // id3
        assert_eq!(canceled_count, 1); // id2
        assert_eq!(completed_count, 1); // id1
    }

    // =========================================================================
    // safe_increment / safe_increment_persistent tests
    // =========================================================================

    #[test]
    fn test_safe_increment_starts_at_one() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            let id = safe_increment(&env, &DataKey::PendingAdmin); // reuse any key type
            assert_eq!(id, 1);
        });
    }

    #[test]
    fn test_safe_increment_is_sequential() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            assert_eq!(safe_increment(&env, &DataKey::PendingAdmin), 1);
            assert_eq!(safe_increment(&env, &DataKey::PendingAdmin), 2);
            assert_eq!(safe_increment(&env, &DataKey::PendingAdmin), 3);
        });
    }

    #[test]
    fn test_safe_increment_persistent_starts_at_one() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            let id = safe_increment_persistent(&env, &DataKey::PendingAdmin);
            assert_eq!(id, 1);
        });
    }

    #[test]
    fn test_safe_increment_persistent_is_sequential() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            assert_eq!(safe_increment_persistent(&env, &DataKey::PendingAdmin), 1);
            assert_eq!(safe_increment_persistent(&env, &DataKey::PendingAdmin), 2);
            assert_eq!(safe_increment_persistent(&env, &DataKey::PendingAdmin), 3);
        });
    }

    #[test]
    fn test_instance_and_persistent_counters_are_independent() {
        // The same key in instance vs persistent storage must not share state.
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            // Advance instance counter to 3.
            safe_increment(&env, &DataKey::PendingAdmin);
            safe_increment(&env, &DataKey::PendingAdmin);
            safe_increment(&env, &DataKey::PendingAdmin);

            // Persistent counter for the same key must still start at 1.
            assert_eq!(safe_increment_persistent(&env, &DataKey::PendingAdmin), 1);
        });
    }

    #[test]
    fn test_safe_increment_ns_isolates_namespaces() {
        use soroban_sdk::Symbol;

        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            let ns_a = Symbol::new(&env, "patient_a");
            let ns_b = Symbol::new(&env, "patient_b");
            let sub = Symbol::new(&env, "records");

            // Advance patient_a counter to 5.
            for _ in 0..5 {
                safe_increment_ns(&env, &ns_a, &sub);
            }

            // patient_b counter must be independent and start at 1.
            assert_eq!(safe_increment_ns(&env, &ns_b, &sub), 1);

            // patient_a counter continues from 6.
            assert_eq!(safe_increment_ns(&env, &ns_a, &sub), 6);
        });
    }

    #[test]
    fn test_safe_increment_persistent_ns_isolates_namespaces() {
        use soroban_sdk::Symbol;

        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            let ns_a = Symbol::new(&env, "hosp_a");
            let ns_b = Symbol::new(&env, "hosp_b");
            let sub = Symbol::new(&env, "claims");

            safe_increment_persistent_ns(&env, &ns_a, &sub);
            safe_increment_persistent_ns(&env, &ns_a, &sub);

            // hosp_b must start at 1 regardless of hosp_a's counter.
            assert_eq!(safe_increment_persistent_ns(&env, &ns_b, &sub), 1);
        });
    }

    #[test]
    fn test_different_sub_keys_in_same_namespace_are_independent() {
        use soroban_sdk::Symbol;

        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            let ns = Symbol::new(&env, "patient_x");
            let sub_records = Symbol::new(&env, "records");
            let sub_visits = Symbol::new(&env, "visits");

            safe_increment_ns(&env, &ns, &sub_records);
            safe_increment_ns(&env, &ns, &sub_records);

            // visits counter for the same patient must start at 1.
            assert_eq!(safe_increment_ns(&env, &ns, &sub_visits), 1);
        });
    }

    #[test]
    #[should_panic(expected = "counter overflow")]
    fn test_safe_increment_panics_on_overflow() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            // Seed the counter at u64::MAX so the next increment overflows.
            env.storage()
                .instance()
                .set(&DataKey::PendingAdmin, &u64::MAX);
            safe_increment(&env, &DataKey::PendingAdmin);
        });
    }

    #[test]
    #[should_panic(expected = "counter overflow")]
    fn test_safe_increment_persistent_panics_on_overflow() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(HealthcareRegistry, (&Address::generate(&env),));

        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::PendingAdmin, &u64::MAX);
            safe_increment_persistent(&env, &DataKey::PendingAdmin);
        });
    }
}
