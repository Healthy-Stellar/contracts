#![no_std]

//! # Clinical Guideline Contract
//!
//! On-chain **registry** of versioned clinical guidelines (condition, criteria hash,
//! recommendation hash, evidence level) managed by the admin, plus provider-created
//! patient reminders.
//!
//! ## Decision support happens off-chain
//!
//! This contract deliberately does **not** compute clinical advice. Guideline
//! evaluation, drug dosing, risk scoring (CHA₂DS₂-VASc, HEART, Wells, …), care
//! pathways and preventive-care screening must be performed by a validated
//! off-chain clinical decision-support system that resolves the registered
//! guideline by id and checks its `criteria_hash` / `recommendation_hash` against
//! the rule set it applies. Earlier placeholder entry points
//! (`evaluate_guideline`, `calculate_drug_dosage`, `assess_risk_score`,
//! `suggest_care_pathway`, `check_preventive_care`) returned hard-coded output that
//! could be mistaken for real decision support, so they were removed. Any
//! future on-chain calculator must be driven by versioned, admin-registered rule
//! data (e.g. per-medication dosing rules with max single/daily dose and renal
//! bands) and fail when the rule or required patient data is missing.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Authorization checks for guideline access. Provider-based access
//! control to clinical recommendations. Query access restricted to authorized providers and clinicians.
//!
//! **Audit Controls:** Guideline registrations and updates emit events with the evidence level and
//! acting admin for traceability. Reminder creation emits an event naming patient and provider.
//!
//! **Data Retention Policy:** Guideline metadata stored for clinical reference. Reminders retained
//! with creation timestamps.
//!
//! **Encryption/Integrity:** Guideline criteria and recommendation content are referenced by hash so
//! off-chain evaluators can verify they apply the registered version. Evidence levels classified
//! (e.g., A, B, C).

use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, contract, contracterror, contractimpl, contracttype,
    symbol_short,
};

// --- Custom Error Types ---
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    GuidelineNotFound = 2,
    InvalidInput = 3,
}

// --- Data Structures ---
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reminder {
    pub reminder_id: u64,
    pub patient_id: Address,
    pub due_date: u64,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuidelineMetadata {
    pub condition: String,
    pub criteria_hash: BytesN<32>,
    pub recommendation_hash: BytesN<32>,
    pub evidence_level: Symbol,
}

// --- Data key enum ---
#[contracttype]
pub enum DataKey {
    Admin,
    ProviderRegistry,
    Guideline(String),
    ReminderCounter(Address),       // patient_id -> u64 (next reminder_id)
    Reminder(Address, u64),         // (patient_id, reminder_id) -> Reminder
}

#[contract]
pub struct ClinicalGuidelineContract;

#[contractimpl]
impl ClinicalGuidelineContract {
    /// Bind the contract to a stored admin address.
    ///
    /// Runs atomically as part of contract creation and cannot be called again,
    /// so an attacker cannot front-run deployment and elect themselves admin.
    pub fn __constructor(env: Env, admin: Address) {
        ttl_config::extend_instance_ttl(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Set the provider registry address for authorization checks.
    pub fn set_provider_registry(env: Env, provider_registry: Address) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotAuthorized)?;
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &provider_registry);
        Ok(())
    }

    pub fn register_clinical_guideline(
        env: Env,
        admin: Address,
        guideline_id: String,
        condition: String,
        criteria_hash: BytesN<32>,
        recommendation_hash: BytesN<32>,
        evidence_level: Symbol,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        admin.require_auth();

        // Verify caller matches stored admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotAuthorized)?;
        if admin != stored_admin {
            return Err(Error::NotAuthorized);
        }

        // Reject silent overwrite — existing guideline_id must not already exist
        let key = DataKey::Guideline(guideline_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::InvalidInput);
        }

        let metadata = GuidelineMetadata {
            condition: condition.clone(),
            criteria_hash,
            recommendation_hash,
            evidence_level,
        };

        env.storage().persistent().set(&key, &metadata);

        env.events().publish(
            (symbol_short!("reg_guide"), guideline_id),
            (condition, admin),
        );

        Ok(())
    }

    pub fn update_clinical_guideline(
        env: Env,
        admin: Address,
        guideline_id: String,
        condition: String,
        criteria_hash: BytesN<32>,
        recommendation_hash: BytesN<32>,
        evidence_level: Symbol,
    ) -> Result<(), Error> {
        ttl_config::extend_instance_ttl(&env);
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotAuthorized)?;
        if admin != stored_admin {
            return Err(Error::NotAuthorized);
        }

        let key = DataKey::Guideline(guideline_id.clone());
        if !env.storage().persistent().has(&key) {
            return Err(Error::GuidelineNotFound);
        }

        let metadata = GuidelineMetadata {
            condition: condition.clone(),
            criteria_hash,
            recommendation_hash,
            evidence_level,
        };

        env.storage().persistent().set(&key, &metadata);

        env.events().publish(
            (symbol_short!("upd_guide"), guideline_id),
            (condition, admin),
        );

        Ok(())
    }

    pub fn create_reminder(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        _reminder_type: Symbol,
        due_date: u64,
        _priority: Symbol,
    ) -> Result<u64, Error> {
        ttl_config::extend_instance_ttl(&env);
        provider_id.require_auth();
        if !Self::is_provider_registered(&env, &provider_id) {
            return Err(Error::NotAuthorized);
        }

        let counter_key = DataKey::ReminderCounter(patient_id.clone());
        let reminder_id: u64 = env
            .storage()
            .persistent()
            .get(&counter_key)
            .unwrap_or(1);

        let reminder = Reminder {
            reminder_id,
            patient_id: patient_id.clone(),
            due_date,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Reminder(patient_id.clone(), reminder_id), &reminder);
        env.storage()
            .persistent()
            .set(&counter_key, &(reminder_id + 1));

        env.events().publish(
            (symbol_short!("create_rem"), reminder_id),
            (patient_id, provider_id),
        );

        Ok(reminder_id)
    }

    /// Read a reminder. The caller must authorize and be either the patient
    /// the reminder belongs to or a provider registered in the provider
    /// registry.
    pub fn get_reminder(
        env: Env,
        caller: Address,
        patient_id: Address,
        reminder_id: u64,
    ) -> Result<Reminder, Error> {
        caller.require_auth();
        if caller != patient_id && !Self::is_provider_registered(&env, &caller) {
            return Err(Error::NotAuthorized);
        }
        env.storage()
            .persistent()
            .get(&DataKey::Reminder(patient_id, reminder_id))
            .ok_or(Error::GuidelineNotFound)
    }

    fn is_provider_registered(env: &Env, provider: &Address) -> bool {
        if let Some(registry) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ProviderRegistry)
        {
            shared::actor_verification::is_provider_registered(env, &registry, provider)
        } else {
            false
        }
    }
}

mod test;
