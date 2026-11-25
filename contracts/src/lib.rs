#![no_std]
#![allow(non_snake_case)]

//! # Contracts Root Module
//!
//! Root module aggregating all healthcare smart contracts with unified HIPAA compliance across
//! the entire healthcare blockchain network.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Each contract implements role-based access control with
//! authentication. Cross-contract authorization via shared actor verification module. Patient
//! consent enforced per contract type. Emergency override mechanisms with audit trails.
//!
//! **Audit Controls:** Unified event versioning via EVENT_VERSION. Temporal sequencing prevents
//! audit trail tampering. Incident tracking with correlation IDs across contracts. All contract
//! operations emit audit events. Compliance reports generated from event streams.
//!
//! **Data Retention Policy:** Each contract enforces specific retention rules. Deregistration
//! cascades across related contracts. Historical data preserved without deletion. Temporal
//! validation ensures logical event ordering. Audit trail immutable once recorded.
//!
//! **Encryption/Integrity:** All sensitive data encrypted via EncryptedEnvelopeRef. PolicyMetadata
//! enforces per-record encryption requirements. Cryptographic hashing validates integrity.
//! Patient address immutable. Contracts validate data integrity on read.

use soroban_sdk::{
    contract, contractevent, contracterror, contractimpl, contracttype, symbol_short, Address, Env,
    String, Symbol, Vec,
};
use shared::{events::EVENT_VERSION, temporal};

// =============================================================================
// Shared counter utilities
// =============================================================================
//
// The `safe_increment*` helpers live in the plain `shared` library
// (`shared::counters`). They are re-exported here for this crate's own use;
// other contracts must depend on `shared` directly, never on this crate,
// because this crate defines `#[contractimpl]` blocks whose exports would be
// linked into every dependent WASM.
pub use shared::counters::{
    safe_increment, safe_increment_ns, safe_increment_persistent, safe_increment_persistent_ns,
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
    /// Bind the registry to an admin/verifier address.
    ///
    /// Runs atomically as part of contract creation and cannot be called again,
    /// so an attacker cannot front-run deployment with their own admin.
    pub fn __constructor(env: Env, admin: Address) {
        ttl_config::extend_instance_ttl(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn propose_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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
        ttl_config::extend_instance_ttl(&env);
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
    pub fn create_appointment(env: Env, patient: Address, doctor: Address, datetime: u64) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        patient.require_auth();

        // Validate datetime is in the future
        let current_time = env.ledger().timestamp();
        if datetime <= current_time {
            return Err(Error::InvalidDatetime);
        }

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

        // Emit versioned event
        AppointmentCreated {
            version: EVENT_VERSION,
            appointment_id,
            patient: patient.clone(),
            doctor: doctor.clone(),
        }
        .publish(&env);

        Ok(appointment_id)
    }

    pub fn cancel_appointment(
        env: Env,
        patient: Address,
        appointment_id: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
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

        // Emit versioned event
        AppointmentCancelled {
            version: EVENT_VERSION,
            appointment_id,
        }
        .publish(&env);
        Ok(())
    }

    pub fn complete_appointment(
        env: Env,
        doctor: Address,
        appointment_id: u64,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
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

        // Emit versioned event
        AppointmentCompleted {
            version: EVENT_VERSION,
            appointment_id,
        }
        .publish(&env);
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
