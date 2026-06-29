#![no_std]
#![allow(non_snake_case)]

use soroban_sdk::{
    contract, contractevent, contracterror, contractimpl, contracttype, symbol_short, Address, Env,
    String, Symbol, Vec,
};
use shared::{events::EVENT_VERSION, temporal};

// =============================================================================
// Shared counter utilities
// =============================================================================
//
// Every contract that needs a monotonically-increasing ID should call one of
// these helpers instead of open-coding `unwrap_or(0) + 1`.  The helpers:
//
//   1. Read the current value (defaulting to 0 on first use).
//   2. Perform a checked add — panicking on overflow so the contract halts
//      rather than silently wrapping and reusing an old ID.
//   3. Persist the incremented value back to storage.
//   4. Return the *new* value (i.e. the ID to use for the record being created).
//
// Two storage tiers are provided:
//   • `safe_increment`            — instance storage  (cheap, contract-lifetime)
//   • `safe_increment_persistent` — persistent storage (survives ledger archival)
//
// A namespaced variant is provided for per-entity counters (e.g. per-patient
// record sequences) where two entities must never share an ID space:
//   • `safe_increment_ns`            — instance storage,   key = (namespace, sub_key)
//   • `safe_increment_persistent_ns` — persistent storage, key = (namespace, sub_key)
//
// All four functions are generic over the storage-key type so callers can pass
// any `contracttype`-derived key without boxing.

/// Increment a `u64` counter stored in **instance** storage and return the new
/// value.  Panics with `"counter overflow"` if the counter would exceed
/// `u64::MAX`.
pub fn safe_increment<K>(env: &Env, key: &K) -> u64
where
    K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
{
    let current: u64 = env.storage().instance().get(key).unwrap_or(0u64);
    let next = current.checked_add(1).expect("counter overflow");
    env.storage().instance().set(key, &next);
    next
}

/// Increment a `u64` counter stored in **persistent** storage and return the
/// new value.  Panics with `"counter overflow"` if the counter would exceed
/// `u64::MAX`.
pub fn safe_increment_persistent<K>(env: &Env, key: &K) -> u64
where
    K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
{
    let current: u64 = env.storage().persistent().get(key).unwrap_or(0u64);
    let next = current.checked_add(1).expect("counter overflow");
    env.storage().persistent().set(key, &next);
    next
}

/// Increment a namespaced `u64` counter in **instance** storage.
///
/// The storage key is `(namespace, sub_key)`, keeping per-entity counters
/// isolated from each other and from global counters.
pub fn safe_increment_ns(env: &Env, namespace: &soroban_sdk::Symbol, sub_key: &soroban_sdk::Symbol) -> u64 {
    let compound = (namespace, sub_key);
    let current: u64 = env.storage().instance().get(&compound).unwrap_or(0u64);
    let next = current.checked_add(1).expect("counter overflow");
    env.storage().instance().set(&compound, &next);
    next
}

/// Increment a namespaced `u64` counter in **persistent** storage.
///
/// The storage key is `(namespace, sub_key)`, keeping per-entity counters
/// isolated from each other and from global counters.
pub fn safe_increment_persistent_ns(env: &Env, namespace: &soroban_sdk::Symbol, sub_key: &soroban_sdk::Symbol) -> u64 {
    let compound = (namespace, sub_key);
    let current: u64 = env.storage().persistent().get(&compound).unwrap_or(0u64);
    let next = current.checked_add(1).expect("counter overflow");
    env.storage().persistent().set(&compound, &next);
    next
}

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
    PendingAdmin,
}

const ADMIN_PROPOSED: &str = "admin_proposed";
const ADMIN_ACCEPTED: &str = "admin_accepted";
const ADMIN_TRANSFER_CANCELLED: &str = "admin_transfer_cancelled";

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
    /// datetime must be strictly in the future for new appointments
    InvalidDatetime = 7,
}

/// Versioned event: a new appointment was scheduled.
#[contractevent]
pub struct AppointmentCreated {
    pub version: u32,
    pub appointment_id: u64,
    pub patient: Address,
    pub doctor: Address,
}

/// Versioned event: an appointment was cancelled by the patient.
#[contractevent]
pub struct AppointmentCancelled {
    pub version: u32,
    pub appointment_id: u64,
}

