#![no_std]

use shared::incident_tracking::{
    capture_incident, get_incidents_by_correlation_id as shared_get_by_corr, IncidentSeverity,
};
use shared::privacy::{
    validate_encrypted_ref, validate_policy_metadata, EncryptedEnvelopeRef, PolicyMetadata,
};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env,
    String, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedicalRecord {
    pub record_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub encrypted_ref: EncryptedEnvelopeRef,
    pub record_type: String,
    pub timestamp: u64,
    pub integrity_hash: BytesN<32>,
    pub policy: PolicyMetadata,
}

#[contracttype]
pub enum DataKey {
    Record(u64),
    RecordCounter,
    Consent(Address, Address), // (patient, provider) -> bool
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    RecordNotFound = 1,
    Unauthorized = 2,
    ConsentNotGranted = 3,
    InvalidEncryptedEnvelope = 4,
    InvalidPolicyMetadata = 5,
}

fn compute_hash(
    env: &Env,
    record_id: u64,
    patient: &Address,
    provider: &Address,
    encrypted_ref: &EncryptedEnvelopeRef,
    record_type: &String,
    timestamp: u64,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_array(&record_id.to_be_bytes());
    let patient_bytes = patient.clone().to_xdr(env);
    data.append(&patient_bytes);
    let provider_bytes = provider.clone().to_xdr(env);
    data.append(&provider_bytes);
    data.append(&Bytes::from(encrypted_ref.content_hash.clone()));
    let type_bytes = record_type.clone().to_xdr(env);
    data.append(&type_bytes);
    data.extend_from_array(&timestamp.to_be_bytes());
    env.crypto().sha256(&data).into()
}

fn has_consent(env: &Env, patient: &Address, provider: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Consent(patient.clone(), provider.clone()))
        .unwrap_or(false)
}

#[contract]
pub struct HealthRecords;

#[contractimpl]
impl HealthRecords {
    /// Patient grants a provider consent to create/access their records.
    pub fn grant_consent(env: Env, patient: Address, provider: Address) {
        patient.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Consent(patient, provider), &true);
    }

    /// Patient revokes a provider's consent.
    pub fn revoke_consent(env: Env, patient: Address, provider: Address) {
        patient.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Consent(patient, provider), &false);
    }

    /// Create a record. Requires both patient and provider auth, plus prior patient consent.
    pub fn create_record(
        env: Env,
        patient: Address,
        provider: Address,
        encrypted_ref: EncryptedEnvelopeRef,
        record_type: String,
        policy: PolicyMetadata,
    ) -> Result<u64, Error> {
        patient.require_auth();
        provider.require_auth();
        validate_encrypted_ref(&encrypted_ref).map_err(|_| Error::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| Error::InvalidPolicyMetadata)?;

        if !has_consent(&env, &patient, &provider) {
            return Err(Error::ConsentNotGranted);
        }

        let counter_key = DataKey::RecordCounter;
        let record_id: u64 = shared_contracts::safe_increment_persistent(&env, &counter_key);

        let timestamp = env.ledger().timestamp();

        let integrity_hash = compute_hash(
            &env,
            record_id,
            &patient,
            &provider,
            &encrypted_ref,
            &record_type,
            timestamp,
        );

        let record = MedicalRecord {
            record_id,
            patient,
            provider,
            encrypted_ref,
            record_type,
            timestamp,
            integrity_hash,
            policy,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id), &record);

        Ok(record_id)
    }

    /// Retrieve a record. Caller must be the patient or a consented provider.
    pub fn get_record(env: Env, caller: Address, record_id: u64) -> Result<MedicalRecord, Error> {
        caller.require_auth();

        let record: MedicalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)?;

        if caller != record.patient && !has_consent(&env, &record.patient, &caller) {
            return Err(Error::Unauthorized);
        }

        Ok(record)
    }

    /// Verify integrity. Caller must be the patient or a consented provider.
    pub fn verify_record_integrity(
        env: Env,
        caller: Address,
        record_id: u64,
        expected_hash: Bytes,
    ) -> Result<bool, Error> {
        caller.require_auth();

        let record: MedicalRecord =
            match env.storage().persistent().get(&DataKey::Record(record_id)) {
                Some(r) => r,
                None => return Ok(false),
            };

        if caller != record.patient && !has_consent(&env, &record.patient, &caller) {
            return Err(Error::Unauthorized);
        }

        if expected_hash.len() != 32 {
            return Ok(false);
        }

        let recomputed = compute_hash(
            &env,
            record.record_id,
            &record.patient,
            &record.provider,
            &record.encrypted_ref,
            &record.record_type,
            record.timestamp,
        );

        let recomputed_bytes: Bytes = recomputed.into();
        Ok(recomputed_bytes == expected_hash)
    }

    /// Capture an incident for this contract, optionally linking it to a
    /// cross-contract correlation ID.  Returns the new incident ID.
    pub fn report_incident(
        env: Env,
        reporter: Address,
        error_code: u32,
        description: String,
        correlation_id: Option<BytesN<32>>,
    ) -> u64 {
        reporter.require_auth();
        capture_incident(
            &env,
            IncidentSeverity::High,
            String::from_str(&env, "health-records"),
            error_code,
            description,
            reporter,
            correlation_id,
        )
    }

    /// Return all incident IDs that share the given correlation ID.
    pub fn get_incidents_by_correlation_id(
        env: Env,
        correlation_id: BytesN<32>,
    ) -> Vec<u64> {
        shared_get_by_corr(&env, correlation_id)
    }
}

#[cfg(test)]
mod test;
#[cfg(test)]
mod cid_fuzz_tests;
