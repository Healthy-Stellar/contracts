#![no_std]

use soroban_sdk::{contracterror, contracttype, Address, String, Symbol, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    ImmunizationCounter,
    PatientImmunizations(Address), // List of IDs (u64)
    ImmunizationRecord(u64),
    AdverseEvents(u64), // List of AdverseEvent
    PatientVaccineSeries(Address), // List of VaccineSeries
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    RecordNotFound = 2,
    InvalidDoseNumber = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaccineRecord {
    pub patient_id: Address,
    pub provider_id: Address,
    pub vaccine_name: String,
    pub cvx_code: String,
    pub lot_number: String,
    pub manufacturer: String,
    pub administration_date: u64,
    pub expiration_date: u64,
    pub dose_number: u32,
    pub route: Symbol,
    pub site: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdverseEvent {
    pub reporter: Address,
    pub event_description: String,
    pub severity: Symbol,
    pub onset_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaccineSeries {
    pub series_name: String,
    pub doses_required: u32,
    pub schedule_hash: BytesN<32>,
}