/// Versioned event: an appointment was marked completed by the doctor.
#[contractevent]
pub struct AppointmentCompleted {
    pub version: u32,
    pub appointment_id: u64,
}

#[contract]
pub struct HealthcareRegistry;

#[contractimpl]
impl HealthcareRegistry {
    // Set an admin/verifier during initialization
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn propose_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotAuthorized)?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);
        env.events()
            .publish((Symbol::new(&env, ADMIN_PROPOSED),), new_admin);
        Ok(())
    }

    pub fn accept_admin(env: Env) -> Result<(), Error> {
        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .ok_or(Error::NotFound)?;
        pending.require_auth();

        env.storage().instance().set(&DataKey::Admin, &pending);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events()
            .publish((Symbol::new(&env, ADMIN_ACCEPTED),), pending);
        Ok(())
    }

    pub fn cancel_admin_transfer(env: Env) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotAuthorized)?;
        admin.require_auth();

        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events()
            .publish((Symbol::new(&env, ADMIN_TRANSFER_CANCELLED),), admin);
        Ok(())
    }

    pub fn register_institution(
        env: Env,
        wallet: Address,
        name: String,
        license_id: String,
        metadata: String,
    ) -> Result<(), Error> {
        wallet.require_auth();

        let key = DataKey::Inst(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyRegistered);
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
        Ok(())
    }

    pub fn get_institution(env: Env, wallet: Address) -> Result<InstitutionData, Error> {
        let key = DataKey::Inst(wallet);
        env.storage().persistent().get(&key).ok_or(Error::NotFound)
    }

    pub fn update_institution(env: Env, wallet: Address, metadata: String) -> Result<(), Error> {
        wallet.require_auth();

        let key = DataKey::Inst(wallet.clone());
        let mut data: InstitutionData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;

        data.metadata = metadata;
        env.storage().persistent().set(&key, &data);
        Ok(())
    }

    pub fn verify_institution(env: Env, verifier: Address, wallet: Address) -> Result<(), Error> {
        verifier.require_auth();

        // Access Control: Check if caller is the admin
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotAuthorized)?;
        if verifier != admin {
            return Err(Error::NotAuthorized);
        }

        let key = DataKey::Inst(wallet.clone());
        let mut data: InstitutionData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;

        data.is_verified = true;
        env.storage().persistent().set(&key, &data);
        Ok(())
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

    pub fn cancel_appointment(
        env: Env,
        patient: Address,
        appointment_id: u64,
    ) -> Result<(), Error> {
        patient.require_auth();

        let appointment_key = AppointmentKey::Appointment(appointment_id);
        let mut appointment: Appointment = env
            .storage()
            .persistent()
            .get(&appointment_key)
            .ok_or(Error::AppointmentNotFound)?;

        // Only patient can cancel, and only if appointment is scheduled
        if appointment.patient != patient {
            return Err(Error::UnauthorizedAppointmentAction);
        }

        if !matches!(appointment.status, AppointmentStatus::Scheduled) {
            return Err(Error::InvalidAppointmentStatus);
        }

        appointment.status = AppointmentStatus::Canceled;
        env.storage()
            .persistent()
            .set(&appointment_key, &appointment);

        // Emit event
        env.events()
            .publish((symbol_short!("appt_can"), appointment_id), patient);
        Ok(())
    }

    pub fn complete_appointment(
        env: Env,
        doctor: Address,
        appointment_id: u64,
    ) -> Result<(), Error> {
        doctor.require_auth();

        let appointment_key = AppointmentKey::Appointment(appointment_id);
        let mut appointment: Appointment = env
            .storage()
            .persistent()
            .get(&appointment_key)
            .ok_or(Error::AppointmentNotFound)?;

        // Only doctor can complete, and only if appointment is scheduled
        if appointment.doctor != doctor {
            return Err(Error::UnauthorizedAppointmentAction);
        }

        if !matches!(appointment.status, AppointmentStatus::Scheduled) {
            return Err(Error::InvalidAppointmentStatus);
        }

        appointment.status = AppointmentStatus::Completed;
        env.storage()
            .persistent()
            .set(&appointment_key, &appointment);

        // Emit event
        env.events()
            .publish((symbol_short!("appt_cmp"), appointment_id), doctor);
        Ok(())
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
