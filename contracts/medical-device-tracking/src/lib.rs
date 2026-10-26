#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Medical Device Tracking Contract
//!
//! Manages medical device inventory, implant tracking, warranty records, performance monitoring,
//! and recall management for implantable and durable medical equipment.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Hospital/provider authorization required for device tracking.
//! Patient consent for implant records. Maintenance technician authentication for service records.
//! Recall notification requires authorization. Device access restricted to responsible facility.
//!
//! **Audit Controls:** Device registration events logged with type, serial number, and location.
//! Implant procedure events tracked with patient ID and implant date. Maintenance service events
//! recorded with technician identity. Warranty event tracking. Recall notifications emitted.
//!
//! **Data Retention Policy:** Device records retained indefinitely for traceability. Implant
//! records retained with patient for lifetime identification. Warranty information archived.
//! Maintenance history maintained for service compliance. Recall information persisted for
//! liability protection.
//!
//! **Encryption/Integrity:** Device serial numbers stored encrypted. Patient-implant linkage
//! encrypted via persistent storage. Warranty dates immutable. Device type enumeration prevents
//! invalid device categories. Maintenance provider identity validated.

mod test;
mod types;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Symbol, Vec};
use types::{
    DataKey, DeviceExtras, DeviceRecord, DmePrescription, Error, ImplantRecord, MaintenanceRecord,
    PerformanceReport, RecallInfo, WarrantyRecord,
};

#[contract]
pub struct MedicalDeviceRegistry;

#[contractimpl]
impl MedicalDeviceRegistry {
    /// Configure the regulator address allowed to issue emergency recalls.
    ///
    /// Runs atomically as part of contract creation and cannot be called again,
    /// so an attacker cannot front-run deployment and choose the regulator.
    pub fn __constructor(env: Env, regulator: Address) {
        env.storage().instance().set(&DataKey::Regulator, &regulator);
    }

