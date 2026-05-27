#![no_std]

use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Vec, contract, contracterror, contractimpl, contracttype,
};
use shared::temporal;

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
    InvalidStatusTransition = 9,
    InvalidTransfer = 10,
    QuantityExceeded = 11,
    RefillExceeded = 12,
    PharmacyNotAuthorized = 13,
    TransferChainBroken = 14,
    MissingTransferReason = 15,
    ControlledSubstanceViolation = 16,
    RegistryGoverned = 17,
    HighImpactRequiresProposal = 18,
    ProposalNotFound = 19,
    ProposalAlreadyFinalized = 20,
    /// valid_until must be in the future and within MAX_VALIDITY_WINDOW_SECS of issue time
    InvalidValidityWindow = 21,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryProposalAction {
    Medication(Medication),
    Interaction(Interaction),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryProposal {
    pub id: u64,
    pub proposer: Address,
    pub action: RegistryProposalAction,
    pub created_at: u64,
    pub approved: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogSnapshot {
    pub version: u64,
    pub created_by: Address,
    pub created_at: u64,
    pub medication_count: u32,
    pub interaction_count: u32,
}

#[contracttype]
pub enum DataKey {
    Medication(String),
    MedicationCatalog,
    InteractionCounter,
    InteractionById(u64),
    InteractionPair(String, String),
    InteractionCatalog,
    PatientAllergies(Address),
    PatientConditions(Address),
    MedicationContraindications(String),
    InteractionOverride(u64, Address),
    RegistryAdmin,
    RegistryWriter(Address),
    RegistryProposalCounter,
    RegistryProposal(u64),
    SnapshotCounter,
    CatalogSnapshot(u64),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrescriptionStatus {
    Issued,
    Active,
    Dispensed,
    PartiallyDispensed,
    Expired,
    Transferred,
    Cancelled,
    Suspended,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Prescription {
    pub provider_id: Address,
    pub patient_id: Address,
    pub medication_name: String,
    pub quantity: u32,
    pub quantity_dispensed: u32,
    pub refills_allowed: u32,
    pub refills_remaining: u32,
    pub refills_used: u32,
    pub is_controlled: bool,
    pub schedule: Option<u32>, // Controlled substance schedule
    pub current_pharmacy: Option<Address>,
    pub issuing_pharmacy: Option<Address>,
    pub status: PrescriptionStatus,
    pub issued_at: u64,
    pub valid_until: u64,
    pub last_dispensed: Option<u64>,
    pub transfer_count: u32,
    pub transfer_history: Vec<TransferRecord>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TransferRecord {
    pub from_pharmacy: Address,
    pub to_pharmacy: Address,
    pub transfer_reason: String,
    pub transferred_at: u64,
    pub transferred_by: Address,
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
    pub pharmacy_id: Option<Address>,
}

#[contracttype]
pub struct TransferRequest {
    pub prescription_id: u64,
    pub to_pharmacy: Address,
    pub transfer_reason: String,
    pub urgency: Symbol,
}

#[contracttype]
pub struct DispenseRequest {
    pub prescription_id: u64,
    pub quantity: u32,
    pub lot: String,
    pub expires_at: u64,
    pub ndc_code: String,
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
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        // #215 – valid_until must be in the future and within a 1-year window
        temporal::must_be_future(&env, req.valid_until)
            .map_err(|_| Error::InvalidValidityWindow)?;
        temporal::within_validity_window(
            env.ledger().timestamp(),
            req.valid_until,
            shared::temporal::MAX_VALIDITY_WINDOW_SECS,
        )
        .map_err(|_| Error::InvalidValidityWindow)?;

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
            quantity_dispensed: 0,
            refills_allowed: req.refills_allowed,
            refills_remaining: req.refills_allowed,
            refills_used: 0,
            is_controlled: req.is_controlled,
            schedule: req.schedule,
            current_pharmacy: req.pharmacy_id.clone(),
            issuing_pharmacy: req.pharmacy_id,
            status: PrescriptionStatus::Issued,
            issued_at: env.ledger().timestamp(),
            valid_until: req.valid_until,
            last_dispensed: None,
            transfer_count: 0,
            transfer_history: Vec::new(&env),
        };

        env.storage().persistent().set(&id, &prescription);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "ID_COUNTER"), &(id + 1));

        Ok(id)
    }

    pub fn dispense_prescription(
        env: Env,
        req: DispenseRequest,
        pharmacy_id: Address,
    ) -> Result<(), Error> {
        pharmacy_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&req.prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate prescription is in dispensible state
        if !matches!(
            p.status,
            PrescriptionStatus::Issued
                | PrescriptionStatus::Active
                | PrescriptionStatus::PartiallyDispensed
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Check expiration
        if env.ledger().timestamp() > p.valid_until {
            return Err(Error::Expired);
        }

        // Validate pharmacy authorization
        if let Some(ref current_pharmacy) = p.current_pharmacy {
            if current_pharmacy != &pharmacy_id {
                return Err(Error::PharmacyNotAuthorized);
            }
        } else {
            // First dispense sets the pharmacy
            p.current_pharmacy = Some(pharmacy_id.clone());
        }

        // Validate quantity constraints
        if p.quantity_dispensed + req.quantity > p.quantity {
            return Err(Error::QuantityExceeded);
        }

        // Controlled substance checks
        if p.is_controlled {
            if let Some(schedule) = p.schedule {
                if schedule == 2 && req.quantity > p.quantity / 2 {
                    return Err(Error::ControlledSubstanceViolation);
                }
            }
        }

        // Update prescription state
        p.quantity_dispensed += req.quantity;
        p.last_dispensed = Some(env.ledger().timestamp());

        // Update status based on remaining quantity
        if p.quantity_dispensed >= p.quantity {
            p.status = PrescriptionStatus::Dispensed;
        } else {
            p.status = PrescriptionStatus::PartiallyDispensed;
        }

        env.storage().persistent().set(&req.prescription_id, &p);

        // Emit dispense event — quantity omitted to prevent clinical PII exposure on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_dispensed"),),
            (req.prescription_id, pharmacy_id),
        );

        Ok(())
    }

    pub fn transfer_prescription(
        env: Env,
        req: TransferRequest,
        from_pharmacy: Address,
    ) -> Result<(), Error> {
        from_pharmacy.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&req.prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate transfer reason
        if req.transfer_reason.is_empty() {
            return Err(Error::MissingTransferReason);
        }

        // Verify current pharmacy ownership
        if let Some(current_pharmacy) = p.current_pharmacy {
            if current_pharmacy != from_pharmacy {
                return Err(Error::PharmacyNotAuthorized);
            }
        } else {
            return Err(Error::TransferChainBroken);
        }

        // Validate prescription is transferable
        if !matches!(
            p.status,
            PrescriptionStatus::Issued
                | PrescriptionStatus::Active
                | PrescriptionStatus::PartiallyDispensed
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Check expiration
        if env.ledger().timestamp() > p.valid_until {
            return Err(Error::Expired);
        }

        // Transfer limits for controlled substances
        if p.is_controlled && p.transfer_count >= 1 {
            return Err(Error::ControlledSubstanceViolation);
        }

        // Create transfer record
        let transfer_record = TransferRecord {
            from_pharmacy: from_pharmacy.clone(),
            to_pharmacy: req.to_pharmacy.clone(),
            transfer_reason: req.transfer_reason.clone(),
            transferred_at: env.ledger().timestamp(),
            transferred_by: from_pharmacy.clone(),
        };

        // Update prescription
        p.transfer_history.push_back(transfer_record);
        p.transfer_count += 1;
        p.current_pharmacy = Some(req.to_pharmacy.clone());
        p.status = PrescriptionStatus::Transferred;

        env.storage().persistent().set(&req.prescription_id, &p);

        // Emit transfer event — transfer_reason omitted to avoid free-text PII on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_transferred"),),
            (req.prescription_id, from_pharmacy, req.to_pharmacy),
        );

        Ok(())
    }

    pub fn accept_transfer(
        env: Env,
        prescription_id: u64,
        pharmacy_id: Address,
    ) -> Result<(), Error> {
        pharmacy_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&prescription_id)
            .ok_or(Error::NotFound)?;

        // Verify pharmacy is the destination
        if let Some(ref current_pharmacy) = p.current_pharmacy {
            if current_pharmacy != &pharmacy_id {
                return Err(Error::PharmacyNotAuthorized);
            }
        } else {
            return Err(Error::TransferChainBroken);
        }

        // Validate status
        if !matches!(p.status, PrescriptionStatus::Transferred) {
            return Err(Error::InvalidStatusTransition);
        }

        // Accept transfer and activate prescription
        p.status = PrescriptionStatus::Active;
        env.storage().persistent().set(&prescription_id, &p);

        // Emit acceptance event
        env.events().publish(
            (Symbol::new(&env, "transfer_accepted"),),
            (prescription_id, pharmacy_id),
        );

        Ok(())
    }

    pub fn register_medication(
        env: Env,
        ndc_code: String,
        generic_name: String,
        brand_names: Vec<String>,
        drug_class: Symbol,
        interaction_profile_hash: BytesN<32>,
    ) -> Result<(), Error> {
        if is_registry_governed(&env) {
            return Err(Error::RegistryGoverned);
        }
        put_medication(
            &env,
            Medication {
                ndc_code,
                generic_name,
                brand_names,
                drug_class,
                interaction_profile_hash,
            },
        )
    }

    pub fn initialize_registry_governance(env: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        if is_registry_governed(&env) {
            return Err(Error::AlreadyExists);
        }
        env.storage()
            .persistent()
            .set(&DataKey::RegistryAdmin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::RegistryWriter(admin.clone()), &true);
        Ok(())
    }

    pub fn add_registry_writer(env: Env, admin: Address, writer: Address) -> Result<(), Error> {
        require_registry_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::RegistryWriter(writer), &true);
        Ok(())
    }

    pub fn register_medication_by(
        env: Env,
        writer: Address,
        ndc_code: String,
        generic_name: String,
        brand_names: Vec<String>,
        drug_class: Symbol,
        interaction_profile_hash: BytesN<32>,
    ) -> Result<(), Error> {
        require_registry_writer(&env, &writer)?;
        put_medication(
            &env,
            Medication {
                ndc_code,
                generic_name,
                brand_names,
                drug_class,
                interaction_profile_hash,
            },
        )
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
        if is_registry_governed(&env) {
            return Err(Error::RegistryGoverned);
        }
        put_interaction(
            &env,
            drug1_ndc,
            drug2_ndc,
            severity,
            interaction_type,
            clinical_effects,
            management_strategy,
        )
    }

    pub fn add_interaction_by(
        env: Env,
        writer: Address,
        drug1_ndc: String,
        drug2_ndc: String,
        severity: Symbol,
        interaction_type: Symbol,
        clinical_effects: String,
        management_strategy: String,
    ) -> Result<(), Error> {
        require_registry_writer(&env, &writer)?;
        if requires_documentation(&env, &severity) {
            return Err(Error::HighImpactRequiresProposal);
        }
        put_interaction(
            &env,
            drug1_ndc,
            drug2_ndc,
            severity,
            interaction_type,
            clinical_effects,
            management_strategy,
        )
    }

    pub fn propose_interaction_update(
        env: Env,
        writer: Address,
        drug1_ndc: String,
        drug2_ndc: String,
        severity: Symbol,
        interaction_type: Symbol,
        clinical_effects: String,
        management_strategy: String,
    ) -> Result<u64, Error> {
        require_registry_writer(&env, &writer)?;
        if !is_valid_severity(&env, &severity) {
            return Err(Error::InvalidSeverity);
        }
        if !medications_exist(&env, &drug1_ndc, &drug2_ndc) {
            return Err(Error::NotFound);
        }
        if env.storage().persistent().has(&DataKey::InteractionPair(
            drug1_ndc.clone(),
            drug2_ndc.clone(),
        )) {
            return Err(Error::AlreadyExists);
        }

        create_registry_proposal(
            &env,
            writer,
            RegistryProposalAction::Interaction(Interaction {
                id: 0,
                drug1_ndc: drug1_ndc.clone(),
                drug2_ndc: drug2_ndc.clone(),
                severity,
                interaction_type,
                clinical_effects,
                management_strategy,
            }),
        )
    }

    pub fn propose_medication_update(
        env: Env,
        writer: Address,
        ndc_code: String,
        generic_name: String,
        brand_names: Vec<String>,
        drug_class: Symbol,
        interaction_profile_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        require_registry_writer(&env, &writer)?;
        let key = DataKey::Medication(ndc_code.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }
        create_registry_proposal(
            &env,
            writer,
            RegistryProposalAction::Medication(Medication {
                ndc_code,
                generic_name,
                brand_names,
                drug_class,
                interaction_profile_hash,
            }),
        )
    }

    pub fn approve_registry_proposal(
        env: Env,
        admin: Address,
        proposal_id: u64,
    ) -> Result<(), Error> {
        require_registry_admin(&env, &admin)?;
        let mut proposal: RegistryProposal = env
            .storage()
            .persistent()
            .get(&DataKey::RegistryProposal(proposal_id))
            .ok_or(Error::ProposalNotFound)?;
        if proposal.approved {
            return Err(Error::ProposalAlreadyFinalized);
        }
        match proposal.action.clone() {
            RegistryProposalAction::Medication(medication) => put_medication(&env, medication)?,
            RegistryProposalAction::Interaction(interaction) => put_interaction(
                &env,
                interaction.drug1_ndc,
                interaction.drug2_ndc,
                interaction.severity,
                interaction.interaction_type,
                interaction.clinical_effects,
                interaction.management_strategy,
            )?,
        }
        proposal.approved = true;
        env.storage()
            .persistent()
            .set(&DataKey::RegistryProposal(proposal_id), &proposal);
        Ok(())
    }

    pub fn create_catalog_snapshot(env: Env, admin: Address) -> Result<u64, Error> {
        require_registry_admin(&env, &admin)?;
        let version = env
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::SnapshotCounter)
            .unwrap_or(0)
            + 1;
        let medication_count = medication_catalog_len(&env);
        let interaction_count = interaction_catalog_len(&env);
        let snapshot = CatalogSnapshot {
            version,
            created_by: admin,
            created_at: env.ledger().timestamp(),
            medication_count,
            interaction_count,
        };
        env.storage()
            .persistent()
            .set(&DataKey::CatalogSnapshot(version), &snapshot);
        env.storage()
            .instance()
            .set(&DataKey::SnapshotCounter, &version);
        Ok(version)
    }

    pub fn get_catalog_snapshot(env: Env, version: u64) -> Result<CatalogSnapshot, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::CatalogSnapshot(version))
            .ok_or(Error::NotFound)
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

                // #302: Validate severity against the explicit allowlist.
                // Silently-invalid severities would bypass requires_documentation(),
                // which always returns false for unrecognised symbols.
                if !is_valid_severity(&env, &interaction.severity) {
                    return Err(Error::InvalidSeverity);
                }

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

    pub fn refill_prescription(
        env: Env,
        prescription_id: u64,
        pharmacy_id: Address,
        provider_id: Address,
    ) -> Result<(), Error> {
        pharmacy_id.require_auth();
        provider_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate prescription allows refills
        if p.refills_allowed == 0 {
            return Err(Error::RefillExceeded);
        }

        // Check remaining refills
        if p.refills_remaining == 0 {
            return Err(Error::RefillExceeded);
        }

        // Validate prescription is in refillable state
        if !matches!(
            p.status,
            PrescriptionStatus::Active
                | PrescriptionStatus::PartiallyDispensed
                | PrescriptionStatus::Dispensed
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Check expiration
        if env.ledger().timestamp() > p.valid_until {
            return Err(Error::Expired);
        }

        // Validate pharmacy authorization
        if let Some(ref current_pharmacy) = p.current_pharmacy {
            if current_pharmacy != &pharmacy_id {
                return Err(Error::PharmacyNotAuthorized);
            }
        } else {
            return Err(Error::PharmacyNotAuthorized);
        }

        // Validate provider authorization
        if p.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        // Decrement refills and reset quantity for new fill
        p.refills_remaining -= 1;
        p.refills_used += 1;
        p.quantity_dispensed = 0;
        p.status = PrescriptionStatus::Active;
        p.last_dispensed = None;

        // Extend validity if needed (30 days from refill)
        let new_valid_until = env.ledger().timestamp() + (30 * 24 * 60 * 60); // 30 days in seconds
        if new_valid_until > p.valid_until {
            p.valid_until = new_valid_until;
        }

        env.storage().persistent().set(&prescription_id, &p);

        // Emit refill event — refills_remaining omitted to avoid clinical detail on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_refilled"),),
            (prescription_id, pharmacy_id, provider_id),
        );

        Ok(())
    }

    pub fn cancel_prescription(
        env: Env,
        prescription_id: u64,
        provider_id: Address,
        reason: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut p: Prescription = env
            .storage()
            .persistent()
            .get(&prescription_id)
            .ok_or(Error::NotFound)?;

        // Validate provider authorization
        if p.provider_id != provider_id {
            return Err(Error::Unauthorized);
        }

        // Only active or issued prescriptions can be cancelled
        if !matches!(
            p.status,
            PrescriptionStatus::Issued
                | PrescriptionStatus::Active
                | PrescriptionStatus::PartiallyDispensed
        ) {
            return Err(Error::InvalidStatusTransition);
        }

        // Cannot cancel if already partially dispensed (unless for safety reasons)
        if matches!(p.status, PrescriptionStatus::PartiallyDispensed) && p.quantity_dispensed > 0 {
            if reason != String::from_str(&env, "safety_concern")
                && reason != String::from_str(&env, "adverse_reaction")
            {
                return Err(Error::InvalidStatusTransition);
            }
        }

        p.status = PrescriptionStatus::Cancelled;
        env.storage().persistent().set(&prescription_id, &p);

        // Emit cancellation event — reason omitted to avoid free-text PII on-chain (#227)
        env.events().publish(
            (Symbol::new(&env, "prescription_cancelled"),),
            (prescription_id, provider_id),
        );

        Ok(())
    }
}

