#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, vec, Address, BytesN, Env, String, Symbol, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    NotAuthorized = 2,
    RequiresExplicitConsent = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MentalHealthAssessment {
    pub assessment_id: u64,
    pub patient_id: Address,
    pub assessment_date: u64,
    pub assessment_type: Symbol,
    pub phq9_score: Option<u32>,
    pub gad7_score: Option<u32>,
    pub suicide_risk_level: Option<Symbol>,
    pub diagnosis_codes: Vec<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreatmentGoal {
    pub goal_description: String,
    pub target_date: u64,
    pub measurement_method: String,
    pub status: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutcomeMeasure {
    pub measure_name: String,
    pub baseline_score: u32,
    pub current_score: u32,
    pub improvement_percentage: u32, // Note: using u32 instead of f32 for Soroban contract compatibility
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreatmentPlan {
    pub plan_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub diagnoses: Vec<String>,
    pub treatment_goals: Vec<TreatmentGoal>,
    pub interventions: Vec<String>,
    pub frequency: String,
    pub review_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TherapySession {
    pub treatment_plan_id: u64,
    pub session_date: u64,
    pub session_type: Symbol,
    pub duration_minutes: u32,
    pub interventions_used: Vec<String>,
    pub progress_notes_hash: BytesN<32>,
    pub homework_assigned: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymptomSeverity {
    pub patient_id: Address,
    pub symptom_type: Symbol,
    pub severity_score: u32,
    pub measurement_date: u64,
    pub measurement_tool: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hospitalization {
    pub hospital_id: u64,
    pub patient_id: Address,
    pub admission_date: u64,
    pub admission_reason: String,
    pub legal_status: Symbol,
    pub facility_id: Address,
    pub discharge_date: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafetyPlan {
    pub plan_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub warning_signs: Vec<String>,
    pub coping_strategies: Vec<String>,
    pub support_contacts: Vec<String>,
    pub crisis_resources: Vec<String>,
    pub plan_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Screening {
    pub screening_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub screening_tool: Symbol,
    pub screening_date: u64,
}

#[contracttype]
pub enum DataKey {
    AssessmentCounter,
    PlanCounter,
    HospitalizationCounter,
    ScreeningCounter,
    Assessment(u64),
    TreatmentPlan(u64),
    Hospitalization(u64),
    SafetyPlan(u64),
    Screening(u64),
    PrivacyFlag(Address, Symbol),
    Session(u64, u64),
    Symptom(Address, Symbol, u64),
    Outcomes(u64, u64),
}

#[contract]
pub struct MentalHealthContract;

#[contractimpl]
impl MentalHealthContract {
    pub fn conduct_mental_health_assessment(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        assessment_date: u64,
        assessment_type: Symbol,
        _presenting_concerns: Vec<String>,
        _assessment_tools_used: Vec<Symbol>,
        _assessment_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let mut count: u64 = env.storage().instance().get(&DataKey::AssessmentCounter).unwrap_or(0);
        count += 1;

        let assessment = MentalHealthAssessment {
            assessment_id: count,
            patient_id,
            assessment_date,
            assessment_type,
            phq9_score: None,
            gad7_score: None,
            suicide_risk_level: None,
            diagnosis_codes: vec![&env],
        };

        env.storage().persistent().set(&DataKey::Assessment(count), &assessment);
        env.storage().instance().set(&DataKey::AssessmentCounter, &count);

        Ok(count)
    }

    pub fn record_phq9_score(
        env: Env,
        assessment_id: u64,
        total_score: u32,
        _item_scores: Vec<u32>,
        _assessment_date: u64,
    ) -> Result<(), Error> {
        let mut assessment: MentalHealthAssessment = env
            .storage()
            .persistent()
            .get(&DataKey::Assessment(assessment_id))
            .ok_or(Error::NotFound)?;

        assessment.phq9_score = Some(total_score);
        env.storage().persistent().set(&DataKey::Assessment(assessment_id), &assessment);

        Ok(())
    }

    pub fn record_gad7_score(
        env: Env,
        assessment_id: u64,
        total_score: u32,
        _item_scores: Vec<u32>,
        _assessment_date: u64,
    ) -> Result<(), Error> {
        let mut assessment: MentalHealthAssessment = env
            .storage()
            .persistent()
            .get(&DataKey::Assessment(assessment_id))
            .ok_or(Error::NotFound)?;

        assessment.gad7_score = Some(total_score);
        env.storage().persistent().set(&DataKey::Assessment(assessment_id), &assessment);

        Ok(())
    }

    pub fn assess_suicide_risk(
        env: Env,
        assessment_id: u64,
        provider_id: Address,
        risk_level: Symbol,
        _risk_factors: Vec<String>,
        _protective_factors: Vec<String>,
        _safety_plan_created: bool,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut assessment: MentalHealthAssessment = env
            .storage()
            .persistent()
            .get(&DataKey::Assessment(assessment_id))
            .ok_or(Error::NotFound)?;

        assessment.suicide_risk_level = Some(risk_level);
        env.storage().persistent().set(&DataKey::Assessment(assessment_id), &assessment);

        Ok(())
    }

    pub fn create_safety_plan(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        warning_signs: Vec<String>,
        coping_strategies: Vec<String>,
        support_contacts: Vec<String>,
        crisis_resources: Vec<String>,
        plan_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let mut count: u64 = env.storage().instance().get(&DataKey::PlanCounter).unwrap_or(0);
        count += 1;

        let plan = SafetyPlan {
            plan_id: count,
            patient_id,
            provider_id,
            warning_signs,
            coping_strategies,
            support_contacts,
            crisis_resources,
            plan_hash,
        };

        env.storage().persistent().set(&DataKey::SafetyPlan(count), &plan);
        env.storage().instance().set(&DataKey::PlanCounter, &count);

        Ok(count)
    }

    pub fn create_treatment_plan(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        diagnoses: Vec<String>,
        treatment_goals: Vec<TreatmentGoal>,
        interventions: Vec<String>,
        frequency: String,
        review_date: u64,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let mut count: u64 = env.storage().instance().get(&DataKey::PlanCounter).unwrap_or(0);
        count += 1;

        let plan = TreatmentPlan {
            plan_id: count,
            patient_id,
            provider_id,
            diagnoses,
            treatment_goals,
            interventions,
            frequency,
            review_date,
        };

        env.storage().persistent().set(&DataKey::TreatmentPlan(count), &plan);
        env.storage().instance().set(&DataKey::PlanCounter, &count);

        Ok(count)
    }

    pub fn record_therapy_session(
        env: Env,
        treatment_plan_id: u64,
        session_date: u64,
        session_type: Symbol,
        duration_minutes: u32,
        interventions_used: Vec<String>,
        progress_notes_hash: BytesN<32>,
        homework_assigned: Option<String>,
    ) -> Result<(), Error> {
        let session = TherapySession {
            treatment_plan_id,
            session_date,
            session_type,
            duration_minutes,
            interventions_used,
            progress_notes_hash,
            homework_assigned,
        };

        env.storage().persistent().set(&DataKey::Session(treatment_plan_id, session_date), &session);

        Ok(())
    }

    pub fn track_symptom_severity(
        env: Env,
        patient_id: Address,
        symptom_type: Symbol,
        severity_score: u32,
        measurement_date: u64,
        measurement_tool: Symbol,
    ) -> Result<(), Error> {
        let symp = SymptomSeverity {
            patient_id: patient_id.clone(),
            symptom_type: symptom_type.clone(),
            severity_score,
            measurement_date,
            measurement_tool,
        };

        env.storage().persistent().set(&DataKey::Symptom(patient_id, symptom_type, measurement_date), &symp);

        Ok(())
    }

    pub fn document_hospitalization(
        env: Env,
        patient_id: Address,
        admission_date: u64,
        admission_reason: String,
        legal_status: Symbol,
        facility_id: Address,
        discharge_date: Option<u64>,
    ) -> Result<u64, Error> {
        let mut count: u64 = env.storage().instance().get(&DataKey::HospitalizationCounter).unwrap_or(0);
        count += 1;

        let hosp = Hospitalization {
            hospital_id: count,
            patient_id,
            admission_date,
            admission_reason,
            legal_status,
            facility_id,
            discharge_date,
        };

        env.storage().persistent().set(&DataKey::Hospitalization(count), &hosp);
        env.storage().instance().set(&DataKey::HospitalizationCounter, &count);

        Ok(count)
    }

    pub fn request_substance_screening(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        screening_tool: Symbol,
        screening_date: u64,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let is_private = env
            .storage()
            .persistent()
            .get(&DataKey::PrivacyFlag(patient_id.clone(), Symbol::new(&env, "substance_abuse")))
            .unwrap_or(false);

        if is_private {
            return Err(Error::RequiresExplicitConsent);
        }

        let mut count: u64 = env.storage().instance().get(&DataKey::ScreeningCounter).unwrap_or(0);
        count += 1;

        let screen = Screening {
            screening_id: count,
            patient_id,
            provider_id,
            screening_tool,
            screening_date,
        };

        env.storage().persistent().set(&DataKey::Screening(count), &screen);
        env.storage().instance().set(&DataKey::ScreeningCounter, &count);

        Ok(count)
    }

    pub fn track_treatment_outcomes(
        env: Env,
        treatment_plan_id: u64,
        measurement_date: u64,
        outcome_measures: Vec<OutcomeMeasure>,
        _functional_improvement: bool,
    ) -> Result<(), Error> {
        env.storage().persistent().set(&DataKey::Outcomes(treatment_plan_id, measurement_date), &outcome_measures);
        Ok(())
    }

    pub fn set_enhanced_privacy_flag(
        env: Env,
        patient_id: Address,
        record_type: Symbol,
        requires_explicit_consent: bool,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        env.storage().persistent().set(&DataKey::PrivacyFlag(patient_id, record_type), &requires_explicit_consent);
        Ok(())
    }
}

mod test;
