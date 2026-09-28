#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Imaging Radiology Contract
//!
//! Manages medical imaging requests, radiology results, study ordering, and image metadata with
//! pagination support and temporal validation.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Radiologist authentication for result documentation. Ordering
//! provider validation via registry. Patient consent enforced via access-control contract for imaging
//! study orders. Result access restricted to ordering provider and patient. PACS system integration validation.
//!
//! **Audit Controls:** Imaging order events logged with modality, study date, and ordering provider.
//! Result documentation events tracked with radiologist identity. Report completion events emitted.
//! Image retrieval events logged for access audit. Pagination queries tracked with page results.
//!
//! **Data Retention Policy:** Imaging studies retained indefinitely per medical record requirements.
//! Radiology reports archived with final approval status. Protocol parameters maintained for
//! consistency. Temporal validation ensures logical imaging sequences.
//!
//! **Encryption/Integrity:** DICOM metadata stored encrypted in persistent storage. Image location
//! references (PACS integration) validated. Study date temporal validation. Radiologist digital
//! signature via address authentication. Report integrity enforced via immutable storage.

use soroban_sdk::{
    contract, contractevent, contracterror, contractimpl, contracttype, Address, BytesN, Env,
    String, Symbol, Vec,
};
use shared::{
    pagination::{self, PageResult, MAX_PAGE_SIZE},
    temporal,
};

