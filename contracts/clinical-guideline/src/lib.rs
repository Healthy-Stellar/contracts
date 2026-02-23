#![no_std]
use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Vec, contract, contracterror, contractimpl, contracttype,
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
pub struct GuidelineRecommendation {
    pub guideline_id: String,
    pub applicable: bool,
    pub recommendation: String,
    pub strength: Symbol,
    pub evidence_level: Symbol,
    pub alternative_options: Vec<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DosageRecommendation {
    pub medication: String,
    pub recommended_dose: String,
    pub frequency: String,
    pub route: Symbol,
    pub duration: Option<u64>,
    pub renal_adjustment: bool,
    pub monitoring_required: Vec<String>,
}

// Placeholder for logic-heavy structures
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarePathway {
    pub condition: String,
    pub steps: Vec<String>,
}

#[contract]
pub struct ClinicalGuidelineContract;

#[contractimpl]
impl ClinicalGuidelineContract {
    pub fn register_clinical_guideline(
        env: Env,
        admin: Address,
        guideline_id: String,
        _condition: String,
        criteria_hash: BytesN<32>,
        _recommendation_hash: BytesN<32>,
        _evidence_level: Symbol,
    ) -> Result<(), Error> {
        admin.require_auth();
        // Use guideline_id as the storage key
        env.storage()
            .persistent()
            .set(&guideline_id, &criteria_hash);
        Ok(())
    }

    pub fn evaluate_guideline(
        env: Env,
        _patient_id: Address,
        _provider_id: Address,
        guideline_id: String,
        patient_data_hash: BytesN<32>,
    ) -> Result<GuidelineRecommendation, Error> {
        // Retrieve stored criteria
        let stored_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&guideline_id)
            .ok_or(Error::GuidelineNotFound)?;

        let is_applicable = stored_hash == patient_data_hash;

        Ok(GuidelineRecommendation {
            guideline_id,
            applicable: is_applicable,
            recommendation: String::from_str(&env, "Follow Standard Protocol"),
            strength: Symbol::new(&env, "High"),
            evidence_level: Symbol::new(&env, "Level_A"),
            alternative_options: Vec::new(&env),
        })
    }

    pub fn calculate_drug_dosage(
        env: Env,
        _patient_id: Address,
        medication: String,
        weight_grams: u64, // Used u64 for fixed-point math instead of f32
        _age: u32,
        renal_function: Option<u32>,
    ) -> Result<DosageRecommendation, Error> {
        // Simple example: 5mg per kg (1000g)
        let _dose_mg = (weight_grams * 5) / 1000;
        let is_renal_impaired = renal_function.unwrap_or(100) < 60;

        Ok(DosageRecommendation {
            medication,
            recommended_dose: String::from_str(&env, "5mg/kg"),
            frequency: String::from_str(&env, "QD"),
            route: Symbol::new(&env, "Oral"),
            duration: Some(10),
            renal_adjustment: is_renal_impaired,
            monitoring_required: Vec::new(&env),
        })
    }

    pub fn assess_risk_score(
        env: Env,
        _patient_id: Address,
        _risk_calculator: Symbol,
        input_parameters: Vec<i32>,
    ) -> Result<i32, Error> {
        let mut total_score: i32 = 0;
        for val in input_parameters.iter() {
            total_score += val;
        }
        Ok(total_score)
    }

    pub fn suggest_care_pathway(
        env: Env,
        _patient_id: Address,
        condition: String,
        _current_treatment: Vec<String>,
    ) -> Result<CarePathway, Error> {
        let mut steps = Vec::new(&env);
        steps.push_back(String::from_str(&env, "Initial Assessment"));
        steps.push_back(String::from_str(&env, "Lab Tests"));

        Ok(CarePathway { condition, steps })
    }

    pub fn create_reminder(
        env: Env,
        patient_id: Address,
        _provider_id: Address,
        _reminder_type: Symbol,
        due_date: u64,
        _priority: Symbol,
    ) -> Result<u64, Error> {
        // Use ledger timestamp + patient address hash as a simple ID
        let reminder_id = env.ledger().timestamp();
        env.storage().temporary().set(&patient_id, &due_date);
        Ok(reminder_id)
    }

    pub fn check_preventive_care(
        env: Env,
        _patient_id: Address,
        age: u32,
        _gender: Symbol,
        _risk_factors: Vec<Symbol>,
    ) -> Result<Vec<Symbol>, Error> {
        let mut alerts = Vec::new(&env);

        if age > 45 {
            alerts.push_back(Symbol::new(&env, "Cardiac_Screening"));
        }
        if age > 18 {
            alerts.push_back(Symbol::new(&env, "Blood_Pressure_Check"));
        }

        Ok(alerts)
    }
}

mod test;
