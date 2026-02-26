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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskScore {
    pub calculator: Symbol,
    pub score: i32,
    pub interpretation: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarePathway {
    pub condition: String,
    pub steps: Vec<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuidelineMetadata {
    pub condition: String,
    pub criteria_hash: BytesN<32>,
    pub recommendation_hash: BytesN<32>,
    pub evidence_level: Symbol,
}

#[contract]
pub struct ClinicalGuidelineContract;

#[contractimpl]
impl ClinicalGuidelineContract {
    pub fn register_clinical_guideline(
        env: Env,
        admin: Address,
        guideline_id: String,
        condition: String,
        criteria_hash: BytesN<32>,
        recommendation_hash: BytesN<32>,
        evidence_level: Symbol,
    ) -> Result<(), Error> {
        admin.require_auth();

        let metadata = GuidelineMetadata {
            condition,
            criteria_hash,
            recommendation_hash,
            evidence_level,
        };

        env.storage().persistent().set(&guideline_id, &metadata);
        Ok(())
    }

    pub fn evaluate_guideline(
        env: Env,
        _patient_id: Address,
        _provider_id: Address,
        guideline_id: String,
        patient_data_hash: BytesN<32>,
    ) -> Result<GuidelineRecommendation, Error> {
        let metadata: GuidelineMetadata = env
            .storage()
            .persistent()
            .get(&guideline_id)
            .ok_or(Error::GuidelineNotFound)?;

        let is_applicable = metadata.criteria_hash == patient_data_hash;

        Ok(GuidelineRecommendation {
            guideline_id,
            applicable: is_applicable,
            recommendation: String::from_str(&env, "Follow evidence-based recommendation"),
            strength: Symbol::new(&env, "Strong"),
            evidence_level: metadata.evidence_level,
            alternative_options: Vec::new(&env),
        })
    }

    pub fn calculate_drug_dosage(
        env: Env,
        _patient_id: Address,
        medication: String,
        weight_dg: u32, // Decigrams (0.1g) to avoid f32
        _age: u32,
        renal_function: Option<u32>,
    ) -> Result<DosageRecommendation, Error> {
        let is_renal_impaired = renal_function.unwrap_or(100) < 60;

        // Example: 5mg per kg (1000g = 10000dg)
        // dosage = (weight_dg / 10000) * 5
        let dose_mg = (weight_dg as u64 * 5) / 10000;

        Ok(DosageRecommendation {
            medication,
            recommended_dose: String::from_str(&env, "Dosage calculated based on weight"),
            frequency: String::from_str(&env, "TID"),
            route: Symbol::new(&env, "Oral"),
            duration: Some(dose_mg), // Simplified
            renal_adjustment: is_renal_impaired,
            monitoring_required: Vec::new(&env),
        })
    }

    pub fn assess_risk_score(
        env: Env,
        _patient_id: Address,
        risk_calculator: Symbol,
        input_parameters: Vec<i32>,
    ) -> Result<RiskScore, Error> {
        let mut total_score: i32 = 0;
        for val in input_parameters.iter() {
            total_score += val;
        }
        Ok(RiskScore {
            calculator: risk_calculator,
            score: total_score,
            interpretation: String::from_str(&env, "Risk assessment complete"),
        })
    }

    pub fn suggest_care_pathway(
        env: Env,
        _patient_id: Address,
        condition: String,
        _current_treatment: Vec<String>,
    ) -> Result<CarePathway, Error> {
        let mut steps = Vec::new(&env);
        steps.push_back(String::from_str(&env, "Initial Diagnosis"));
        steps.push_back(String::from_str(&env, "Standard Treatment"));
        steps.push_back(String::from_str(&env, "Follow-up"));

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

        if age > 50 {
            alerts.push_back(Symbol::new(&env, "Screening_A"));
        }
        if age > 20 {
            alerts.push_back(Symbol::new(&env, "Regular_Checkup"));
        }

        Ok(alerts)
    }
}

mod test;