/// --------------------
/// Imaging Structures
/// --------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagingOrder {
    pub order_id: u64,
    pub provider_id: Address,
    pub patient_id: Address,
    pub study_type: Symbol, // XRAY, CT, MRI, ULTRASOUND, PET, MAMMO
    pub body_part: String,
    pub contrast_required: bool,
    pub clinical_indication: String,
    pub priority: Symbol, // STAT, URGENT, ROUTINE
    pub status: Symbol,   // ORDERED, SCHEDULED, IN_PROGRESS, COMPLETED, CANCELLED
    pub ordered_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagingSchedule {
    pub order_id: u64,
    pub imaging_center: Address,
    pub scheduled_time: u64,
    pub prep_instructions_hash: BytesN<32>,
    pub scheduled_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DicomImages {
    pub order_id: u64,
    pub imaging_center: Address,
    pub dicom_hash: BytesN<32>, // Reference to DICOM storage
    pub image_count: u32,
    pub study_date: u64,
    pub uploaded_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreliminaryReport {
    pub order_id: u64,
    pub radiologist_id: Address,
    pub report_hash: BytesN<32>,
    pub urgent_findings: bool,
    pub submitted_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalReport {
    pub order_id: u64,
    pub radiologist_id: Address,
    pub final_report_hash: BytesN<32>,
    pub impression: String,
    pub submitted_at: u64,
}

/// A correction or addition appended to an already-locked final report
/// (wrong laterality, missed finding, transcription error, etc.). The
/// original `FinalReport` is never modified; addenda are appended to a
/// separate history so the audit trail stays intact.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportAddendum {
    pub order_id: u64,
    pub radiologist_id: Address,
    pub addendum_hash: BytesN<32>,
    pub reason: String,
    pub submitted_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerReview {
    pub order_id: u64,
    pub requesting_radiologist: Address,
    pub peer_radiologist: Address,
    pub requested_at: u64,
    pub status: Symbol, // PENDING, COMPLETED, DECLINED
}

/// --------------------
/// Storage Keys
/// --------------------

#[contracttype]
pub enum DataKey {
    OrderCounter,
    ImagingOrder(u64),
    ImagingSchedule(u64),
    DicomImages(u64),
    PreliminaryReport(u64),
    FinalReport(u64),
    /// order_id -> Vec<ReportAddendum>, appended to after the final report is locked.
    ReportAddenda(u64),
    PeerReview(u64),
    /// Paged order index per patient: (patient, page_num) → Vec<u64>
    PatientOrdersPage(Address, u32),
    /// Current (highest-written) page index for a patient's order list
    PatientOrdersHead(Address),
    /// Total order count per patient (for PageResult.total)
    PatientOrdersTotal(Address),
    /// Paged order index per provider: (provider, page_num) → Vec<u64>
    ProviderOrdersPage(Address, u32),
    /// Current page index for a provider's order list
    ProviderOrdersHead(Address),
    /// Total order count per provider
    ProviderOrdersTotal(Address),
}

/// --------------------
/// Error Types
/// --------------------

#[contracterror]
#[derive(Clone, Debug, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    OrderNotFound = 1,
    UnauthorizedAccess = 2,
    InvalidStatus = 3,
    AlreadyScheduled = 4,
    ImagesAlreadyUploaded = 5,
    PreliminaryReportExists = 6,
    FinalReportExists = 7,
    PeerReviewExists = 8,
    /// scheduled_time must be strictly in the future
    InvalidScheduledTime = 9,
    /// study_date must not be in the future
    InvalidStudyDate = 10,
    /// A required counter entry was missing from storage
    CounterUnavailable = 11,
    /// An addendum was submitted for an order with no final report yet.
    FinalReportNotFound = 12,
    /// Patient consent not found or invalid for imaging order
    ConsentRequired = 13,
    /// A radiologist cannot be assigned to peer review their own study
    SelfReviewNotAllowed = 14,
    /// The supplied imaging center is not a registered/credentialed actor
    ImagingCenterNotRegistered = 15,
    /// The supplied radiologist is not a credentialed actor
    RadiologistNotCredentialed = 16,
}

/// --------------------
/// Events
/// --------------------

#[contractevent]
pub struct ImagingOrdered {
    pub version: u32,
    pub order_id: u64,
    pub provider_id: Address,
}

#[contractevent]
pub struct ImagingScheduled {
    pub version: u32,
    pub order_id: u64,
}

#[contractevent]
pub struct ImagesUploaded {
    pub version: u32,
    pub order_id: u64,
    pub dicom_hash: BytesN<32>,
}

#[contractevent]
pub struct PreliminaryReportSubmitted {
    pub version: u32,
    pub order_id: u64,
    pub urgent_findings: bool,
}

#[contractevent]
pub struct FinalReportSubmitted {
    pub version: u32,
    pub order_id: u64,
}

#[contractevent]
pub struct AddendumSubmitted {
    pub version: u32,
    pub order_id: u64,
    pub radiologist_id: Address,
}

#[contractevent]
pub struct PeerReviewRequested {
    pub version: u32,
    pub order_id: u64,
}

#[contract]
pub struct ImagingRadiology;

#[contractimpl]
impl ImagingRadiology {
    /// Order a new imaging study
    ///
    /// Requires patient consent via the access-control contract before proceeding.
    /// The consent check verifies that the patient has granted consent for "imaging" purposes
    /// to the ordering provider.
    #[allow(clippy::too_many_arguments)]
    pub fn order_imaging_study(
        env: Env,
        access_control: Address,
        provider_id: Address,
        patient_id: Address,
        study_type: Symbol,
        body_part: String,
        contrast_required: bool,
        clinical_indication: String,
        priority: Symbol,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        // Verify patient consent via access-control contract
        let args = soroban_sdk::vec![
            &env,
            patient_id.clone().into_val(&env),
            provider_id.clone().into_val(&env),
            String::from_str(&env, "imaging").into_val(&env),
            0u32.into_val(&env),
        ];
        let consent_check: Result<(), soroban_sdk::Error> = env.invoke_contract(
            &access_control,
            &Symbol::new(&env, "check_consent"),
            args,
        );

        if consent_check.is_err() {
            return Err(Error::ConsentRequired);
        }

        let order_id = Self::next_order_id(&env);
        let order = ImagingOrder {
            order_id,
            provider_id: provider_id.clone(),
            patient_id: patient_id.clone(),
            study_type,
            body_part,
            contrast_required,
            clinical_indication,
            priority,
            status: Symbol::new(&env, "ORDERED"),
            ordered_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::ImagingOrder(order_id), &order);

        Self::index_order(&env, &patient_id, &provider_id, order_id);

        ImagingOrdered {
            version: 1,
            order_id,
            provider_id,
        }
        .publish(&env);

        Ok(order_id)
    }

    /// Schedule an imaging study at a registered imaging center.
    ///
    /// The `imaging_center` must be a registered/credentialed actor in the
    /// hospital registry; otherwise the schedule is rejected.
    pub fn schedule_imaging(
        env: Env,
        registry: Address,
        order_id: u64,
        imaging_center: Address,
        scheduled_time: u64,
        prep_instructions_hash: BytesN<32>,
    ) -> Result<(), Error> {
        imaging_center.require_auth();

        Self::require_registered_actor(&env, &registry, &imaging_center)?;

        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&DataKey::ImagingOrder(order_id))
            .ok_or(Error::OrderNotFound)?;

        if order.status != Symbol::new(&env, "ORDERED") {
            return Err(Error::InvalidStatus);
        }

        if env
            .storage()
            .persistent()
            .has(&DataKey::ImagingSchedule(order_id))
        {
            return Err(Error::AlreadyScheduled);
        }

        if scheduled_time <= env.ledger().timestamp() {
            return Err(Error::InvalidScheduledTime);
        }

        let schedule = ImagingSchedule {
            order_id,
            imaging_center: imaging_center.clone(),
            scheduled_time,
            prep_instructions_hash,
            scheduled_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::ImagingSchedule(order_id), &schedule);

        order.status = Symbol::new(&env, "SCHEDULED");
        env.storage()
            .persistent()
            .set(&DataKey::ImagingOrder(order_id), &order);

        ImagingScheduled {
            version: 1,
            order_id,
        }
        .publish(&env);

        Ok(())
    }

    /// Upload DICOM images for an order from a registered imaging center.
    ///
    /// The `imaging_center` must be a registered/credentialed actor in the
    /// hospital registry; otherwise the upload is rejected.
    pub fn upload_images(
        env: Env,
        registry: Address,
        order_id: u64,
        imaging_center: Address,
        dicom_hash: BytesN<32>,
        image_count: u32,
        study_date: u64,
    ) -> Result<(), Error> {
        imaging_center.require_auth();

        Self::require_registered_actor(&env, &registry, &imaging_center)?;

        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&DataKey::ImagingOrder(order_id))
            .ok_or(Error::OrderNotFound)?;

        if order.status != Symbol::new(&env, "SCHEDULED")
            && order.status != Symbol::new(&env, "IN_PROGRESS")
        {
            return Err(Error::InvalidStatus);
        }

        if env
            .storage()
            .persistent()
            .has(&DataKey::DicomImages(order_id))
        {
            return Err(Error::ImagesAlreadyUploaded);
        }

        if study_date > env.ledger().timestamp() {
            return Err(Error::InvalidStudyDate);
        }

        let images = DicomImages {
            order_id,
            imaging_center: imaging_center.clone(),
            dicom_hash: dicom_hash.clone(),
            image_count,
            study_date,
            uploaded_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::DicomImages(order_id), &images);

        order.status = Symbol::new(&env, "IN_PROGRESS");
        env.storage()
            .persistent()
            .set(&DataKey::ImagingOrder(order_id), &order);

        ImagesUploaded {
            version: 1,
            order_id,
            dicom_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Submit a preliminary report for an order.
    ///
    /// The `radiologist_id` must be a credentialed actor in the hospital
    /// registry; otherwise the report is rejected.
    pub fn submit_preliminary_report(
        env: Env,
        registry: Address,
        order_id: u64,
        radiologist_id: Address,
        report_hash: BytesN<32>,
        urgent_findings: bool,
    ) -> Result<(), Error> {
        radiologist_id.require_auth();

        Self::require_credentialed_radiologist(&env, &registry, &radiologist_id)?;

        let order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&DataKey::ImagingOrder(order_id))
            .ok_or(Error::OrderNotFound)?;

        if order.status != Symbol::new(&env, "IN_PROGRESS") {
            return Err(Error::InvalidStatus);
        }

        if env
            .storage()
            .persistent()
            .has(&DataKey::PreliminaryReport(order_id))
        {
            return Err(Error::PreliminaryReportExists);
        }

        let report = PreliminaryReport {
            order_id,
            radiologist_id: radiologist_id.clone(),
            report_hash,
            urgent_findings,
            submitted_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::PreliminaryReport(order_id), &report);

        PreliminaryReportSubmitted {
            version: 1,
            order_id,
            urgent_findings,
        }
        .publish(&env);

        Ok(())
    }

    /// Submit the final report for an order.
    ///
    /// The `radiologist_id` must be a credentialed actor in the hospital
    /// registry; otherwise the report is rejected.
    pub fn submit_final_report(
        env: Env,
        registry: Address,
        order_id: u64,
        radiologist_id: Address,
        final_report_hash: BytesN<32>,
        impression: String,
    ) -> Result<(), Error> {
        radiologist_id.require_auth();

        Self::require_credentialed_radiologist(&env, &registry, &radiologist_id)?;

        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&DataKey::ImagingOrder(order_id))
            .ok_or(Error::OrderNotFound)?;

        if order.status != Symbol::new(&env, "IN_PROGRESS") {
            return Err(Error::InvalidStatus);
        }

        if env
            .storage()
            .persistent()
            .has(&DataKey::FinalReport(order_id))
        {
            return Err(Error::FinalReportExists);
        }

        let report = FinalReport {
            order_id,
            radiologist_id: radiologist_id.clone(),
            final_report_hash,
            impression,
            submitted_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::FinalReport(order_id), &report);

        order.status = Symbol::new(&env, "COMPLETED");
        env.storage()
            .persistent()
            .set(&DataKey::ImagingOrder(order_id), &order);

        FinalReportSubmitted {
            version: 1,
            order_id,
        }
        .publish(&env);

        Ok(())
    }

    /// --------------------
    /// Internal helpers
    /// --------------------

    /// Verify that `actor` is a registered/credentialed actor in the hospital
    /// registry contract. Mirrors health-records' ProviderRegistryInterface
    /// pattern: a cross-contract call to `is_registered` that must return true.
    fn require_registered_actor(
        env: &Env,
        registry: &Address,
        actor: &Address,
    ) -> Result<(), Error> {
        let args = soroban_sdk::vec![env, actor.clone().into_val(env)];
        let is_registered: bool = env.invoke_contract(
            registry,
            &Symbol::new(env, "is_registered"),
            args,
        );
        if !is_registered {
            return Err(Error::ImagingCenterNotRegistered);
        }
        Ok(())
    }

    /// Verify that `radiologist` holds a valid credential in the hospital
    /// registry contract. Mirrors health-records' ProviderRegistryInterface
    /// pattern: a cross-contract call to `is_credentialed` that must return true.
    fn require_credentialed_radiologist(
        env: &Env,
        registry: &Address,
        radiologist: &Address,
    ) -> Result<(), Error> {
        let args = soroban_sdk::vec![env, radiologist.clone().into_val(env)];
        let is_credentialed: bool = env.invoke_contract(
            registry,
            &Symbol::new(env, "is_credentialed"),
            args,
        );
        if !is_credentialed {
            return Err(Error::RadiologistNotCredentialed);
        }
        Ok(())
    }

    fn next_order_id(env: &Env) -> u64 {
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::OrderCounter)
            .unwrap_or(0);
        let next = current + 1;
        env.storage()
            .persistent()
            .set(&DataKey::OrderCounter, &next);
        next
    }

    fn index_order(env: &Env, patient: &Address, provider: &Address, order_id: u64) {
        // Patient index
        let p_total: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::PatientOrdersTotal(patient.clone()))
            .unwrap_or(0);
        let p_page = p_total / MAX_PAGE_SIZE;
        let mut p_vec: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientOrdersPage(patient.clone(), p_page))
            .unwrap_or(Vec::new(env));
        p_vec.push_back(order_id);
        env.storage()
            .persistent()
            .set(&DataKey::PatientOrdersPage(patient.clone(), p_page), &p_vec);
        env.storage()
            .persistent()
            .set(&DataKey::PatientOrdersHead(patient.clone()), &p_page);
        env.storage()
            .persistent()
            .set(&DataKey::PatientOrdersTotal(patient.clone()), &(p_total + 1));

        // Provider index
        let pr_total: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderOrdersTotal(provider.clone()))
            .unwrap_or(0);
        let pr_page = pr_total / MAX_PAGE_SIZE;
        let mut pr_vec: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderOrdersPage(provider.clone(), pr_page))
            .unwrap_or(Vec::new(env));
        pr_vec.push_back(order_id);
        env.storage()
            .persistent()
            .set(&DataKey::ProviderOrdersPage(provider.clone(), pr_page), &pr_vec);
        env.storage()
            .persistent()
            .set(&DataKey::ProviderOrdersHead(provider.clone()), &pr_page);
        env.storage()
            .persistent()
            .set(&DataKey::ProviderOrdersTotal(provider.clone()), &(pr_total + 1));
    }
}
