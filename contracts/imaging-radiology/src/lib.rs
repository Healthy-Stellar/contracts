#![no_std]
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contractevent, contracterror, contractimpl, contracttype, Address, BytesN, Env,
    String, Symbol,
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
pub struct PeerReviewRequested {
    pub version: u32,
    pub order_id: u64,
}

#[contract]
pub struct ImagingRadiology;

#[contractimpl]
impl ImagingRadiology {
    /// Order a new imaging study
    #[allow(clippy::too_many_arguments)]
    pub fn order_imaging_study(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        study_type: Symbol,
        body_part: String,
        contrast_required: bool,
        clinical_indication: String,
        priority: Symbol,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let counter_key = DataKey::OrderCounter;
        let order_id: u64 = shared_contracts::safe_increment_persistent(&env, &counter_key);

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

        let order_key = DataKey::ImagingOrder(order_id);
        env.storage().persistent().set(&order_key, &order);

        // Paged patient order index
        let p = patient_id.clone();
        pagination::push_paged(
            &env,
            |page| DataKey::PatientOrdersPage(p.clone(), page),
            || DataKey::PatientOrdersHead(p.clone()),
            order_id,
        );
        let pt_key = DataKey::PatientOrdersTotal(patient_id.clone());
        let pt: u32 = env.storage().persistent().get(&pt_key).unwrap_or(0);
        env.storage().persistent().set(&pt_key, &(pt + 1));

        // Paged provider order index
        let prov = provider_id.clone();
        pagination::push_paged(
            &env,
            |page| DataKey::ProviderOrdersPage(prov.clone(), page),
            || DataKey::ProviderOrdersHead(prov.clone()),
            order_id,
        );
        let pv_key = DataKey::ProviderOrdersTotal(provider_id.clone());
        let pv: u32 = env.storage().persistent().get(&pv_key).unwrap_or(0);
        env.storage().persistent().set(&pv_key, &(pv + 1));

        ImagingOrdered {
            version: shared::events::EVENT_VERSION,
            order_id,
            provider_id,
        }
        .publish(&env);

        Ok(order_id)
    }

    /// Schedule an imaging study.
    ///
    /// `scheduled_time` must be strictly in the future: imaging cannot be
    /// scheduled for a time that has already passed.
    pub fn schedule_imaging(
        env: Env,
        order_id: u64,
        imaging_center: Address,
        scheduled_time: u64,
        prep_instructions_hash: BytesN<32>,
    ) -> Result<(), Error> {
        imaging_center.require_auth();

        // #215 – scheduling windows must be in the future
        temporal::must_be_future(&env, scheduled_time)
            .map_err(|_| Error::InvalidScheduledTime)?;

        let order_key = DataKey::ImagingOrder(order_id);
        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&order_key)
            .ok_or(Error::OrderNotFound)?;

        let schedule_key = DataKey::ImagingSchedule(order_id);
        if env.storage().persistent().has(&schedule_key) {
            return Err(Error::AlreadyScheduled);
        }

        let schedule = ImagingSchedule {
            order_id,
            imaging_center,
            scheduled_time,
            prep_instructions_hash,
            scheduled_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&schedule_key, &schedule);

        order.status = Symbol::new(&env, "SCHEDULED");
        env.storage().persistent().set(&order_key, &order);

        ImagingScheduled {
            version: shared::events::EVENT_VERSION,
            order_id,
        }
        .publish(&env);

