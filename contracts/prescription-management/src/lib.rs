#![no_std]

use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, contract, contracterror, contractimpl, contracttype,
    panic_with_error,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Expired = 1,
    Unauthorized = 2,
    InvalidPrescription = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrescriptionStatus {
    Active,
    Dispensed,
    Expired,
    Transferred,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Prescription {
    pub provider_id: Address,
    pub patient_id: Address,
    pub medication_name: String,
    pub quantity: u32,
    pub refills_remaining: u32,
    pub is_controlled: bool,
    pub current_pharmacy: Option<Address>,
    pub status: PrescriptionStatus,
    pub valid_until: u64,
    // Add additional fields here as needed
}

// Struct to bypass the 10-parameter limit
#[contracttype]
pub struct IssueRequest {
    pub medication_name: String,
    pub ndc_code: String,
    pub dosage: String,
    pub quantity: u32,
    pub days_supply: u32,
    pub refills_allowed: u32,
    pub instructions_hash: BytesN<32>,
    pub is_controlled: bool,
    pub schedule: Option<u32>,
    pub valid_until: u64,
    pub substitution_allowed: bool,
}

#[contract]
pub struct PrescriptionContract;

#[contractimpl]
impl PrescriptionContract {
    pub fn issue_prescription(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        req: IssueRequest,
    ) -> u64 {
        provider_id.require_auth();

        let id = env
            .storage()
            .instance()
            .get::<_, u64>(&Symbol::new(&env, "ID_COUNTER"))
            .unwrap_or(0);

        let prescription = Prescription {
            provider_id,
            patient_id,
            medication_name: req.medication_name,
            quantity: req.quantity,
            refills_remaining: req.refills_allowed,
            is_controlled: req.is_controlled,
            current_pharmacy: None,
            status: PrescriptionStatus::Active,
            valid_until: req.valid_until,
        };

        env.storage().persistent().set(&id, &prescription);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "ID_COUNTER"), &(id + 1));

        id
    }

    pub fn dispense_prescription(
        env: Env,
        prescription_id: u64,
        pharmacy_id: Address,
        _quantity: u32,
        _lot: String,
    ) {
        pharmacy_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&prescription_id)
            .expect("Prescription not found");

        if env.ledger().timestamp() > p.valid_until {
            panic_with_error!(&env, Error::Expired);
        }

        p.status = PrescriptionStatus::Dispensed;
        p.current_pharmacy = Some(pharmacy_id);

        env.storage().persistent().set(&prescription_id, &p);
    }

    pub fn transfer_prescription(
        env: Env,
        prescription_id: u64,
        from_pharmacy: Address,
        to_pharmacy: Address,
    ) {
        from_pharmacy.require_auth();

        let mut p: Prescription = env.storage().persistent().get(&prescription_id).unwrap();

        p.current_pharmacy = Some(to_pharmacy);
        p.status = PrescriptionStatus::Transferred;

        env.storage().persistent().set(&prescription_id, &p);
    }
}

mod test;
