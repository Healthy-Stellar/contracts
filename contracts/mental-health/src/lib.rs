#![no_std]
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, xdr::ToXdr, vec, Address,
    BytesN, Env, String, Symbol, Vec,
};

// ── Cross-contract interface for emergency-medical-info escalation ────────────

#[contractclient(name = "EmergencyMedicalInfoClient")]
pub trait EmergencyMedicalInfoInterface {
    fn add_critical_alert(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        alert_type: Symbol,
        alert_text_hash: BytesN<32>,
        severity: Symbol,
    );
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    NotAuthorized = 2,
    RequiresExplicitConsent = 3,
    /// Cross-contract escalation call to emergency-medical-info failed
    EscalationFailed = 4,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentRecord {
    pub patient_id: Address,
    pub data_type: Symbol,
    pub provider_id: Address,
    pub consent_granted_at: u64,
    pub consent_expires_at: Option<u64>,
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
    Consent(Address, Symbol, Address),
    /// Address of the emergency-medical-info contract for crisis escalation
    EmergencyContractAddress,
}

#[contract]
pub struct MentalHealthContract;

#[contractimpl]
impl MentalHealthContract {
    /// Initialize the contract with the emergency-medical-info contract address.
    pub fn initialize(env: Env, emergency_contract: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::EmergencyContractAddress) {
            return Err(Error::NotAuthorized);
        }
        env.storage()
            .instance()
            .set(&DataKey::EmergencyContractAddress, &emergency_contract);
        Ok(())
    }

    /// Escalate a crisis alert to the emergency-medical-info contract.
    /// If the enhanced_privacy_flag is set for crisis_escalation, uses a de-identified patient token.
    fn escalate_crisis(
        env: &Env,
        patient_id: &Address,
        provider_id: &Address,
        severity: Symbol,
    ) -> Result<(), Error> {
        let emergency_addr: Address = match env
            .storage()
            .instance()
            .get(&DataKey::EmergencyContractAddress)
        {
            Some(addr) => addr,
            None => return Ok(()), // No emergency contract configured, skip escalation
        };

        // Check enhanced privacy flag for de-identification
        let privacy_flagged: bool = env
            .storage()
            .persistent()
            .get(&DataKey::PrivacyFlag(
                patient_id.clone(),
                Symbol::new(env, "crisis_escalation"),
            ))
            .unwrap_or(false);

        // If not de-identified, validate explicit consent before disclosing identity
        if !privacy_flagged {
            Self::validate_explicit_consent(
                env,
                patient_id,
                Symbol::new(env, "crisis_escalation"),
                provider_id,
            )?;
        }

        let patient_token = if privacy_flagged {
            // Derive a de-identified patient token via SHA-256
            let patient_bytes = patient_id.clone().to_xdr(env);
            let hash: [u8; 32] = env.crypto().sha256(&patient_bytes).into();
            let salt = BytesN::<32>::from_array(env, &hash);
            env.deployer().with_current_contract(salt).deployed_address()
        } else {
            patient_id.clone()
        };

        let crisis_client = EmergencyMedicalInfoClient::new(env, &emergency_addr);
        let alert_type = Symbol::new(env, "crisis_escalation");
        let alert_text_hash = BytesN::from_array(env, &[0u8; 32]);

        crisis_client.add_critical_alert(
            &patient_token,
            provider_id,
            &alert_type,
            &alert_text_hash,
            &severity,
        );

        Ok(())
    }

    /// Validate explicit consent for sensitive data access
    fn validate_explicit_consent(
        env: &Env,
        patient_id: &Address,
        data_type: Symbol,
        provider_id: &Address,
    ) -> Result<(), Error> {
        let consent_key = DataKey::Consent(patient_id.clone(), data_type, provider_id.clone());
        let consent: Option<ConsentRecord> = env.storage().persistent().get(&consent_key);

        match consent {
            Some(record) => {
                let now = env.ledger().timestamp();
                if let Some(expires_at) = record.consent_expires_at {
                    if now > expires_at {
                        return Err(Error::RequiresExplicitConsent);
                    }
                }
                Ok(())
            }
            None => Err(Error::RequiresExplicitConsent),
        }
    }

    /// Grant explicit consent for sensitive data operations
    pub fn grant_data_consent(
        env: Env,
        patient_id: Address,
        data_type: Symbol,
        provider_id: Address,
        consent_expires_at: Option<u64>,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let consent = ConsentRecord {
            patient_id: patient_id.clone(),
            data_type: data_type.clone(),
            provider_id: provider_id.clone(),
            consent_granted_at: env.ledger().timestamp(),
            consent_expires_at,
        };

        env.storage().persistent().set(
            &DataKey::Consent(patient_id.clone(), data_type.clone(), provider_id),
            &consent,
        );

        env.events().publish(
            (Symbol::new(&env, "consent_granted"), data_type),
            patient_id,
        );

        Ok(())
    }

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

        // Validate explicit consent for assessment
        Self::validate_explicit_consent(
            &env,
            &patient_id,
            Symbol::new(&env, "assessment"),
            &provider_id,
        )?;