        Ok(())
    }

    /// Upload DICOM images for a study.
    ///
    /// `study_date` must not be in the future: images are uploaded after the
    /// study is performed.
    pub fn upload_images(
        env: Env,
        order_id: u64,
        imaging_center: Address,
        dicom_hash: BytesN<32>,
        image_count: u32,
        study_date: u64,
    ) -> Result<(), Error> {
        imaging_center.require_auth();

        // #215 – study_date must be a past or present timestamp
        temporal::not_future(&env, study_date)
            .map_err(|_| Error::InvalidStudyDate)?;

        let order_key = DataKey::ImagingOrder(order_id);
        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&order_key)
            .ok_or(Error::OrderNotFound)?;

        let images_key = DataKey::DicomImages(order_id);
        if env.storage().persistent().has(&images_key) {
            return Err(Error::ImagesAlreadyUploaded);
        }

        let images = DicomImages {
            order_id,
            imaging_center,
            dicom_hash: dicom_hash.clone(),
            image_count,
            study_date,
            uploaded_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&images_key, &images);

        order.status = Symbol::new(&env, "IN_PROGRESS");
        env.storage().persistent().set(&order_key, &order);

        ImagesUploaded {
            version: shared::events::EVENT_VERSION,
            order_id,
            dicom_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Submit preliminary report
    pub fn submit_preliminary_report(
        env: Env,
        order_id: u64,
        radiologist_id: Address,
        report_hash: BytesN<32>,
        urgent_findings: bool,
    ) -> Result<(), Error> {
        radiologist_id.require_auth();

        let order_key = DataKey::ImagingOrder(order_id);
        env.storage()
            .persistent()
            .get::<_, ImagingOrder>(&order_key)
            .ok_or(Error::OrderNotFound)?;

        let images_key = DataKey::DicomImages(order_id);
        if !env.storage().persistent().has(&images_key) {
            return Err(Error::InvalidStatus);
        }

        let prelim_key = DataKey::PreliminaryReport(order_id);
        if env.storage().persistent().has(&prelim_key) {
            return Err(Error::PreliminaryReportExists);
        }

        let report = PreliminaryReport {
            order_id,
            radiologist_id,
            report_hash,
            urgent_findings,
            submitted_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&prelim_key, &report);

        PreliminaryReportSubmitted {
            version: shared::events::EVENT_VERSION,
            order_id,
            urgent_findings,
        }
        .publish(&env);

        Ok(())
    }

    /// Submit final report
    pub fn submit_final_report(
        env: Env,
        order_id: u64,
        radiologist_id: Address,
        final_report_hash: BytesN<32>,
        impression: String,
    ) -> Result<(), Error> {
        radiologist_id.require_auth();

        let order_key = DataKey::ImagingOrder(order_id);
        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&order_key)
            .ok_or(Error::OrderNotFound)?;

        let images_key = DataKey::DicomImages(order_id);
        if !env.storage().persistent().has(&images_key) {
            return Err(Error::InvalidStatus);
        }

        let final_key = DataKey::FinalReport(order_id);
        if env.storage().persistent().has(&final_key) {
            return Err(Error::FinalReportExists);
        }

        let report = FinalReport {
            order_id,
            radiologist_id,
            final_report_hash,
            impression,
            submitted_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&final_key, &report);

        order.status = Symbol::new(&env, "COMPLETED");
        env.storage().persistent().set(&order_key, &order);

        FinalReportSubmitted {
            version: shared::events::EVENT_VERSION,
            order_id,
        }
        .publish(&env);

        Ok(())
    }

    /// Request peer review
    pub fn request_peer_review(
        env: Env,
        order_id: u64,
        requesting_radiologist: Address,
        peer_radiologist: Address,
    ) -> Result<(), Error> {
        requesting_radiologist.require_auth();

        let order_key = DataKey::ImagingOrder(order_id);
        env.storage()
            .persistent()
            .get::<_, ImagingOrder>(&order_key)
            .ok_or(Error::OrderNotFound)?;

        let peer_key = DataKey::PeerReview(order_id);
        if env.storage().persistent().has(&peer_key) {
            return Err(Error::PeerReviewExists);
        }

        let peer_review = PeerReview {
            order_id,
            requesting_radiologist,
            peer_radiologist,
            requested_at: env.ledger().timestamp(),
            status: Symbol::new(&env, "PENDING"),
        };

        env.storage().persistent().set(&peer_key, &peer_review);

        PeerReviewRequested {
            version: shared::events::EVENT_VERSION,
            order_id,
        }
        .publish(&env);

        Ok(())
    }

    /// Get imaging order details
    pub fn get_imaging_order(
        env: Env,
        order_id: u64,
        requester: Address,
    ) -> Result<Option<ImagingOrder>, Error> {
        let key = DataKey::ImagingOrder(order_id);
        let order = env.storage().persistent().get::<_, ImagingOrder>(&key);
        if let Some(ref existing) = order {
            Self::require_order_read_access(&env, existing, &requester)?;
        } else {
            requester.require_auth();
        }
        Ok(order)
    }

    /// Get imaging schedule
    pub fn get_imaging_schedule(
        env: Env,
        order_id: u64,
        requester: Address,
    ) -> Result<Option<ImagingSchedule>, Error> {
        Self::load_order_for_read(&env, order_id, &requester)?;
        let key = DataKey::ImagingSchedule(order_id);
        Ok(env.storage().persistent().get(&key))
    }

    /// Get DICOM images reference
    pub fn get_dicom_images(
        env: Env,
        order_id: u64,
        requester: Address,
    ) -> Result<Option<DicomImages>, Error> {
        Self::load_order_for_read(&env, order_id, &requester)?;
        let key = DataKey::DicomImages(order_id);
        Ok(env.storage().persistent().get(&key))
    }

    /// Get preliminary report
    pub fn get_preliminary_report(
        env: Env,
        order_id: u64,
        requester: Address,
    ) -> Result<Option<PreliminaryReport>, Error> {
        Self::load_order_for_read(&env, order_id, &requester)?;
        let key = DataKey::PreliminaryReport(order_id);
        Ok(env.storage().persistent().get(&key))
    }

    /// Get final report
    pub fn get_final_report(
        env: Env,
        order_id: u64,
        requester: Address,
    ) -> Result<Option<FinalReport>, Error> {
        Self::load_order_for_read(&env, order_id, &requester)?;
        let key = DataKey::FinalReport(order_id);
        Ok(env.storage().persistent().get(&key))
    }

    /// Get peer review request
    pub fn get_peer_review(
        env: Env,
        order_id: u64,
        requester: Address,
    ) -> Result<Option<PeerReview>, Error> {
        Self::load_order_for_read(&env, order_id, &requester)?;
        let key = DataKey::PeerReview(order_id);
        Ok(env.storage().persistent().get(&key))
    }

    /// Get a page of order IDs for a patient.
    ///
    /// Each page contains at most `MAX_PAGE_SIZE` IDs.  Pass the returned
    /// `next_page` value as `page` to retrieve the following page; stop when
    /// `next_page == NO_NEXT_PAGE`.
    pub fn get_patient_orders(
        env: Env,
        patient_id: Address,
        requester: Address,
        page: u32,
    ) -> Result<PageResult, Error> {
        requester.require_auth();
        if requester != patient_id {
            return Err(Error::UnauthorizedAccess);
        }
        let p = patient_id.clone();
        let total_key = DataKey::PatientOrdersTotal(patient_id.clone());
        let total: u32 = env
            .storage()
            .persistent()
            .get(&total_key)
            .ok_or(Error::CounterUnavailable)?;
        Ok(pagination::get_paged(
            &env,
            |pg| DataKey::PatientOrdersPage(p.clone(), pg),
            || DataKey::PatientOrdersHead(p.clone()),
            || total,
            page,
        ))
    }

    /// Get a page of order IDs for a provider.
    ///
    /// See `get_patient_orders` for pagination semantics.
    pub fn get_provider_orders(
        env: Env,
        provider_id: Address,
        requester: Address,
        page: u32,
    ) -> Result<PageResult, Error> {
        requester.require_auth();
        if requester != provider_id {
            return Err(Error::UnauthorizedAccess);
        }
        let prov = provider_id.clone();
        let total_key = DataKey::ProviderOrdersTotal(provider_id.clone());
        let total: u32 = env
            .storage()
            .persistent()
            .get(&total_key)
            .ok_or(Error::CounterUnavailable)?;
        Ok(pagination::get_paged(
            &env,
            |pg| DataKey::ProviderOrdersPage(prov.clone(), pg),
            || DataKey::ProviderOrdersHead(prov.clone()),
            || total,
            page,
        ))
    }

    /// Maximum items per page (re-exported for callers).
    pub fn max_page_size(_env: Env) -> u32 {
        MAX_PAGE_SIZE
    }

    fn load_order_for_read(
        env: &Env,
        order_id: u64,
        requester: &Address,
    ) -> Result<ImagingOrder, Error> {
        let order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&DataKey::ImagingOrder(order_id))
            .ok_or(Error::OrderNotFound)?;
        Self::require_order_read_access(env, &order, requester)?;
        Ok(order)
    }

    fn require_order_read_access(
        _env: &Env,
        order: &ImagingOrder,
        requester: &Address,
    ) -> Result<(), Error> {
        requester.require_auth();
        if *requester == order.patient_id || *requester == order.provider_id {
            return Ok(());
        }
        Err(Error::UnauthorizedAccess)
    }
}

#[cfg(test)]
mod test;
#[cfg(test)]
mod cid_fuzz_tests;
