#[cfg(test)]
mod tests {
    use crate::{
        AppointmentScheduling, AppointmentSchedulingClient, AppointmentStatus, DataKey,
        HealthcareRegistry, HealthcareRegistryClient,
    };

    use soroban_sdk::{
        testutils::{Address as _, MockAuth, MockAuthInvoke},
        Address, Env, IntoVal, String, Vec,
    };

    fn setup_registry_test(
        env: &Env,
    ) -> (Address, HealthcareRegistryClient<'static>, Address, Address) {
        let contract_id = env.register(HealthcareRegistry, ());
        let client = HealthcareRegistryClient::new(env, &contract_id);

        let admin = Address::generate(env);
        let institution = Address::generate(env);

        client.init(&admin);

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
    #[should_panic(expected = "Already registered")]
    fn test_duplicate_registration_fails() {
        let env = Env::default();
        let (client, _, inst_addr) = setup_test(&env);
        env.mock_all_auths();

        let name = String::from_str(&env, "Clinic A");
        client.register_institution(&inst_addr, &name, &name, &name);
        client.register_institution(&inst_addr, &name, &name, &name);
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
    #[should_panic(expected = "Not authorized to verify")]
    fn test_unauthorized_verification_fails() {
        let env = Env::default();
        let (client, _, inst_addr) = setup_test(&env);
        let fake_admin = Address::generate(&env);
        env.mock_all_auths();

        let name = String::from_str(&env, "Clinic A");
        client.register_institution(&inst_addr, &name, &name, &name);

        client.verify_institution(&fake_admin, &inst_addr);
    }

    #[test]
    fn test_propose_and_accept_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, _client, _admin, _) = setup_registry_test(&env);
        let new_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            HealthcareRegistry::propose_admin(env.clone(), new_admin.clone());
        });

        assert_eq!(
            stored_pending_admin(&env, &contract_id),
            Some(new_admin.clone())
        );

        env.as_contract(&contract_id, || {
            HealthcareRegistry::accept_admin(env.clone());
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
            HealthcareRegistry::propose_admin(env.clone(), new_admin.clone());
        });

        env.as_contract(&contract_id, || {
            HealthcareRegistry::cancel_admin_transfer(env.clone());
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
    #[should_panic(expected = "Unauthorized to cancel this appointment")]
    fn test_unauthorized_cancel_appointment() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        let unauthorized_user = Address::generate(&env);
        env.mock_all_auths();

        let datetime = 1640995200;
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        client.cancel_appointment(&unauthorized_user, &appointment_id);
    }

    #[test]
    #[should_panic(expected = "Can only cancel scheduled appointments")]
    fn test_cancel_completed_appointment_fails() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        env.mock_all_auths();

        let datetime = 1640995200;
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        client.complete_appointment(&doctor, &appointment_id);
        client.cancel_appointment(&patient, &appointment_id);
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
    #[should_panic(expected = "Unauthorized to complete this appointment")]
    fn test_unauthorized_complete_appointment() {
        let env = Env::default();
        let (client, patient, doctor) = setup_appointment_test(&env);
        let unauthorized_user = Address::generate(&env);
        env.mock_all_auths();

        let datetime = 1640995200;
        let appointment_id = client.create_appointment(&patient, &doctor, &datetime);

        client.complete_appointment(&unauthorized_user, &appointment_id);
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
}
