#![no_std]
#![allow(clippy::too_many_arguments)]

mod test;
mod types;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Symbol, Vec};
use types::{
    DataKey, DeviceRecord, DmePrescription, Error, ImplantRecord, MaintenanceRecord,
    PerformanceReport, RecallInfo, WarrantyRecord,
};

#[contract]
pub struct MedicalDeviceRegistry;

#[contractimpl]
impl MedicalDeviceRegistry {
    /// Configure the regulator address allowed to issue emergency recalls.
    pub fn initialize(env: Env, regulator: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Regulator) {
            return Err(Error::AlreadyInitialized);
        }

        regulator.require_auth();
        env.storage().instance().set(&DataKey::Regulator, &regulator);
        Ok(())
    }

    /// Register a medical device with its Unique Device Identifier (UDI).
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
        warranty_expiration_date: Option<u64>,
        maintenance_interval_days: Option<u64>,
    ) -> Result<u64, Error> {
        manufacturer_id.require_auth();

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::DeviceCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::DeviceCounter, &new_id);

        let next_scheduled_maintenance = if let Some(interval) = maintenance_interval_days {
            Some(manufacturing_date + interval * 86400)
        } else {
            None
        };

        let device = DeviceRecord {
            device_id: new_id,
            device_udi,
            device_type,
            manufacturer_id,
            manufacturer,
            model_number,
            lot_number,
            manufacturing_date,
            expiration_date,
            device_specs_hash,
            warranty_expiration_date,
            next_scheduled_maintenance,
            maintenance_interval_days,
        };
        env.storage()
            .persistent()
            .set(&DataKey::DeviceRecord(new_id), &device);

        Ok(new_id)
    }

    /// Record a device implantation procedure for a patient.
    pub fn implant_device(
        env: Env,
        patient_id: Address,
        device_id: u64,
        provider_id: Address,
        implant_date: u64,
        implant_location: String,
        surgical_notes_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

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
            .set(&DataKey::PatientImplants(patient_id), &patient_implants);

        let mut device_implants: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceImplants(device_id))
            .unwrap_or(Vec::new(&env));
        device_implants.push_back(new_id);
        env.storage()
            .persistent()
            .set(&DataKey::DeviceImplants(device_id), &device_implants);

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
            maintenance_type,
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
            severity,
            recall_date,
            action_required,
            resolution_deadline: None,
            emergency_scope: None,
        };
        env.storage()
            .persistent()
            .set(&DataKey::RecallInfo(new_id), &recall);

        for device_id in device_ids {
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
            severity,
            recall_date,
            action_required,
            resolution_deadline: None,
            emergency_scope: Some(emergency_scope),
        };
        env.storage()
            .persistent()
            .set(&DataKey::RecallInfo(new_id), &recall);

        for device_id in device_ids {
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

        Ok(new_id)
    }

    /// Return the IDs of all patients with an active implant from the recalled devices.
    pub fn notify_affected_patients(
        env: Env,
        recall_id: u64,
        _notification_date: u64,
    ) -> Result<Vec<Address>, Error> {
        let recall: RecallInfo = env
            .storage()
            .persistent()
            .get(&DataKey::RecallInfo(recall_id))
            .ok_or(Error::RecordNotFound)?;

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

        record.is_active = false;
        record.removal_date = Some(removal_date);
        record.removal_reason = Some(removal_reason);
        record.explant_analysis_hash = explant_analysis_hash;
        env.storage()
            .persistent()
            .set(&DataKey::ImplantRecord(implant_record_id), &record);

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

        if !env
            .storage()
            .persistent()
            .has(&DataKey::ImplantRecord(implant_record_id))
        {
            return Err(Error::RecordNotFound);
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
    pub fn schedule_next_maintenance(
        env: Env,
        device_id: u64,
        maintenance_completed_date: u64,
    ) -> Result<(), Error> {
        let mut device: DeviceRecord = env
            .storage()
            .persistent()
            .get(&DataKey::DeviceRecord(device_id))
            .ok_or(Error::RecordNotFound)?;

        if let Some(interval) = device.maintenance_interval_days {
            device.next_scheduled_maintenance =
                Some(maintenance_completed_date + interval * 86400);
            env.storage()
                .persistent()
                .set(&DataKey::DeviceRecord(device_id), &device);
            Ok(())
        } else {
            Err(Error::InvalidInput)
        }
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
                    Self::check_warranty_status(&env, device_id, current_time).unwrap_or(false);
                if !has_active_warranty {
                    out_of_warranty.push_back(device_id);
                }
            }
        }

        Ok(out_of_warranty)
    }
}
