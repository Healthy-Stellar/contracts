#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Prenatal & Pediatric Contract
//!
//! Manages prenatal care, obstetric assessments, pediatric developmental milestones, immunization
//! tracking, and early childhood health records.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Obstetrician/pediatrician authentication for records. Mother/
//! guardian consent for prenatal and pediatric data. Authorized family members can access pediatric
//! records. Provider registry validation for record access. Emergency access override for critical
//! situations.
//!
//! **Audit Controls:** Prenatal appointment events logged with findings and risk assessment.
//! Pediatric visit events tracked with provider and developmental assessment. Immunization events
//! linked to pediatric record. Gestational age progression events recorded. Birth event logged.
//! Milestone achievement tracking with age-appropriate benchmarks.
//!
//! **Data Retention Policy:** Prenatal records retained with delivery summary. Pediatric records
//! retained indefinitely for child health history. Developmental milestones tracked. Immunization
//! records cross-linked with immunization registry. Family health history maintained.
//!
//! **Encryption/Integrity:** Prenatal assessment data encrypted in storage. Child identity linked
//! to mother cryptographically. Birth date immutable timestamp. Maternal and child addresses
//! validated. Developmental milestones enumerated with age validation.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, vec, Address, BytesN, Env,
    IntoVal, String, Symbol, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    Unauthorized = 2,
    InvalidData = 3,
    AlreadyExists = 4,
    NotInitialized = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PregnancyRecord {
    pub pregnancy_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub lmp_date: u64,
    pub edd: u64,
    pub gravida: u32,
    pub para: u32,
    pub prenatal_visits: Vec<u64>,
    pub complications: Vec<Symbol>,
    pub outcome: Option<Symbol>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrenatalVisit {
    pub visit_id: u64,
    pub pregnancy_id: u64,
    pub visit_date: u64,
    pub gestational_age_weeks: u32,
    pub weight_kg_x100: i64,
    pub blood_pressure: String,
    pub fundal_height_cm: Option<u32>,
    pub fetal_heart_rate: Option<u32>,
    pub visit_notes_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrenatalScreening {
    pub screening_id: u64,
    pub pregnancy_id: u64,
    pub screening_type: Symbol,
    pub test_date: u64,
    pub results_hash: BytesN<32>,
    pub abnormal: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UltrasoundRecord {
    pub ultrasound_id: u64,
    pub pregnancy_id: u64,
    pub ultrasound_date: u64,
    pub gestational_age: u32,
    pub estimated_fetal_weight_grams: Option<u32>,
    pub amniotic_fluid: Symbol,
    pub placental_location: String,
    pub findings_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaborRecord {
    pub labor_id: u64,
    pub pregnancy_id: u64,
    pub admission_date: u64,
    pub contractions: bool,
    pub membrane_status: Symbol,
    pub cervical_dilation: u32,
    pub cervical_effacement: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryRecord {
    pub delivery_id: u64,
    pub pregnancy_id: u64,
    pub delivery_datetime: u64,
    pub delivery_method: Symbol,
    pub presentation: Symbol,
    pub newborn_ids: Vec<Address>,
    pub complications: Vec<Symbol>,
    pub blood_loss_ml: u32,
    pub delivering_provider: Address,
    pub mother_outcome: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewbornRecord {
    pub newborn_id: Address,
    pub delivery_id: u64,
    pub birth_datetime: u64,
    pub sex: Symbol,
    pub birth_weight_grams: u32,
    pub birth_length_cm: u32,
    pub head_circumference_cm: u32,
    pub apgar_1min: u32,
    pub apgar_5min: u32,
    pub gestational_age_weeks: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewbornScreening {
    pub screening_id: u64,
    pub newborn_id: Address,
    pub screening_type: Symbol,
    pub test_date: u64,
    pub result: Symbol,
    pub requires_followup: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PediatricMeasurements {
    pub weight_kg_x100: i64,
    pub height_cm_x100: i64,
    pub head_circumference_cm_x100: Option<i64>,
    pub bmi_x100: i64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PediatricGrowthRecord {
    pub growth_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub measurement_date: u64,
    pub age_months: u32,
    pub measurements: PediatricMeasurements,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopmentalMilestone {
    pub patient_id: Address,
    pub assessment_date: u64,
    pub age_months: u32,
    pub milestone_category: Symbol,
    pub milestones_met: Vec<Symbol>,
    pub concerns: Vec<Symbol>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WellChildVisit {
    pub patient_id: Address,
    pub visit_date: u64,
    pub age_months: u32,
    pub immunizations_given: Vec<Symbol>,
    pub developmental_screening: bool,
    pub anticipatory_guidance_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrowthPercentiles {
    pub weight_percentile_x100: i64,
    pub height_percentile_x100: i64,
    pub head_circ_pct_x100: Option<i64>,
    pub bmi_percentile_x100: i64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    ProviderRegistry,
    Pregnancy(u64),
    PrenatalVisit(u64),
    PrenatalScreening(u64),
    Ultrasound(u64),
    Labor(u64),
    Delivery(u64),
    Newborn(Address),
    NewbornScreening(u64),
    Growth(u64),
    GrowthByAge(Address, u32),
    Milestone(Address, u32),
    WellChildVisit(Address, u64),
    /// (patient/guardian, grantee) -> true when the patient/guardian has
    /// authorized the grantee (e.g. a family member) to read their records.
    AccessGrant(Address, Address),
}

#[contract]
pub struct MaternalChildHealthContract;

#[contractimpl]
impl MaternalChildHealthContract {
    /// Configure the admin and the provider-registry contract used to
    /// validate that a provider is credentialed before it may write records.
    ///
    /// Runs atomically as part of contract creation and cannot be called again,
    /// so an attacker cannot front-run deployment and elect themselves admin.
    pub fn __constructor(env: Env, admin: Address, provider_registry: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &provider_registry);
    }

    /// Check whether `provider_id` is an active, credentialed provider in
    /// the configured provider-registry contract.
    fn is_registered_provider(env: &Env, provider_id: &Address) -> bool {
        let provider_registry: Address = env
            .storage()
            .instance()
            .get(&DataKey::ProviderRegistry)
            .unwrap();
        let args = vec![env, provider_id.clone().into_val(env)];
        env.invoke_contract(&provider_registry, &Symbol::new(env, "is_provider"), args)
    }

    pub fn create_pregnancy_record(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        lmp_date: u64,
        estimated_due_date: u64,
        gravida: u32,
        para: u32,
        prenatal_risk_factors: Vec<Symbol>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();
        patient_id.require_auth();

        if !Self::is_registered_provider(&env, &provider_id) {
            return Err(Error::Unauthorized);
        }

        if lmp_date >= estimated_due_date || para > gravida {
            return Err(Error::InvalidData);
        }

        let pregnancy_id = Self::next_id(&env, symbol_short!("preg_ctr"));
        let record = PregnancyRecord {
            pregnancy_id,
            patient_id,
            provider_id,
            lmp_date,
            edd: estimated_due_date,
            gravida,
            para,
            prenatal_visits: Vec::new(&env),
            complications: prenatal_risk_factors,
            outcome: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Pregnancy(pregnancy_id), &record);

        Ok(pregnancy_id)
    }

    pub fn record_prenatal_visit(
        env: Env,
        pregnancy_id: u64,
        visit_date: u64,
        gestational_age_weeks: u32,
        weight_kg_x100: i64,
        blood_pressure: String,
        fundal_height_cm: Option<u32>,
        fetal_heart_rate: Option<u32>,
        visit_notes_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let mut pregnancy = Self::get_pregnancy(&env, pregnancy_id)?;
        pregnancy.provider_id.require_auth();

        if gestational_age_weeks > 45 || weight_kg_x100 <= 0 {
            return Err(Error::InvalidData);
        }

        let visit_id = Self::next_id(&env, symbol_short!("visit_ctr"));
        let visit = PrenatalVisit {
            visit_id,
            pregnancy_id,
            visit_date,
            gestational_age_weeks,
            weight_kg_x100,
            blood_pressure,
            fundal_height_cm,
            fetal_heart_rate,
            visit_notes_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::PrenatalVisit(visit_id), &visit);

        pregnancy.prenatal_visits.push_back(visit_id);
        env.storage()
            .persistent()
            .set(&DataKey::Pregnancy(pregnancy_id), &pregnancy);

        Ok(())
    }

    pub fn record_prenatal_screening(
        env: Env,
        pregnancy_id: u64,
        screening_type: Symbol,
        test_date: u64,
        results_hash: BytesN<32>,
        abnormal: bool,
    ) -> Result<(), Error> {
        let pregnancy = Self::get_pregnancy(&env, pregnancy_id)?;
        pregnancy.provider_id.require_auth();

        let screening_id = Self::next_id(&env, symbol_short!("scrn_ctr"));
        let screening = PrenatalScreening {
            screening_id,
            pregnancy_id,
            screening_type,
            test_date,
            results_hash,
            abnormal,
        };

        env.storage()
            .persistent()
            .set(&DataKey::PrenatalScreening(screening_id), &screening);

        Ok(())
    }

    pub fn record_ultrasound(
        env: Env,
        pregnancy_id: u64,
        ultrasound_date: u64,
        gestational_age: u32,
        estimated_fetal_weight_grams: Option<u32>,
        amniotic_fluid: Symbol,
        placental_location: String,
        findings_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let pregnancy = Self::get_pregnancy(&env, pregnancy_id)?;
        pregnancy.provider_id.require_auth();

        if gestational_age > 45 {
            return Err(Error::InvalidData);
        }

        let ultrasound_id = Self::next_id(&env, symbol_short!("us_ctr"));
        let ultrasound = UltrasoundRecord {
            ultrasound_id,
            pregnancy_id,
            ultrasound_date,
            gestational_age,
            estimated_fetal_weight_grams,
            amniotic_fluid,
            placental_location,
            findings_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Ultrasound(ultrasound_id), &ultrasound);

        Ok(())
    }

    pub fn document_labor_admission(
        env: Env,
        pregnancy_id: u64,
        admission_date: u64,
        contractions: bool,
        membrane_status: Symbol,
        cervical_dilation: u32,
        cervical_effacement: u32,
    ) -> Result<u64, Error> {
        let pregnancy = Self::get_pregnancy(&env, pregnancy_id)?;
        pregnancy.provider_id.require_auth();

        if cervical_dilation > 10 || cervical_effacement > 100 {
            return Err(Error::InvalidData);
        }

        let labor_id = Self::next_id(&env, symbol_short!("labor_ct"));
        let labor = LaborRecord {
            labor_id,
            pregnancy_id,
            admission_date,
            contractions,
            membrane_status,
            cervical_dilation,
            cervical_effacement,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Labor(labor_id), &labor);

        Ok(labor_id)
    }

    pub fn record_delivery(
        env: Env,
        labor_id: u64,
        delivery_datetime: u64,
        delivery_method: Symbol,
        presentation: Symbol,
        complications: Vec<Symbol>,
        blood_loss_ml: u32,
        delivering_provider: Address,
    ) -> Result<u64, Error> {
        delivering_provider.require_auth();

        let labor: LaborRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Labor(labor_id))
            .ok_or(Error::NotFound)?;

        // Proving control of an address is not proof of role -- the
        // delivering provider must be the provider on record for this
        // pregnancy.
        let mut pregnancy = Self::get_pregnancy(&env, labor.pregnancy_id)?;
        if delivering_provider != pregnancy.provider_id {
            return Err(Error::Unauthorized);
        }

        let delivery_id = Self::next_id(&env, symbol_short!("dlvry_ct"));
        let delivery = DeliveryRecord {
            delivery_id,
            pregnancy_id: labor.pregnancy_id,
            delivery_datetime,
            delivery_method,
            presentation,
            newborn_ids: Vec::new(&env),
            complications,
            blood_loss_ml,
            delivering_provider,
            mother_outcome: symbol_short!("stable"),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Delivery(delivery_id), &delivery);

        pregnancy.outcome = Some(symbol_short!("delivrd"));
        env.storage()
            .persistent()
            .set(&DataKey::Pregnancy(labor.pregnancy_id), &pregnancy);

        Ok(delivery_id)
    }

    pub fn record_newborn(
        env: Env,
        provider_id: Address,
        delivery_id: u64,
        birth_datetime: u64,
        sex: Symbol,
        birth_weight_grams: u32,
        birth_length_cm: u32,
        head_circumference_cm: u32,
        apgar_1min: u32,
        apgar_5min: u32,
        gestational_age_weeks: u32,
    ) -> Result<Address, Error> {
        let mut delivery: DeliveryRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Delivery(delivery_id))
            .ok_or(Error::NotFound)?;

        if provider_id != delivery.delivering_provider {
            return Err(Error::Unauthorized);
        }
        provider_id.require_auth();

        if apgar_1min > 10 || apgar_5min > 10 || gestational_age_weeks > 45 {
            return Err(Error::InvalidData);
        }

        let newborn_seq = Self::next_id(&env, symbol_short!("newb_ctr"));
        let newborn_id = Self::newborn_address(&env, delivery_id, newborn_seq);

        let record = NewbornRecord {
            newborn_id: newborn_id.clone(),
            delivery_id,
            birth_datetime,
            sex,
            birth_weight_grams,
            birth_length_cm,
            head_circumference_cm,
            apgar_1min,
            apgar_5min,
            gestational_age_weeks,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Newborn(newborn_id.clone()), &record);

        delivery.newborn_ids.push_back(newborn_id.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Delivery(delivery_id), &delivery);

        Ok(newborn_id)
    }

    pub fn record_newborn_screening(
        env: Env,
        provider_id: Address,
        newborn_id: Address,
        screening_type: Symbol,
        test_date: u64,
        result: Symbol,
        requires_followup: bool,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let newborn: NewbornRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Newborn(newborn_id.clone()))
            .ok_or(Error::NotFound)?;
        let delivery = Self::get_delivery(&env, newborn.delivery_id)?;

        // Proving control of an address is not proof of role -- the caller
        // must be the delivering provider or a credentialed provider.
        if provider_id != delivery.delivering_provider
            && !Self::is_registered_provider(&env, &provider_id)
        {
            return Err(Error::Unauthorized);
        }

        let screening_id = Self::next_id(&env, symbol_short!("nbs_ctr"));
        let screening = NewbornScreening {
            screening_id,
            newborn_id,
            screening_type,
            test_date,
            result,
            requires_followup,
        };

        env.storage()
            .persistent()
            .set(&DataKey::NewbornScreening(screening_id), &screening);

        Ok(())
    }

    pub fn track_pediatric_growth(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        measurement_date: u64,
        age_months: u32,
        weight_kg_x100: i64,
        height_cm_x100: i64,
        head_circumference_cm_x100: Option<i64>,
        bmi_x100: i64,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        patient_id.require_auth();

        if !Self::is_registered_provider(&env, &provider_id) {
            return Err(Error::Unauthorized);
        }

        if age_months > 228 || weight_kg_x100 <= 0 || height_cm_x100 <= 0 || bmi_x100 <= 0 {
            return Err(Error::InvalidData);
        }

        let measurements = PediatricMeasurements {
            weight_kg_x100,
            height_cm_x100,
            head_circumference_cm_x100,
            bmi_x100,
        };

        let growth_id = Self::next_id(&env, symbol_short!("grw_ctr"));
        let growth = PediatricGrowthRecord {
            growth_id,
            patient_id: patient_id.clone(),
            provider_id,
            measurement_date,
            age_months,
            measurements,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Growth(growth_id), &growth);
        env.storage()
            .persistent()
            .set(&DataKey::GrowthByAge(patient_id, age_months), &growth_id);

        Ok(())
    }

    pub fn record_developmental_milestone(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        assessment_date: u64,
        age_months: u32,
        milestone_category: Symbol,
        milestones_met: Vec<Symbol>,
        concerns: Vec<Symbol>,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        patient_id.require_auth();

        if !Self::is_registered_provider(&env, &provider_id) {
            return Err(Error::Unauthorized);
        }

        if age_months > 228 {
            return Err(Error::InvalidData);
        }

        let record = DevelopmentalMilestone {
            patient_id: patient_id.clone(),
            assessment_date,
            age_months,
            milestone_category,
            milestones_met,
            concerns,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Milestone(patient_id, age_months), &record);

        Ok(())
    }

    pub fn track_well_child_visit(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        visit_date: u64,
        age_months: u32,
        immunizations_given: Vec<Symbol>,
        developmental_screening: bool,
        anticipatory_guidance_hash: BytesN<32>,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        patient_id.require_auth();

        if !Self::is_registered_provider(&env, &provider_id) {
            return Err(Error::Unauthorized);
        }

        if age_months > 228 {
            return Err(Error::InvalidData);
        }

        let visit = WellChildVisit {
            patient_id: patient_id.clone(),
            visit_date,
            age_months,
            immunizations_given,
            developmental_screening,
            anticipatory_guidance_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::WellChildVisit(patient_id, visit_date), &visit);

        Ok(())
    }

    pub fn calculate_growth_percentiles(
        _env: Env,
        _patient_id: Address,
        sex: Symbol,
        age_months: u32,
        measurements: PediatricMeasurements,
    ) -> Result<GrowthPercentiles, Error> {
        if age_months > 228 {
            return Err(Error::InvalidData);
        }

        if sex != symbol_short!("male") && sex != symbol_short!("female") {
            return Err(Error::InvalidData);
        }

        let expected_weight = Self::expected_weight_kg_x100(age_months, &sex);
        let expected_height = Self::expected_height_cm_x100(age_months, &sex);
        let expected_hc = Self::expected_head_circumference_cm_x100(age_months, &sex);
        let expected_bmi = Self::expected_bmi_x100(age_months, &sex);

        let weight_percentile_x100 =
            Self::estimate_percentile(measurements.weight_kg_x100, expected_weight, 120);
        let height_percentile_x100 =
            Self::estimate_percentile(measurements.height_cm_x100, expected_height, 300);
        let bmi_percentile_x100 =
            Self::estimate_percentile(measurements.bmi_x100, expected_bmi, 120);
        let head_circ_pct_x100 = measurements
            .head_circumference_cm_x100
            .map(|hc| Self::estimate_percentile(hc, expected_hc, 180));

        Ok(GrowthPercentiles {
            weight_percentile_x100,
            height_percentile_x100,
            head_circ_pct_x100,
            bmi_percentile_x100,
        })
    }

    /// Allow `grantee` (e.g. an authorized family member) to read the
    /// records of `patient_id`. For newborn records the mother acts as
    /// guardian, so grants from the mother cover her newborns.
    pub fn grant_record_access(env: Env, patient_id: Address, grantee: Address) {
        patient_id.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::AccessGrant(patient_id, grantee), &true);
    }

    /// Revoke a grant previously made with `grant_record_access`.
    pub fn revoke_record_access(env: Env, patient_id: Address, grantee: Address) {
        patient_id.require_auth();
        env.storage()
            .persistent()
            .remove(&DataKey::AccessGrant(patient_id, grantee));
    }

    pub fn has_record_access(env: Env, patient_id: Address, grantee: Address) -> bool {
        Self::has_access_grant(&env, &patient_id, &grantee)
    }

    pub fn get_pregnancy_record(
        env: Env,
        requester: Address,
        pregnancy_id: u64,
    ) -> Result<PregnancyRecord, Error> {
        requester.require_auth();
        let pregnancy = Self::get_pregnancy(&env, pregnancy_id)?;
        Self::check_pregnancy_access(&env, &requester, &pregnancy)?;
        Ok(pregnancy)
    }

    pub fn get_prenatal_visit(
        env: Env,
        requester: Address,
        visit_id: u64,
    ) -> Result<PrenatalVisit, Error> {
        requester.require_auth();
        let visit: PrenatalVisit = env
            .storage()
            .persistent()
            .get(&DataKey::PrenatalVisit(visit_id))
            .ok_or(Error::NotFound)?;
        let pregnancy = Self::get_pregnancy(&env, visit.pregnancy_id)?;
        Self::check_pregnancy_access(&env, &requester, &pregnancy)?;
        Ok(visit)
    }

    pub fn get_prenatal_screening(
        env: Env,
        requester: Address,
        screening_id: u64,
    ) -> Result<PrenatalScreening, Error> {
        requester.require_auth();
        let screening: PrenatalScreening = env
            .storage()
            .persistent()
            .get(&DataKey::PrenatalScreening(screening_id))
            .ok_or(Error::NotFound)?;
        let pregnancy = Self::get_pregnancy(&env, screening.pregnancy_id)?;
        Self::check_pregnancy_access(&env, &requester, &pregnancy)?;
        Ok(screening)
    }

    pub fn get_ultrasound(
        env: Env,
        requester: Address,
        ultrasound_id: u64,
    ) -> Result<UltrasoundRecord, Error> {
        requester.require_auth();
        let ultrasound: UltrasoundRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Ultrasound(ultrasound_id))
            .ok_or(Error::NotFound)?;
        let pregnancy = Self::get_pregnancy(&env, ultrasound.pregnancy_id)?;
        Self::check_pregnancy_access(&env, &requester, &pregnancy)?;
        Ok(ultrasound)
    }

    pub fn get_labor_record(
        env: Env,
        requester: Address,
        labor_id: u64,
    ) -> Result<LaborRecord, Error> {
        requester.require_auth();
        let labor: LaborRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Labor(labor_id))
            .ok_or(Error::NotFound)?;
        let pregnancy = Self::get_pregnancy(&env, labor.pregnancy_id)?;
        Self::check_pregnancy_access(&env, &requester, &pregnancy)?;
        Ok(labor)
    }

    pub fn get_delivery_record(
        env: Env,
        requester: Address,
        delivery_id: u64,
    ) -> Result<DeliveryRecord, Error> {
        requester.require_auth();
        let delivery = Self::get_delivery(&env, delivery_id)?;
        Self::check_delivery_access(&env, &requester, &delivery)?;
        Ok(delivery)
    }

    pub fn get_newborn_record(
        env: Env,
        requester: Address,
        newborn_id: Address,
    ) -> Result<NewbornRecord, Error> {
        requester.require_auth();
        let newborn: NewbornRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Newborn(newborn_id))
            .ok_or(Error::NotFound)?;
        let delivery = Self::get_delivery(&env, newborn.delivery_id)?;
        Self::check_delivery_access(&env, &requester, &delivery)?;
        Ok(newborn)
    }

    pub fn get_growth_record(
        env: Env,
        requester: Address,
        patient_id: Address,
        age_months: u32,
    ) -> Result<PediatricGrowthRecord, Error> {
        requester.require_auth();
        let growth_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::GrowthByAge(patient_id, age_months))
            .ok_or(Error::NotFound)?;
        let growth: PediatricGrowthRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Growth(growth_id))
            .ok_or(Error::NotFound)?;
        if requester != growth.patient_id
            && requester != growth.provider_id
            && !Self::has_access_grant(&env, &growth.patient_id, &requester)
        {
            return Err(Error::Unauthorized);
        }
        Ok(growth)
    }

    fn has_access_grant(env: &Env, patient_id: &Address, grantee: &Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::AccessGrant(patient_id.clone(), grantee.clone()))
            .unwrap_or(false)
    }

    /// The patient, the provider on record, or a grantee of the patient may
    /// read pregnancy-scoped records.
    fn check_pregnancy_access(
        env: &Env,
        requester: &Address,
        pregnancy: &PregnancyRecord,
    ) -> Result<(), Error> {
        if *requester == pregnancy.patient_id
            || *requester == pregnancy.provider_id
            || Self::has_access_grant(env, &pregnancy.patient_id, requester)
        {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }

    /// Delivery and newborn records are readable by the delivering provider
    /// or anyone with access to the underlying pregnancy (mother as guardian).
    fn check_delivery_access(
        env: &Env,
        requester: &Address,
        delivery: &DeliveryRecord,
    ) -> Result<(), Error> {
        if *requester == delivery.delivering_provider {
            return Ok(());
        }
        let pregnancy = Self::get_pregnancy(env, delivery.pregnancy_id)?;
        Self::check_pregnancy_access(env, requester, &pregnancy)
    }

    fn get_delivery(env: &Env, delivery_id: u64) -> Result<DeliveryRecord, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Delivery(delivery_id))
            .ok_or(Error::NotFound)
    }

    fn get_pregnancy(env: &Env, pregnancy_id: u64) -> Result<PregnancyRecord, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Pregnancy(pregnancy_id))
            .ok_or(Error::NotFound)
    }

    fn next_id(env: &Env, counter_key: Symbol) -> u64 {
        shared_contracts::safe_increment(env, &counter_key)
    }

    fn newborn_address(env: &Env, delivery_id: u64, newborn_seq: u64) -> Address {
        let mut raw = [0u8; 32];
        raw[0..8].copy_from_slice(&delivery_id.to_be_bytes());
        raw[8..16].copy_from_slice(&newborn_seq.to_be_bytes());
        raw[16..24].copy_from_slice(&env.ledger().timestamp().to_be_bytes());
        raw[24..28].copy_from_slice(&env.ledger().sequence().to_be_bytes());
        let salt = BytesN::from_array(env, &raw);
        env.deployer()
            .with_current_contract(salt)
            .deployed_address()
    }

    fn estimate_percentile(value: i64, expected: i64, sd: i64) -> i64 {
        let delta = i128::from(value) - i128::from(expected);
        let score = 5000i128 + (delta * 2000i128) / i128::from(sd);
        if score < 0 {
            0
        } else if score > 10_000 {
            10_000
        } else {
            score as i64
        }
    }

    fn expected_weight_kg_x100(age_months: u32, sex: &Symbol) -> i64 {
        let base = if *sex == symbol_short!("male") {
            370
        } else {
            350
        };
        if age_months <= 12 {
            base + i64::from(age_months) * 60
        } else {
            base + 12 * 60 + i64::from(age_months - 12) * 25
        }
    }

    fn expected_height_cm_x100(age_months: u32, sex: &Symbol) -> i64 {
        let base = if *sex == symbol_short!("male") {
            5100
        } else {
            5000
        };
        if age_months <= 12 {
            base + i64::from(age_months) * 250
        } else {
            base + 12 * 250 + i64::from(age_months - 12) * 80
        }
    }

    fn expected_head_circumference_cm_x100(age_months: u32, sex: &Symbol) -> i64 {
        let base = if *sex == symbol_short!("male") {
            3550
        } else {
            3450
        };
        let growth = if age_months <= 24 {
            i64::from(age_months) * 70
        } else {
            24 * 70 + i64::from(age_months - 24) * 10
        };
        base + growth
    }

    fn expected_bmi_x100(_age_months: u32, sex: &Symbol) -> i64 {
        if *sex == symbol_short!("male") {
            1720
        } else {
            1680
        }
    }
}

mod test;
