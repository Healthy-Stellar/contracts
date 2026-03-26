#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, String,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedicalRecord {
    pub record_id: u64,
    pub patient: Address,
    pub provider: Address,
    pub ipfs_cid: String,
    pub record_type: String,
    pub timestamp: u64,
    pub integrity_hash: BytesN<32>,
}

#[contracttype]
pub enum DataKey {
    Record(u64),
    RecordCounter,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    RecordNotFound = 1,
}

fn compute_hash(
    env: &Env,
    record_id: u64,
    patient: &Address,
    provider: &Address,
    ipfs_cid: &String,
    record_type: &String,
    timestamp: u64,
) -> BytesN<32> {
    let mut data = Bytes::new(env);

    // record_id as 8 big-endian bytes
    data.extend_from_array(&record_id.to_be_bytes());

    // patient address bytes
    let patient_bytes = patient.clone().to_xdr(env);
    data.append(&patient_bytes);

    // provider address bytes
    let provider_bytes = provider.clone().to_xdr(env);
    data.append(&provider_bytes);

    // ipfs_cid bytes
    let cid_bytes = ipfs_cid.clone().to_xdr(env);
    data.append(&cid_bytes);

    // record_type bytes
    let type_bytes = record_type.clone().to_xdr(env);
    data.append(&type_bytes);

    // timestamp as 8 big-endian bytes
    data.extend_from_array(&timestamp.to_be_bytes());

    env.crypto().sha256(&data)
}

#[contract]
pub struct HealthRecords;

#[contractimpl]
impl HealthRecords {
    pub fn create_record(
        env: Env,
        patient: Address,
        provider: Address,
        ipfs_cid: String,
        record_type: String,
    ) -> u64 {
        patient.require_auth();

        let counter_key = DataKey::RecordCounter;
        let record_id: u64 = env
            .storage()
            .persistent()
            .get(&counter_key)
            .unwrap_or(0u64)
            + 1;

        let timestamp = env.ledger().timestamp();

        let integrity_hash = compute_hash(
            &env,
            record_id,
            &patient,
            &provider,
            &ipfs_cid,
            &record_type,
            timestamp,
        );

        let record = MedicalRecord {
            record_id,
            patient,
            provider,
            ipfs_cid,
            record_type,
            timestamp,
            integrity_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id), &record);
        env.storage().persistent().set(&counter_key, &record_id);

        record_id
    }

    pub fn get_record(env: Env, record_id: u64) -> Result<MedicalRecord, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)
    }

    pub fn verify_record_integrity(env: Env, record_id: u64, expected_hash: Bytes) -> bool {
        let record: MedicalRecord = match env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
        {
            Some(r) => r,
            None => return false,
        };

        if expected_hash.len() != 32 {
            return false;
        }

        let recomputed = compute_hash(
            &env,
            record.record_id,
            &record.patient,
            &record.provider,
            &record.ipfs_cid,
            &record.record_type,
            record.timestamp,
        );

        // Compare expected_hash (Bytes) against recomputed (BytesN<32>)
        let recomputed_bytes: Bytes = recomputed.into();
        recomputed_bytes == expected_hash
    }
}

#[cfg(test)]
mod test;