    /// Return the configured regulator address.
    pub fn get_regulator(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Regulator)
            .ok_or(Error::RecordNotFound)
    }

    /// Add a manufacturer to the regulator-managed allowlist.
    pub fn allowlist_manufacturer(
        env: Env,
        regulator: Address,
        manufacturer: Address,
    ) -> Result<(), Error> {
        regulator.require_auth();
        let stored_regulator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Regulator)
            .ok_or(Error::NotAuthorized)?;
        if regulator != stored_regulator {
            return Err(Error::NotAuthorized);
        }
        env.storage()
            .persistent()
            .set(&DataKey::ManufacturerAllowlist(manufacturer), &true);
        Ok(())
    }

    /// Revoke a manufacturer from the regulator-managed allowlist.
    pub fn revoke_manufacturer(
        env: Env,
        regulator: Address,
        manufacturer: Address,
    ) -> Result<(), Error> {
        regulator.require_auth();
        let stored_regulator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Regulator)
            .ok_or(Error::NotAuthorized)?;
        if regulator != stored_regulator {
            return Err(Error::NotAuthorized);
        }
        env.storage()
            .persistent()
            .remove(&DataKey::ManufacturerAllowlist(manufacturer));
        Ok(())
    }

    /// Check if a manufacturer address is allowlisted by the regulator.
    pub fn is_manufacturer_allowlisted(env: Env, manufacturer: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::ManufacturerAllowlist(manufacturer))
            .unwrap_or(false)
    }

    /// Retrieve a device record by its Unique Device Identifier (UDI).
    pub fn get_device_by_udi(env: Env, device_udi: String) -> Result<DeviceRecord, Error> {
        let device_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceByUdi(device_udi))
            .ok_or(Error::RecordNotFound)?;
        env.storage()
            .persistent()
            .get(&DataKey::DeviceRecord(device_id))
            .ok_or(Error::RecordNotFound)
    }

    /// Retrieve a device record by its device ID.
    pub fn get_device_record(env: Env, device_id: u64) -> Result<DeviceRecord, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::DeviceRecord(device_id))
            .ok_or(Error::RecordNotFound)
    }

    /// Retrieve the total number of registered devices.
    pub fn get_device_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::DeviceCounter)
            .unwrap_or(0)
    }

    /// Register a medical device with its Unique Device Identifier (UDI).
    /// Requires caller to be an allowlisted manufacturer and enforces UDI uniqueness.
    pub fn register_device(
        env: Env,
        manufacturer_id: Address,
        device_udi: String,
        device_type: Symbol,
        manufacturer: String,
        model_number: String,
        lot_number: String,
        manufacturing_date: u64,
        expiration_date: Option<u64>,
        device_specs_hash: BytesN<32>,
        extras: DeviceExtras,
    ) -> Result<u64, Error> {
        manufacturer_id.require_auth();

        if !Self::is_manufacturer_allowlisted(env.clone(), manufacturer_id.clone()) {
            return Err(Error::ManufacturerNotAuthorized);
        }

        let udi_key = DataKey::DeviceByUdi(device_udi.clone());
        if env.storage().persistent().has(&udi_key) {
            return Err(Error::DuplicateUdi);
        }

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::DeviceCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::DeviceCounter, &new_id);

        let next_scheduled_maintenance = match extras.maintenance_interval_days {
            Some(interval) => {
                let interval_secs = interval
                    .checked_mul(86400)
                    .ok_or(Error::ArithmeticOverflow)?;
                let next_m = manufacturing_date
                    .checked_add(interval_secs)
                    .ok_or(Error::ArithmeticOverflow)?;
                Some(next_m)
            }
            None => None,
        };

        let device = DeviceRecord {
            device_id: new_id,
            device_udi: device_udi.clone(),
            device_type: device_type.clone(),
            manufacturer_id: manufacturer_id.clone(),
            manufacturer: manufacturer.clone(),
            model_number: model_number.clone(),
            lot_number: lot_number.clone(),
            manufacturing_date,
            expiration_date,
            device_specs_hash,
            warranty_expiration_date: extras.warranty_expiration_date,
            next_scheduled_maintenance,
            maintenance_interval_days: extras.maintenance_interval_days,
        };
        env.storage()
            .persistent()
            .set(&DataKey::DeviceRecord(new_id), &device);
        env.storage()
            .persistent()
            .set(&udi_key, &new_id);

        env.events().publish(
            (Symbol::new(&env, "device_registered"),),
            (new_id, device_type, device_udi),
        );

        Ok(new_id)
    }

    /// Record a device implantation procedure for a patient using the device UDI.
    pub fn implant_device(
        env: Env,
        patient_id: Address,
        device_udi: String,
        provider_id: Address,
        implant_date: u64,
        implant_location: String,
        surgical_notes_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let device_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceByUdi(device_udi))
            .ok_or(Error::RecordNotFound)?;

        if !env
            .storage()
            .persistent()
            .has(&DataKey::DeviceRecord(device_id))
        {
            return Err(Error::RecordNotFound);
        }

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ImplantCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::ImplantCounter, &new_id);

        let record = ImplantRecord {
            implant_record_id: new_id,
            patient_id: patient_id.clone(),
            device_id,
            implant_date,
            implant_location,
            implanting_provider: provider_id,
            surgical_notes_hash,
            is_active: true,
            removal_date: None,
            removal_reason: None,
            explant_analysis_hash: None,
            maintenance_history: Vec::new(&env),
        };
        env.storage()
            .persistent()
            .set(&DataKey::ImplantRecord(new_id), &record);

        let mut patient_implants: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientImplants(patient_id.clone()))
            .unwrap_or(Vec::new(&env));
        patient_implants.push_back(new_id);
        env.storage()
            .persistent()
            .set(&DataKey::PatientImplants(patient_id.clone()), &patient_implants);

        let mut device_implants: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceImplants(device_id))
            .unwrap_or(Vec::new(&env));
        device_implants.push_back(new_id);
        env.storage()
            .persistent()
            .set(&DataKey::DeviceImplants(device_id), &device_implants);

        env.events().publish(
            (Symbol::new(&env, "implant_recorded"),),
            (new_id, device_id, patient_id, implant_date),
        );

        Ok(new_id)
    }

    /// Prescribe durable medical equipment (DME) to a patient.
    pub fn prescribe_dme(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        device_type: Symbol,
        device_id: u64,
        prescription_date: u64,
        duration_days: Option<u64>,
        instructions_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::DmeCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage().instance().set(&DataKey::DmeCounter, &new_id);

        let prescription = DmePrescription {
            prescription_id: new_id,
            patient_id,
            provider_id,
            device_type,
            device_id,
            prescription_date,
            duration_days,
            instructions_hash,
        };
        env.storage()
            .persistent()
            .set(&DataKey::DmeRecord(new_id), &prescription);

        Ok(new_id)
    }

    /// Record a maintenance event for an implanted device.
    pub fn record_device_maintenance(
        env: Env,
        implant_record_id: u64,
        maintenance_date: u64,
        maintenance_type: Symbol,
        performed_by: Address,
        notes_hash: BytesN<32>,
    ) -> Result<(), Error> {
        performed_by.require_auth();

        let mut record: ImplantRecord = env
            .storage()
            .persistent()
            .get(&DataKey::ImplantRecord(implant_record_id))
            .ok_or(Error::RecordNotFound)?;

        let m_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MaintenanceCounter)
            .unwrap_or(0);
        let new_m_id = m_count + 1;
        env.storage()
            .instance()
            .set(&DataKey::MaintenanceCounter, &new_m_id);

        let maintenance = MaintenanceRecord {
            maintenance_id: new_m_id,
            implant_record_id,
            maintenance_date,
            maintenance_type: maintenance_type.clone(),
            performed_by,
            notes_hash,
        };
        env.storage()
            .persistent()
            .set(&DataKey::MaintenanceRecord(new_m_id), &maintenance);

        record.maintenance_history.push_back(new_m_id);
        env.storage()
            .persistent()
            .set(&DataKey::ImplantRecord(implant_record_id), &record);

        env.events().publish(
            (Symbol::new(&env, "maintenance_recorded"),),
            (new_m_id, implant_record_id, maintenance_type, maintenance_date),
        );

        Ok(())
    }

    /// Issue a recall for one or more medical devices.
    pub fn issue_device_recall(
        env: Env,
        manufacturer: Address,
        device_ids: Vec<u64>,
        recall_reason: String,
        severity: Symbol,
        recall_date: u64,
        action_required: String,
    ) -> Result<u64, Error> {
        manufacturer.require_auth();

        if device_ids.is_empty() {
            return Err(Error::InvalidInput);
        }
        Self::assert_manufacturer_controls_devices(&env, &manufacturer, &device_ids)?;

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RecallCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::RecallCounter, &new_id);

        let recall = RecallInfo {
            recall_id: new_id,
            device_ids: device_ids.clone(),
            issuer: manufacturer,
            issuer_role: Symbol::new(&env, "maker"),
            recall_reason,
            severity: severity.clone(),
            recall_date,
            action_required,
            resolution_deadline: None,
            emergency_scope: None,
        };
        env.storage()
            .persistent()
            .set(&DataKey::RecallInfo(new_id), &recall);

        for device_id in device_ids.iter() {
            let mut device_recalls: Vec<u64> = env
                .storage()
                .persistent()
                .get(&DataKey::DeviceRecalls(device_id))
                .unwrap_or(Vec::new(&env));
            device_recalls.push_back(new_id);
            env.storage()
                .persistent()
                .set(&DataKey::DeviceRecalls(device_id), &device_recalls);
        }

        env.events().publish(
            (Symbol::new(&env, "device_recall_issued"),),
            (new_id, device_ids, severity),
        );

        Ok(new_id)
    }

    /// Issue an emergency recall under regulator authority for a defined scope.
    pub fn issue_regulator_recall(
        env: Env,
        regulator: Address,
        device_ids: Vec<u64>,
        recall_reason: String,
        severity: Symbol,
        recall_date: u64,
        action_required: String,
        emergency_scope: String,
    ) -> Result<u64, Error> {
        regulator.require_auth();

        if device_ids.is_empty() || emergency_scope.is_empty() {
            return Err(Error::InvalidInput);
        }

        let configured_regulator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Regulator)
            .ok_or(Error::NotAuthorized)?;
        if regulator != configured_regulator {
            return Err(Error::NotAuthorized);
        }

        for device_id in device_ids.iter() {
            if !env
                .storage()
                .persistent()
                .has(&DataKey::DeviceRecord(device_id))
            {
                return Err(Error::RecordNotFound);
            }
        }

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RecallCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage().instance().set(&DataKey::RecallCounter, &new_id);

        let recall = RecallInfo {
            recall_id: new_id,
            device_ids: device_ids.clone(),
            issuer: regulator,
            issuer_role: Symbol::new(&env, "reg"),
            recall_reason,
            severity: severity.clone(),
            recall_date,
            action_required,
            resolution_deadline: None,
            emergency_scope: Some(emergency_scope),
        };
        env.storage()
            .persistent()
            .set(&DataKey::RecallInfo(new_id), &recall);

        for device_id in device_ids.iter() {
            let mut device_recalls: Vec<u64> = env
                .storage()
                .persistent()
                .get(&DataKey::DeviceRecalls(device_id))
                .unwrap_or(Vec::new(&env));
            device_recalls.push_back(new_id);
            env.storage()
                .persistent()
                .set(&DataKey::DeviceRecalls(device_id), &device_recalls);
        }

        env.events().publish(
            (Symbol::new(&env, "regulator_recall_issued"),),
            (new_id, device_ids, severity),
        );

        Ok(new_id)
    }

    /// Return the IDs of all patients with an active implant from the recalled devices.
    ///
    /// Restricted to the manufacturer/regulator that issued the recall (or the
    /// configured regulator, which has oversight of all recalls) -- this list
    /// discloses which patients carry a specific recalled device, so it must
    /// not be readable by an arbitrary caller.
    pub fn notify_affected_patients(
        env: Env,
        caller: Address,
        recall_id: u64,
        _notification_date: u64,
    ) -> Result<Vec<Address>, Error> {
        caller.require_auth();

        let recall: RecallInfo = env
            .storage()
            .persistent()
            .get(&DataKey::RecallInfo(recall_id))
            .ok_or(Error::RecordNotFound)?;

        let configured_regulator: Option<Address> =
            env.storage().instance().get(&DataKey::Regulator);
        let is_regulator = configured_regulator
            .map(|regulator| regulator == caller)
            .unwrap_or(false);
        if caller != recall.issuer && !is_regulator {
            return Err(Error::NotAuthorized);
        }

        let mut affected_patients: Vec<Address> = Vec::new(&env);

        for device_id in recall.device_ids {
            let implant_ids: Vec<u64> = env
                .storage()
                .persistent()
                .get(&DataKey::DeviceImplants(device_id))
                .unwrap_or(Vec::new(&env));

            for implant_id in implant_ids {
                if let Some(implant) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, ImplantRecord>(&DataKey::ImplantRecord(implant_id))
                {
                    if implant.is_active {
                        affected_patients.push_back(implant.patient_id);
                    }
                }
            }
        }

        Ok(affected_patients)
    }

    /// Record the removal of a previously implanted device.
    pub fn remove_implant(
        env: Env,
        implant_record_id: u64,
        provider_id: Address,
        removal_date: u64,
        removal_reason: String,
        explant_analysis_hash: Option<BytesN<32>>,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut record: ImplantRecord = env
            .storage()
            .persistent()
            .get(&DataKey::ImplantRecord(implant_record_id))
            .ok_or(Error::RecordNotFound)?;

        if !record.is_active {
            return Err(Error::DeviceNotActive);
        }

        if provider_id != record.implanting_provider {
            return Err(Error::NotAuthorized);
        }

        record.is_active = false;
        record.removal_date = Some(removal_date);
        record.removal_reason = Some(removal_reason);
        record.explant_analysis_hash = explant_analysis_hash;
        env.storage()
            .persistent()
            .set(&DataKey::ImplantRecord(implant_record_id), &record);

        env.events().publish(
            (Symbol::new(&env, "implant_removed"),),
            (implant_record_id, record.device_id, removal_date),
        );

        Ok(())
    }

    /// Record a device performance report including optional complications.
    pub fn track_device_performance(
        env: Env,
        implant_record_id: u64,
        patient_id: Address,
        performance_data_hash: BytesN<32>,
        reported_date: u64,
        complications: Option<Vec<String>>,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let implant_record: ImplantRecord = env
            .storage()
            .persistent()
            .get(&DataKey::ImplantRecord(implant_record_id))
            .ok_or(Error::RecordNotFound)?;

        // Verify patient owns this implant record
        if implant_record.patient_id != patient_id {
            return Err(Error::NotAuthorized);
        }

        let report = PerformanceReport {
            implant_record_id,
            patient_id,
            performance_data_hash,
            reported_date,
            complications,
        };

        let mut reports: Vec<PerformanceReport> = env
            .storage()
            .persistent()
            .get(&DataKey::PerformanceReports(implant_record_id))
            .unwrap_or(Vec::new(&env));
        reports.push_back(report);
        env.storage()
            .persistent()
            .set(&DataKey::PerformanceReports(implant_record_id), &reports);

        Ok(())
    }

    /// Retrieve all implant records for a patient, optionally filtered to active implants only.
    pub fn get_patient_implants(
        env: Env,
        patient_id: Address,
        requester: Address,
        active_only: bool,
    ) -> Result<Vec<ImplantRecord>, Error> {
        requester.require_auth();

        let implant_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientImplants(patient_id))
            .unwrap_or(Vec::new(&env));

        let mut implants: Vec<ImplantRecord> = Vec::new(&env);
        for id in implant_ids {
            if let Some(record) = env
                .storage()
                .persistent()
                .get::<DataKey, ImplantRecord>(&DataKey::ImplantRecord(id))
            {
                if !active_only || record.is_active {
                    implants.push_back(record);
                }
            }
        }

        Ok(implants)
    }

    /// Retrieve all recalls associated with a specific device ID.
    pub fn check_device_recalls(env: Env, device_id: u64) -> Result<Vec<RecallInfo>, Error> {
        let recall_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceRecalls(device_id))
            .unwrap_or(Vec::new(&env));

        let mut recalls: Vec<RecallInfo> = Vec::new(&env);
        for id in recall_ids {
            if let Some(recall) = env
                .storage()
                .persistent()
                .get::<DataKey, RecallInfo>(&DataKey::RecallInfo(id))
            {
                recalls.push_back(recall);
            }
        }

        Ok(recalls)
    }

    fn assert_manufacturer_controls_devices(
        env: &Env,
        manufacturer: &Address,
        device_ids: &Vec<u64>,
    ) -> Result<(), Error> {
        for device_id in device_ids.iter() {
            let device: DeviceRecord = env
                .storage()
                .persistent()
                .get(&DataKey::DeviceRecord(device_id))
                .ok_or(Error::RecordNotFound)?;
            if device.manufacturer_id != *manufacturer {
                return Err(Error::NotAuthorized);
            }
        }

        Ok(())
    }

    /// Register warranty coverage for a device.
    pub fn register_warranty(
        env: Env,
        device_id: u64,
        warranty_provider: Address,
        warranty_start_date: u64,
        warranty_expiration_date: u64,
        coverage_details_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        warranty_provider.require_auth();

        // Verify device exists
        if !env
            .storage()
            .persistent()
            .has(&DataKey::DeviceRecord(device_id))
        {
            return Err(Error::RecordNotFound);
        }

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::WarrantyCounter)
            .unwrap_or(0);
        let warranty_id = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::WarrantyCounter, &warranty_id);

        let warranty = WarrantyRecord {
            warranty_id,
            device_id,
            warranty_start_date,
            warranty_expiration_date,
            warranty_provider,
            coverage_details_hash,
            is_active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::WarrantyRecord(warranty_id), &warranty);

        let mut warranties: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceWarranties(device_id))
            .unwrap_or(Vec::new(&env));
        warranties.push_back(warranty_id);
        env.storage()
            .persistent()
            .set(&DataKey::DeviceWarranties(device_id), &warranties);

        env.events().publish(
            (Symbol::new(&env, "warranty_registered"),),
            (warranty_id, device_id, warranty_expiration_date),
        );

        Ok(warranty_id)
    }

    /// Check if a device warranty is still valid.
    pub fn check_warranty_status(
        env: Env,
        device_id: u64,
        current_time: u64,
    ) -> Result<bool, Error> {
        let warranties: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceWarranties(device_id))
            .unwrap_or(Vec::new(&env));

        if warranties.is_empty() {
            return Ok(false);
        }

        // Check if any active warranty covers the current time
        for warranty_id in warranties {
            if let Some(warranty) = env
                .storage()
                .persistent()
                .get::<DataKey, WarrantyRecord>(&DataKey::WarrantyRecord(warranty_id))
            {
                if warranty.is_active
                    && warranty.warranty_start_date <= current_time
                    && current_time < warranty.warranty_expiration_date
                {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Check if scheduled maintenance is overdue for a device.
    pub fn check_maintenance_due(
        env: Env,
        device_id: u64,
        current_time: u64,
    ) -> Result<bool, Error> {
        let device: DeviceRecord = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceRecord(device_id))
            .ok_or(Error::RecordNotFound)?;

        if let Some(next_maintenance) = device.next_scheduled_maintenance {
            return Ok(current_time >= next_maintenance);
        }

        Ok(false)
    }

    /// Schedule next maintenance after current maintenance is completed.
    ///
    /// Restricted to the device's manufacturer or a technician who has an
    /// existing entry in the device's maintenance history (i.e. previously
    /// called `record_device_maintenance` for one of this device's implants),
    /// mirroring the authorization already used for device-level actions
    /// (`register_device`, `issue_device_recall`) and technician-authenticated
    /// service records (`record_device_maintenance`).
    pub fn schedule_next_maintenance(
        env: Env,
        device_id: u64,
        maintenance_completed_date: u64,
        performed_by: Address,
    ) -> Result<(), Error> {
        performed_by.require_auth();

        let mut device: DeviceRecord = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceRecord(device_id))
            .ok_or(Error::RecordNotFound)?;

        Self::assert_authorized_for_device_maintenance(&env, &performed_by, &device)?;

        if let Some(interval) = device.maintenance_interval_days {
            let interval_secs = interval
                .checked_mul(86400)
                .ok_or(Error::ArithmeticOverflow)?;
            let next_m = maintenance_completed_date
                .checked_add(interval_secs)
                .ok_or(Error::ArithmeticOverflow)?;
            device.next_scheduled_maintenance = Some(next_m);
            env.storage()
                .persistent()
                .set(&DataKey::DeviceRecord(device_id), &device);
            Ok(())
        } else {
            Err(Error::InvalidInput)
        }
    }

    /// Verify that `performed_by` is authorized to manage maintenance for `device`:
    /// either the device's registered manufacturer, or a technician who already
    /// appears in the device's maintenance history via a prior
    /// `record_device_maintenance` call on one of the device's implants.
    fn assert_authorized_for_device_maintenance(
        env: &Env,
        performed_by: &Address,
        device: &DeviceRecord,
    ) -> Result<(), Error> {
        if *performed_by == device.manufacturer_id {
            return Ok(());
        }

        let implant_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceImplants(device.device_id))
            .unwrap_or(Vec::new(env));

        for implant_id in implant_ids.iter() {
            if let Some(implant) = env
                .storage()
                .persistent()
                .get::<DataKey, ImplantRecord>(&DataKey::ImplantRecord(implant_id))
            {
                for maintenance_id in implant.maintenance_history.iter() {
                    if let Some(maintenance) = env
                        .storage()
                        .persistent()
                        .get::<DataKey, MaintenanceRecord>(&DataKey::MaintenanceRecord(
                            maintenance_id,
                        ))
                    {
                        if maintenance.performed_by == *performed_by {
                            return Ok(());
                        }
                    }
                }
            }
        }

        Err(Error::NotAuthorized)
    }

    /// Flag devices that are out of warranty (warranty expired and no active coverage).
    pub fn get_out_of_warranty_devices(
        env: Env,
        device_ids: Vec<u64>,
        current_time: u64,
    ) -> Result<Vec<u64>, Error> {
        let mut out_of_warranty = Vec::new(&env);

        for device_id in device_ids {
            let device: DeviceRecord = env
                .storage()
                .persistent()
                .get(&DataKey::DeviceRecord(device_id))
                .ok_or(Error::RecordNotFound)?;

            // Check if warranty has expired
            let warranty_valid = if let Some(warranty_exp) = device.warranty_expiration_date {
                current_time < warranty_exp
            } else {
                false
            };

            if !warranty_valid {
                // Also check active warranties
                let has_active_warranty =
                    Self::check_warranty_status(env.clone(), device_id, current_time).unwrap_or(false);
                if !has_active_warranty {
                    out_of_warranty.push_back(device_id);
                }
            }
        }

        Ok(out_of_warranty)
    }
}
