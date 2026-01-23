use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstitutionData {
    pub name: String,
    pub license_id: String,
    pub metadata: String,
    pub is_verified: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Appointment {
    pub id: u64,
    pub patient: Address,
    pub doctor: Address,
    pub datetime: u64,
    pub status: AppointmentStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppointmentStatus {
    Scheduled,
    Canceled,
    Completed,
}

#[contracttype]
pub enum DataKey {
    Inst(Address),
    Admin, // To manage the 'verifier' role
}

#[contracttype]
pub enum AppointmentKey {
    Appointment(u64),
    AppointmentCounter,
    UserAppointments(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyRegistered = 1,
    NotFound = 2,
    NotAuthorized = 3,
    AppointmentNotFound = 4,
    InvalidAppointmentStatus = 5,
    UnauthorizedAppointmentAction = 6,
}

#[contract]
pub struct HealthcareRegistry;

#[contractimpl]
impl HealthcareRegistry {
    // Set an admin/verifier during initialization
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn register_institution(
        env: Env,
        wallet: Address,
        name: String,
        license_id: String,
        metadata: String,
    ) {
        wallet.require_auth();

        let key = DataKey::Inst(wallet.clone());
        if env.storage().persistent().has(&key) {
            panic!("Already registered");
        }

        let data = InstitutionData {
            name,
            license_id,
            metadata,
            is_verified: false,
        };

        env.storage().persistent().set(&key, &data);

        // Event emission
        env.events()
            .publish((symbol_short!("reg"), wallet), symbol_short!("success"));
    }

    pub fn get_institution(env: Env, wallet: Address) -> InstitutionData {
        let key = DataKey::Inst(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .expect("Institution not found")
    }

    pub fn update_institution(env: Env, wallet: Address, metadata: String) {
        wallet.require_auth();

        let key = DataKey::Inst(wallet.clone());
        let mut data: InstitutionData = env.storage().persistent().get(&key).expect("Not found");

        data.metadata = metadata;
        env.storage().persistent().set(&key, &data);
    }

    pub fn verify_institution(env: Env, verifier: Address, wallet: Address) {
        verifier.require_auth();

        // Access Control: Check if caller is the admin
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if verifier != admin {
            panic!("Not authorized to verify");
        }

        let key = DataKey::Inst(wallet.clone());
        let mut data: InstitutionData = env.storage().persistent().get(&key).expect("Not found");

        data.is_verified = true;
        env.storage().persistent().set(&key, &data);
    }
}

#[contract]
pub struct AppointmentScheduling;

#[contractimpl]
impl AppointmentScheduling {
    pub fn create_appointment(env: Env, patient: Address, doctor: Address, datetime: u64) -> u64 {
        patient.require_auth();

        // Get next appointment ID
        let counter_key = AppointmentKey::AppointmentCounter;
        let appointment_id = env.storage().persistent().get(&counter_key).unwrap_or(0u64) + 1;

        // Create appointment
        let appointment = Appointment {
            id: appointment_id,
            patient: patient.clone(),
            doctor: doctor.clone(),
            datetime,
            status: AppointmentStatus::Scheduled,
        };

        // Store appointment
        let appointment_key = AppointmentKey::Appointment(appointment_id);
        env.storage()
            .persistent()
            .set(&appointment_key, &appointment);

        // Update counter
        env.storage()
            .persistent()
            .set(&counter_key, &appointment_id);

        // Add to patient's appointments
        let patient_key = AppointmentKey::UserAppointments(patient.clone());
        let mut patient_appointments: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));
        patient_appointments.push_back(appointment_id);
        env.storage()
            .persistent()
            .set(&patient_key, &patient_appointments);

        // Add to doctor's appointments
        let doctor_key = AppointmentKey::UserAppointments(doctor.clone());
        let mut doctor_appointments: Vec<u64> = env
            .storage()
            .persistent()
            .get(&doctor_key)
            .unwrap_or(Vec::new(&env));
        doctor_appointments.push_back(appointment_id);
        env.storage()
            .persistent()
            .set(&doctor_key, &doctor_appointments);

        // Emit event
        env.events().publish(
            (symbol_short!("appt_cr"), appointment_id),
            (patient, doctor),
        );

        appointment_id
    }

    pub fn cancel_appointment(env: Env, patient: Address, appointment_id: u64) {
        patient.require_auth();

        let appointment_key = AppointmentKey::Appointment(appointment_id);
        let mut appointment: Appointment = env
            .storage()
            .persistent()
            .get(&appointment_key)
            .ok_or(Error::AppointmentNotFound)
            .unwrap();

        // Only patient can cancel, and only if appointment is scheduled
        if appointment.patient != patient {
            panic!("Unauthorized to cancel this appointment");
        }

        if !matches!(appointment.status, AppointmentStatus::Scheduled) {
            panic!("Can only cancel scheduled appointments");
        }

        appointment.status = AppointmentStatus::Canceled;
        env.storage()
            .persistent()
            .set(&appointment_key, &appointment);

        // Emit event
        env.events()
            .publish((symbol_short!("appt_can"), appointment_id), patient);
    }

    pub fn complete_appointment(env: Env, doctor: Address, appointment_id: u64) {
        doctor.require_auth();

        let appointment_key = AppointmentKey::Appointment(appointment_id);
        let mut appointment: Appointment = env
            .storage()
            .persistent()
            .get(&appointment_key)
            .ok_or(Error::AppointmentNotFound)
            .unwrap();

        // Only doctor can complete, and only if appointment is scheduled
        if appointment.doctor != doctor {
            panic!("Unauthorized to complete this appointment");
        }

        if !matches!(appointment.status, AppointmentStatus::Scheduled) {
            panic!("Can only complete scheduled appointments");
        }

        appointment.status = AppointmentStatus::Completed;
        env.storage()
            .persistent()
            .set(&appointment_key, &appointment);

        // Emit event
        env.events()
            .publish((symbol_short!("appt_cmp"), appointment_id), doctor);
    }

    pub fn get_appointments(env: Env, user: Address) -> Vec<Appointment> {
        let user_key = AppointmentKey::UserAppointments(user);
        let appointment_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&user_key)
            .unwrap_or(Vec::new(&env));

        let mut appointments = Vec::new(&env);
        for id in appointment_ids.iter() {
            if let Some(appointment) = env
                .storage()
                .persistent()
                .get(&AppointmentKey::Appointment(id))
            {
                appointments.push_back(appointment);
            }
        }

        appointments
    }
}

mod test;
