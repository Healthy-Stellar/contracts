#![no_std]

use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Vec, contract, contracterror, contractimpl, contracttype,
    panic_with_error,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Expired = 1,
    Unauthorized = 2,
    InvalidPrescription = 3,
    AlreadyExists = 4,
    NotFound = 5,
    InvalidSeverity = 6,
    InteractionNotFound = 7,
    MissingOverrideReason = 8,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Medication {
    pub ndc_code: String,
    pub generic_name: String,
    pub brand_names: Vec<String>,
    pub drug_class: Symbol,
    pub interaction_profile_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Interaction {
    pub id: u64,
    pub drug1_ndc: String,
    pub drug2_ndc: String,
    pub severity: Symbol,
    pub interaction_type: Symbol,
    pub clinical_effects: String,
    pub management_strategy: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionWarning {
    pub drug1: String,
    pub drug2: String,
    pub severity: Symbol,
    pub interaction_type: Symbol,
    pub clinical_effects: String,
    pub management: String,
    pub documentation_required: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionOverride {
    pub provider_id: Address,
    pub patient_id: Address,
    pub medication: String,
    pub interaction_id: u64,
    pub override_reason: String,
    pub timestamp: u64,
}

#[contracttype]
pub enum DataKey {
    Medication(String),
    InteractionCounter,
    InteractionById(u64),
    InteractionPair(String, String),
    PatientAllergies(Address),
    PatientConditions(Address),
    MedicationContraindications(String),
    InteractionOverride(u64, Address),
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

    pub fn register_medication(
        env: Env,
        ndc_code: String,
        generic_name: String,
        brand_names: Vec<String>,
        drug_class: Symbol,
        interaction_profile_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let key = DataKey::Medication(ndc_code.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }

        let medication = Medication {
            ndc_code,
            generic_name,
            brand_names,
            drug_class,
            interaction_profile_hash,
        };

        env.storage().persistent().set(&key, &medication);
        Ok(())
    }

    pub fn add_interaction(
        env: Env,
        drug1_ndc: String,
        drug2_ndc: String,
        severity: Symbol,
        interaction_type: Symbol,
        clinical_effects: String,
        management_strategy: String,
    ) -> Result<(), Error> {
        if !is_valid_severity(&env, &severity) {
            return Err(Error::InvalidSeverity);
        }

        let med1_key = DataKey::Medication(drug1_ndc.clone());
        let med2_key = DataKey::Medication(drug2_ndc.clone());
        if !env.storage().persistent().has(&med1_key) || !env.storage().persistent().has(&med2_key)
        {
            return Err(Error::NotFound);
        }

        let pair_key = DataKey::InteractionPair(drug1_ndc.clone(), drug2_ndc.clone());
        if env.storage().persistent().has(&pair_key) {
            return Err(Error::AlreadyExists);
        }

        let interaction_id = env
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::InteractionCounter)
            .unwrap_or(0)
            + 1;

        let interaction = Interaction {
            id: interaction_id,
            drug1_ndc: drug1_ndc.clone(),
            drug2_ndc: drug2_ndc.clone(),
            severity,
            interaction_type,
            clinical_effects,
            management_strategy,
        };

        env.storage()
            .persistent()
            .set(&DataKey::InteractionById(interaction_id), &interaction);
        env.storage().persistent().set(
            &DataKey::InteractionPair(drug1_ndc.clone(), drug2_ndc.clone()),
            &interaction_id,
        );
        env.storage().persistent().set(
            &DataKey::InteractionPair(drug2_ndc, drug1_ndc),
            &interaction_id,
        );
        env.storage()
            .instance()
            .set(&DataKey::InteractionCounter, &interaction_id);

        Ok(())
    }

    pub fn check_interactions(
        env: Env,
        _patient_id: Address,
        new_medication: String,
        current_medications: Vec<String>,
    ) -> Result<Vec<InteractionWarning>, Error> {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Medication(new_medication.clone()))
        {
            return Err(Error::NotFound);
        }

        let mut warnings = Vec::new(&env);
        for current in current_medications {
            let pair_key = DataKey::InteractionPair(new_medication.clone(), current.clone());
            if let Some(interaction_id) = env.storage().persistent().get::<_, u64>(&pair_key) {
                let interaction: Interaction = env
                    .storage()
                    .persistent()
                    .get(&DataKey::InteractionById(interaction_id))
                    .ok_or(Error::InteractionNotFound)?;

                warnings.push_back(InteractionWarning {
                    drug1: interaction.drug1_ndc,
                    drug2: interaction.drug2_ndc,
                    severity: interaction.severity.clone(),
                    interaction_type: interaction.interaction_type,
                    clinical_effects: interaction.clinical_effects,
                    management: interaction.management_strategy,
                    documentation_required: requires_documentation(&env, &interaction.severity),
                });
            }
        }

        Ok(warnings)
    }

    pub fn check_allergy_interaction(
        env: Env,
        patient_id: Address,
        medication: String,
    ) -> Result<Vec<InteractionWarning>, Error> {
        let med: Medication = env
            .storage()
            .persistent()
            .get(&DataKey::Medication(medication.clone()))
            .ok_or(Error::NotFound)?;

        let allergies: Vec<String> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientAllergies(patient_id))
            .unwrap_or(Vec::new(&env));

        let mut warnings = Vec::new(&env);
        for allergy in allergies {
            let is_brand_match = contains_string(&med.brand_names, &allergy);
            if med.generic_name == allergy || med.ndc_code == allergy || is_brand_match {
                warnings.push_back(InteractionWarning {
                    drug1: med.ndc_code.clone(),
                    drug2: allergy,
                    severity: Symbol::new(&env, "contraindicated"),
                    interaction_type: Symbol::new(&env, "allergy"),
                    clinical_effects: String::from_str(
                        &env,
                        "Potential hypersensitivity or allergic reaction.",
                    ),
                    management: String::from_str(
                        &env,
                        "Avoid medication and prescribe a non-cross-reactive alternative.",
                    ),
                    documentation_required: true,
                });
            }
        }

        Ok(warnings)
    }

    pub fn get_contraindications(
        env: Env,
        patient_id: Address,
        medication: String,
        conditions: Vec<String>,
    ) -> Result<Vec<String>, Error> {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Medication(medication.clone()))
        {
            return Err(Error::NotFound);
        }

        let mut all_conditions = conditions;
        let patient_conditions: Vec<String> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientConditions(patient_id))
            .unwrap_or(Vec::new(&env));

        for condition in patient_conditions {
            if !contains_string(&all_conditions, &condition) {
                all_conditions.push_back(condition);
            }
        }

        let contraindications: Vec<String> = env
            .storage()
            .persistent()
            .get(&DataKey::MedicationContraindications(medication))
            .unwrap_or(Vec::new(&env));

        let mut matched = Vec::new(&env);
        for contraindication in contraindications {
            if contains_string(&all_conditions, &contraindication) {
                matched.push_back(contraindication);
            }
        }

        Ok(matched)
    }

    pub fn override_interaction_warning(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        medication: String,
        interaction_id: u64,
        override_reason: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        if override_reason == String::from_str(&env, "") {
            return Err(Error::MissingOverrideReason);
        }

        if !env
            .storage()
            .persistent()
            .has(&DataKey::InteractionById(interaction_id))
        {
            return Err(Error::InteractionNotFound);
        }

        let override_record = InteractionOverride {
            provider_id,
            patient_id: patient_id.clone(),
            medication,
            interaction_id,
            override_reason,
            timestamp: env.ledger().timestamp(),
        };

        env.storage().persistent().set(
            &DataKey::InteractionOverride(interaction_id, patient_id),
            &override_record,
        );

        Ok(())
    }

    pub fn set_patient_allergies(
        env: Env,
        patient_id: Address,
        allergies: Vec<String>,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::PatientAllergies(patient_id), &allergies);
        Ok(())
    }

    pub fn set_patient_conditions(
        env: Env,
        patient_id: Address,
        conditions: Vec<String>,
    ) -> Result<(), Error> {
        patient_id.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::PatientConditions(patient_id), &conditions);
        Ok(())
    }

    pub fn set_medication_contraindications(
        env: Env,
        medication: String,
        contraindications: Vec<String>,
    ) -> Result<(), Error> {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Medication(medication.clone()))
        {
            return Err(Error::NotFound);
        }

        env.storage().persistent().set(
            &DataKey::MedicationContraindications(medication),
            &contraindications,
        );
        Ok(())
    }
}

fn is_valid_severity(env: &Env, severity: &Symbol) -> bool {
    *severity == Symbol::new(env, "minor")
        || *severity == Symbol::new(env, "moderate")
        || *severity == Symbol::new(env, "major")
        || *severity == Symbol::new(env, "contraindicated")
}

fn requires_documentation(env: &Env, severity: &Symbol) -> bool {
    *severity == Symbol::new(env, "major") || *severity == Symbol::new(env, "contraindicated")
}

fn contains_string(values: &Vec<String>, needle: &String) -> bool {
    for value in values.iter() {
        if value == *needle {
            return true;
        }
    }

    false
}

mod test;
