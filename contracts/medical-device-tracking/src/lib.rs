#![no_std]

mod types;
mod test;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Symbol, Vec};
use types::{
    DataKey, DeviceRecord, DmePrescription, Error, ImplantRecord, MaintenanceRecord,
    PerformanceReport, RecallInfo,
};

#[contract]
pub struct MedicalDeviceRegistry;

#[contractimpl]
impl MedicalDeviceRegistry {
    /// Register a medical device with its Unique Device Identifier (UDI).
    pub fn register_device(
        env: Env,
        device_udi: String,
        device_type: Symbol,
        manufacturer: String,
        model_number: String,
        lot_number: String,
        manufacturing_date: u64,
        expiration_date: Option<u64>,
        device_specs_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::DeviceCounter)
            .unwrap_or(0);
        let new_id = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::DeviceCounter, &new_id);

        let device = DeviceRecord {
            device_id: new_id,
            device_udi,
            device_type,
            manufacturer,
            model_number,
            lot_number,
            manufacturing_date,
            expiration_date,
            device_specs_hash,
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
        env.storage()
            .instance()
            .set(&DataKey::DmeCounter, &new_id);

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
            recall_reason,
            severity,
            recall_date,
            action_required,
            resolution_deadline: None,
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
    pub fn check_device_recalls(
        env: Env,
        device_id: u64,
    ) -> Result<Vec<RecallInfo>, Error> {
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
}