fn is_registry_governed(env: &Env) -> bool {
    env.storage().persistent().has(&DataKey::RegistryAdmin)
}

fn require_registry_admin(env: &Env, admin: &Address) -> Result<(), Error> {
    admin.require_auth();
    let configured_admin: Address = env
        .storage()
        .persistent()
        .get(&DataKey::RegistryAdmin)
        .ok_or(Error::Unauthorized)?;
    if configured_admin != *admin {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

fn require_registry_writer(env: &Env, writer: &Address) -> Result<(), Error> {
    writer.require_auth();
    if !is_registry_governed(env) {
        return Err(Error::Unauthorized);
    }
    let authorized = env
        .storage()
        .persistent()
        .get::<_, bool>(&DataKey::RegistryWriter(writer.clone()))
        .unwrap_or(false);
    if !authorized {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

fn put_medication(env: &Env, medication: Medication) -> Result<(), Error> {
    let key = DataKey::Medication(medication.ndc_code.clone());
    if env.storage().persistent().has(&key) {
        return Err(Error::AlreadyExists);
    }

    let mut catalog: Vec<String> = env
        .storage()
        .persistent()
        .get(&DataKey::MedicationCatalog)
        .unwrap_or(Vec::new(env));
    catalog.push_back(medication.ndc_code.clone());

    env.storage().persistent().set(&key, &medication);
    env.storage()
        .persistent()
        .set(&DataKey::MedicationCatalog, &catalog);
    Ok(())
}

fn put_interaction(
    env: &Env,
    drug1_ndc: String,
    drug2_ndc: String,
    severity: Symbol,
    interaction_type: Symbol,
    clinical_effects: String,
    management_strategy: String,
) -> Result<(), Error> {
    if !is_valid_severity(env, &severity) {
        return Err(Error::InvalidSeverity);
    }

    if !medications_exist(env, &drug1_ndc, &drug2_ndc) {
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

    let mut catalog: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::InteractionCatalog)
        .unwrap_or(Vec::new(env));
    catalog.push_back(interaction_id);

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
        .persistent()
        .set(&DataKey::InteractionCatalog, &catalog);
    env.storage()
        .instance()
        .set(&DataKey::InteractionCounter, &interaction_id);

    Ok(())
}

fn medications_exist(env: &Env, drug1_ndc: &String, drug2_ndc: &String) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Medication(drug1_ndc.clone()))
        && env
            .storage()
            .persistent()
            .has(&DataKey::Medication(drug2_ndc.clone()))
}

fn create_registry_proposal(
    env: &Env,
    proposer: Address,
    action: RegistryProposalAction,
) -> Result<u64, Error> {
    let id = env
        .storage()
        .instance()
        .get::<_, u64>(&DataKey::RegistryProposalCounter)
        .unwrap_or(0)
        + 1;
    let proposal = RegistryProposal {
        id,
        proposer,
        action,
        created_at: env.ledger().timestamp(),
        approved: false,
    };
    env.storage()
        .persistent()
        .set(&DataKey::RegistryProposal(id), &proposal);
    env.storage()
        .instance()
        .set(&DataKey::RegistryProposalCounter, &id);
    Ok(id)
}

fn medication_catalog_len(env: &Env) -> u32 {
    let catalog: Vec<String> = env
        .storage()
        .persistent()
        .get(&DataKey::MedicationCatalog)
        .unwrap_or(Vec::new(env));
    catalog.len()
}

fn interaction_catalog_len(env: &Env) -> u32 {
    let catalog: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::InteractionCatalog)
        .unwrap_or(Vec::new(env));
    catalog.len()
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

#[cfg(test)]
mod test;
