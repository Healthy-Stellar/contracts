#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, String, Symbol, Vec,
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
    PatientOrders(Address),
    ProviderOrders(Address),
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

        // Get next order ID
        let counter_key = DataKey::OrderCounter;
        let order_id: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0) + 1;
        env.storage().persistent().set(&counter_key, &order_id);

        // Create imaging order
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

        // Store order
        let order_key = DataKey::ImagingOrder(order_id);
        env.storage().persistent().set(&order_key, &order);

        // Track patient orders
        let patient_key = DataKey::PatientOrders(patient_id.clone());
        let mut patient_orders: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));
        patient_orders.push_back(order_id);
        env.storage()
            .persistent()
            .set(&patient_key, &patient_orders);

        // Track provider orders
        let provider_key = DataKey::ProviderOrders(provider_id.clone());
        let mut provider_orders: Vec<u64> = env
            .storage()
            .persistent()
            .get(&provider_key)
            .unwrap_or(Vec::new(&env));
        provider_orders.push_back(order_id);
        env.storage()
            .persistent()
            .set(&provider_key, &provider_orders);

        Ok(order_id)
    }

    /// Schedule an imaging study
    pub fn schedule_imaging(
        env: Env,
        order_id: u64,
        imaging_center: Address,
        scheduled_time: u64,
        prep_instructions_hash: BytesN<32>,
    ) -> Result<(), Error> {
        imaging_center.require_auth();

        // Verify order exists
        let order_key = DataKey::ImagingOrder(order_id);
        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&order_key)
            .ok_or(Error::OrderNotFound)?;

        // Check if already scheduled
        let schedule_key = DataKey::ImagingSchedule(order_id);
        if env.storage().persistent().has(&schedule_key) {
            return Err(Error::AlreadyScheduled);
        }

        // Create schedule
        let schedule = ImagingSchedule {
            order_id,
            imaging_center,
            scheduled_time,
            prep_instructions_hash,
            scheduled_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&schedule_key, &schedule);

        // Update order status
        order.status = Symbol::new(&env, "SCHEDULED");
        env.storage().persistent().set(&order_key, &order);

        Ok(())
    }

    /// Upload DICOM images for a study
    pub fn upload_images(
        env: Env,
        order_id: u64,
        imaging_center: Address,
        dicom_hash: BytesN<32>,
        image_count: u32,
        study_date: u64,
    ) -> Result<(), Error> {
        imaging_center.require_auth();

        // Verify order exists
        let order_key = DataKey::ImagingOrder(order_id);
        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&order_key)
            .ok_or(Error::OrderNotFound)?;

        // Check if images already uploaded
        let images_key = DataKey::DicomImages(order_id);
        if env.storage().persistent().has(&images_key) {
            return Err(Error::ImagesAlreadyUploaded);
        }

        // Store DICOM reference
        let images = DicomImages {
            order_id,
            imaging_center,
            dicom_hash,
            image_count,
            study_date,
            uploaded_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&images_key, &images);

        // Update order status
        order.status = Symbol::new(&env, "IN_PROGRESS");
        env.storage().persistent().set(&order_key, &order);

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

        // Verify order exists
        let order_key = DataKey::ImagingOrder(order_id);
        env.storage()
            .persistent()
            .get::<_, ImagingOrder>(&order_key)
            .ok_or(Error::OrderNotFound)?;

        // Verify images uploaded
        let images_key = DataKey::DicomImages(order_id);
        if !env.storage().persistent().has(&images_key) {
            return Err(Error::InvalidStatus);
        }

        // Check if preliminary report already exists
        let prelim_key = DataKey::PreliminaryReport(order_id);
        if env.storage().persistent().has(&prelim_key) {
            return Err(Error::PreliminaryReportExists);
        }

        // Create preliminary report
        let report = PreliminaryReport {
            order_id,
            radiologist_id,
            report_hash,
            urgent_findings,
            submitted_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&prelim_key, &report);

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

        // Verify order exists
        let order_key = DataKey::ImagingOrder(order_id);
        let mut order: ImagingOrder = env
            .storage()
            .persistent()
            .get(&order_key)
            .ok_or(Error::OrderNotFound)?;

        // Verify images uploaded
        let images_key = DataKey::DicomImages(order_id);
        if !env.storage().persistent().has(&images_key) {
            return Err(Error::InvalidStatus);
        }

        // Check if final report already exists
        let final_key = DataKey::FinalReport(order_id);
        if env.storage().persistent().has(&final_key) {
            return Err(Error::FinalReportExists);
        }

        // Create final report
        let report = FinalReport {
            order_id,
            radiologist_id,
            final_report_hash,
            impression,
            submitted_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&final_key, &report);

        // Update order status to completed
        order.status = Symbol::new(&env, "COMPLETED");
        env.storage().persistent().set(&order_key, &order);

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

        // Verify order exists
        let order_key = DataKey::ImagingOrder(order_id);
        env.storage()
            .persistent()
            .get::<_, ImagingOrder>(&order_key)
            .ok_or(Error::OrderNotFound)?;

        // Check if peer review already requested
        let peer_key = DataKey::PeerReview(order_id);
        if env.storage().persistent().has(&peer_key) {
            return Err(Error::PeerReviewExists);
        }

        // Create peer review request
        let peer_review = PeerReview {
            order_id,
            requesting_radiologist,
            peer_radiologist,
            requested_at: env.ledger().timestamp(),
            status: Symbol::new(&env, "PENDING"),
        };

        env.storage().persistent().set(&peer_key, &peer_review);

        Ok(())
    }

    /// Get imaging order details
    pub fn get_imaging_order(env: Env, order_id: u64) -> Option<ImagingOrder> {
        let key = DataKey::ImagingOrder(order_id);
        env.storage().persistent().get(&key)
    }

    /// Get imaging schedule
    pub fn get_imaging_schedule(env: Env, order_id: u64) -> Option<ImagingSchedule> {
        let key = DataKey::ImagingSchedule(order_id);
        env.storage().persistent().get(&key)
    }

    /// Get DICOM images reference
    pub fn get_dicom_images(env: Env, order_id: u64) -> Option<DicomImages> {
        let key = DataKey::DicomImages(order_id);
        env.storage().persistent().get(&key)
    }

    /// Get preliminary report
    pub fn get_preliminary_report(env: Env, order_id: u64) -> Option<PreliminaryReport> {
        let key = DataKey::PreliminaryReport(order_id);
        env.storage().persistent().get(&key)
    }

    /// Get final report
    pub fn get_final_report(env: Env, order_id: u64) -> Option<FinalReport> {
        let key = DataKey::FinalReport(order_id);
        env.storage().persistent().get(&key)
    }

    /// Get peer review request
    pub fn get_peer_review(env: Env, order_id: u64) -> Option<PeerReview> {
        let key = DataKey::PeerReview(order_id);
        env.storage().persistent().get(&key)
    }

    /// Get all orders for a patient
    pub fn get_patient_orders(env: Env, patient_id: Address) -> Vec<u64> {
        let key = DataKey::PatientOrders(patient_id);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env))
    }

    /// Get all orders by a provider
    pub fn get_provider_orders(env: Env, provider_id: Address) -> Vec<u64> {
        let key = DataKey::ProviderOrders(provider_id);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env))
    }
}

#[cfg(test)]
mod test;