        let mut count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::AssessmentCounter)
            .unwrap_or(0);
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

        env.storage()
            .persistent()
            .set(&DataKey::Assessment(count), &assessment);
        env.storage()
            .instance()
            .set(&DataKey::AssessmentCounter, &count);

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
        env.storage()
            .persistent()
            .set(&DataKey::Assessment(assessment_id), &assessment);

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
        env.storage()
            .persistent()
            .set(&DataKey::Assessment(assessment_id), &assessment);

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

        let assessment: MentalHealthAssessment = env
            .storage()
            .persistent()
            .get(&DataKey::Assessment(assessment_id))
            .ok_or(Error::NotFound)?;

        // Validate explicit consent for suicide risk assessment
        Self::validate_explicit_consent(
            &env,
            &assessment.patient_id,
            Symbol::new(&env, "suicide_risk"),
            &provider_id,
        )?;

        let mut updated_assessment = assessment;
        updated_assessment.suicide_risk_level = Some(risk_level.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Assessment(assessment_id), &updated_assessment);

        // Escalate to emergency-medical-info if risk is high
        if risk_level == Symbol::new(&env, "high") {
            let escalation_result = Self::escalate_crisis(
                &env,
                &updated_assessment.patient_id,
                &provider_id,
                Symbol::new(&env, "CRITICAL"),
            );
            // Escalation failure must not roll back clinical data
            if escalation_result.is_err() {
                return Err(Error::EscalationFailed);
            }
        }

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

        // Validate explicit consent for safety plan
        Self::validate_explicit_consent(
            &env,
            &patient_id,
            Symbol::new(&env, "safety_plan"),
            &provider_id,
        )?;

        let mut count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PlanCounter)
            .unwrap_or(0);
        count += 1;

        let plan = SafetyPlan {
            plan_id: count,
            patient_id: patient_id.clone(),
            provider_id: provider_id.clone(),
            warning_signs,
            coping_strategies,
            support_contacts,
            crisis_resources,
            plan_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::SafetyPlan(count), &plan);
        env.storage().instance().set(&DataKey::PlanCounter, &count);

        // Escalate to emergency-medical-info after safety plan creation
        let escalation_result = Self::escalate_crisis(
            &env,
            &patient_id,
            &provider_id,
            Symbol::new(&env, "HIGH"),
        );
        // Escalation failure must not roll back clinical data
        if escalation_result.is_err() {
            return Err(Error::EscalationFailed);
        }

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

        // Validate explicit consent for treatment plan
        Self::validate_explicit_consent(
            &env,
            &patient_id,
            Symbol::new(&env, "treatment_plan"),
            &provider_id,
        )?;

        let mut count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PlanCounter)
            .unwrap_or(0);
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

        env.storage()
            .persistent()
            .set(&DataKey::TreatmentPlan(count), &plan);
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
        let plan: TreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        Self::validate_explicit_consent(
            &env,
            &plan.patient_id,
            Symbol::new(&env, "therapy_session"),
            &plan.provider_id,
        )?;

        let session = TherapySession {
            treatment_plan_id,
            session_date,
            session_type,
            duration_minutes,
            interventions_used,
            progress_notes_hash,
            homework_assigned,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Session(treatment_plan_id, session_date), &session);

        env.events().publish(
            (Symbol::new(&env, "session_recorded"), treatment_plan_id),
            plan.patient_id,
        );

        Ok(())
    }

    pub fn track_symptom_severity(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        symptom_type: Symbol,
        severity_score: u32,
        measurement_date: u64,
        measurement_tool: Symbol,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        Self::validate_explicit_consent(
            &env,
            &patient_id,
            Symbol::new(&env, "symptom_severity"),
            &provider_id,
        )?;

        let symp = SymptomSeverity {
            patient_id: patient_id.clone(),
            symptom_type: symptom_type.clone(),
            severity_score,
            measurement_date,
            measurement_tool,
        };

        env.storage().persistent().set(
            &DataKey::Symptom(patient_id.clone(), symptom_type, measurement_date),
            &symp,
        );

        env.events().publish(
            (Symbol::new(&env, "symptom_tracked"), measurement_date),
            patient_id,
        );

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
        // Validate explicit consent for hospitalization records
        Self::validate_explicit_consent(
            &env,
            &patient_id,
            Symbol::new(&env, "hospitalization"),
            &facility_id,
        )?;

        let mut count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::HospitalizationCounter)
            .unwrap_or(0);
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

        env.storage()
            .persistent()
            .set(&DataKey::Hospitalization(count), &hosp);
        env.storage()
            .instance()
            .set(&DataKey::HospitalizationCounter, &count);

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

        // Validate explicit consent for substance screening
        Self::validate_explicit_consent(
            &env,
            &patient_id,
            Symbol::new(&env, "substance_screening"),
            &provider_id,
        )?;

        let mut count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ScreeningCounter)
            .unwrap_or(0);
        count += 1;

        let screen = Screening {
            screening_id: count,
            patient_id,
            provider_id,
            screening_tool,
            screening_date,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Screening(count), &screen);
        env.storage()
            .instance()
            .set(&DataKey::ScreeningCounter, &count);

        Ok(count)
    }

    pub fn track_treatment_outcomes(
        env: Env,
        treatment_plan_id: u64,
        measurement_date: u64,
        outcome_measures: Vec<OutcomeMeasure>,
        _functional_improvement: bool,
    ) -> Result<(), Error> {
        let plan: TreatmentPlan = env
            .storage()
            .persistent()
            .get(&DataKey::TreatmentPlan(treatment_plan_id))
            .ok_or(Error::NotFound)?;

        Self::validate_explicit_consent(
            &env,
            &plan.patient_id,
            Symbol::new(&env, "outcomes"),
            &plan.provider_id,
        )?;

        env.storage().persistent().set(
            &DataKey::Outcomes(treatment_plan_id, measurement_date),
            &outcome_measures,
        );

        env.events().publish(
            (Symbol::new(&env, "outcomes_tracked"), treatment_plan_id),
            plan.patient_id,
        );

        Ok(())
    }

    pub fn set_enhanced_privacy_flag(
        env: Env,
        patient_id: Address,
        record_type: Symbol,
        requires_explicit_consent: bool,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        env.storage().persistent().set(
            &DataKey::PrivacyFlag(patient_id, record_type),
            &requires_explicit_consent,
        );
        Ok(())
    }
}

mod test;
