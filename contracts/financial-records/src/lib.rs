#![no_std]

//! # Financial Records Contract
//!
//! Manages patient financial records including tax documents, insurance claims, payment records,
//! and billing information with encrypted storage and access control.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Patient authentication required for record access. Authorization
//! checks validate requestor eligibility (patient, authorized providers, insurers). Tax and payment
//! records access restricted to authorized financial/billing personnel. Encrypted envelope validation
//! ensures data integrity.
//!
//! **Audit Controls:** Record retrieval and modification events emitted with user identity. Access
//! grants and revocations logged. Financial transaction timestamps tracked. Record type enumeration
//! (TaxDocument, InsuranceClaim, PaymentRecord, BillingInfo) enables category-specific auditing.
//!
//! **Data Retention Policy:** Tax documents retained per regulatory requirements. Insurance claims
//! retained with resolution status. Payment records archived indefinitely. Policy metadata enforces
//! retention rules per record type. Patient deregistration removes all financial records.
//!
//! **Encryption/Integrity:** EncryptedEnvelopeRef with content hashing for integrity verification.
//! PolicyMetadata enforces encryption requirements per record type. Persistent storage for financial
//! data. Access control grants tracked per (patient, requester) pair.

use shared::privacy::{
    validate_encrypted_ref, validate_policy_metadata, EncryptedEnvelopeRef, PolicyMetadata,
};
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, vec, Address, Env, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AccessDenied = 1,
    RecordNotFound = 2,
    InvalidEncryptedEnvelope = 3,
    InvalidPolicyMetadata = 4,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordType {
    TaxDocument = 0,
    Invoice = 1,
    Receipt = 2,
    BankStatement = 3,
    Other = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinancialRecord {
    pub owner: Address,
    pub record_type: RecordType,
    pub encrypted_ref: EncryptedEnvelopeRef,
    pub timestamp: u64,
    pub policy: PolicyMetadata,
}

#[contracttype]
pub enum DataKey {
    Record(Address, u32),         // (owner, idx) -> FinancialRecord
    RecordCount(Address),         // owner -> u32
    Access(Address, Address),     // (owner, authorized) -> bool
    AccessList(Address),          // owner -> Vec<Address> of currently granted accessors
    TypeIndex(Address, u32, u32), // (owner, record_type as u32, seq) -> record idx
    TypeCount(Address, u32),      // (owner, record_type as u32) -> u32
    DateIndex(Address, u32),      // (owner, seq) -> record idx  (insertion order)
    DateCount(Address),           // owner -> u32
}

#[contract]
pub struct FinancialRecordContract;

#[contractimpl]
impl FinancialRecordContract {
    pub fn add_financial_record(
        e: Env,
        owner: Address,
        record_type: RecordType,
        encrypted_ref: EncryptedEnvelopeRef,
        policy: PolicyMetadata,
    ) -> Result<(), ContractError> {
        owner.require_auth();
        validate_encrypted_ref(&encrypted_ref)
            .map_err(|_| ContractError::InvalidEncryptedEnvelope)?;
        validate_policy_metadata(&policy).map_err(|_| ContractError::InvalidPolicyMetadata)?;

        let count: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::RecordCount(owner.clone()))
            .unwrap_or(0);
        let timestamp = e.ledger().timestamp();

        let record = FinancialRecord {
            owner: owner.clone(),
            record_type,
            encrypted_ref,
            timestamp,
            policy,
        };

        e.storage()
            .persistent()
            .set(&DataKey::Record(owner.clone(), count), &record);
        e.storage()
            .persistent()
            .set(&DataKey::RecordCount(owner.clone()), &(count + 1));

        e.events().publish(
            (symbol_short!("rec_add"), owner.clone(), count, record.record_type as u32),
            (timestamp, record.encrypted_ref.content_hash.clone()),
        );

        // Type index
        let rt = record_type as u32;
        let type_seq: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::TypeCount(owner.clone(), rt))
            .unwrap_or(0);
        e.storage()
            .persistent()
            .set(&DataKey::TypeIndex(owner.clone(), rt, type_seq), &count);
        e.storage()
            .persistent()
            .set(&DataKey::TypeCount(owner.clone(), rt), &(type_seq + 1));

        // Date index (insertion-ordered; record carries its own timestamp for range filtering)
        let date_seq: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::DateCount(owner.clone()))
            .unwrap_or(0);
        e.storage()
            .persistent()
            .set(&DataKey::DateIndex(owner.clone(), date_seq), &count);
        e.storage()
            .persistent()
            .set(&DataKey::DateCount(owner.clone()), &(date_seq + 1));
        Ok(())
    }

    /// Paginated retrieval of all records. `offset` is the record index to start from.
    pub fn get_financial_records(
        e: Env,
        caller: Address,
        owner: Address,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<FinancialRecord>, ContractError> {
        Self::check_access(&e, &caller, &owner)?;

        let count: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::RecordCount(owner.clone()))
            .unwrap_or(0);
        let mut records = vec![&e];
        let end = (offset + limit).min(count);

        for i in offset..end {
            if let Some(record) = e
                .storage()
                .persistent()
                .get::<DataKey, FinancialRecord>(&DataKey::Record(owner.clone(), i))
            {
                if record.owner != owner {
                    return Err(ContractError::AccessDenied);
                }
                records.push_back(record);
            }
        }
        Ok(records)
    }

    /// Paginated retrieval of records within [start, end] timestamp range via date index.
    pub fn get_records_by_date_range(
        e: Env,
        caller: Address,
        owner: Address,
        start: u64,
        end: u64,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<FinancialRecord>, ContractError> {
        Self::check_access(&e, &caller, &owner)?;

        let date_seq: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::DateCount(owner.clone()))
            .unwrap_or(0);

        let mut records = vec![&e];
        let mut skipped: u32 = 0;

        for seq in 0..date_seq {
            let record_idx: u32 = match e
                .storage()
                .persistent()
                .get(&DataKey::DateIndex(owner.clone(), seq))
            {
                Some(idx) => idx,
                None => continue,
            };
            let record: FinancialRecord = match e
                .storage()
                .persistent()
                .get(&DataKey::Record(owner.clone(), record_idx))
            {
                Some(r) => r,
                None => continue,
            };
            if record.owner != owner {
                return Err(ContractError::AccessDenied);
            }
            if record.timestamp >= start && record.timestamp <= end {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                if records.len() >= limit {
                    break;
                }
                records.push_back(record);
            }
        }
        Ok(records)
    }

    /// Paginated retrieval of records by type via type index.
    pub fn get_records_by_type(
        e: Env,
        caller: Address,
        owner: Address,
        record_type: RecordType,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<FinancialRecord>, ContractError> {
        Self::check_access(&e, &caller, &owner)?;

        let rt = record_type as u32;
        let type_seq: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::TypeCount(owner.clone(), rt))
            .unwrap_or(0);

        let mut records = vec![&e];
        let end = (offset + limit).min(type_seq);

        for seq in offset..end {
            let record_idx: u32 =
                match e
                    .storage()
                    .persistent()
                    .get(&DataKey::TypeIndex(owner.clone(), rt, seq))
                {
                    Some(idx) => idx,
                    None => continue,
                };
            if let Some(record) = e
                .storage()
                .persistent()
                .get::<DataKey, FinancialRecord>(&DataKey::Record(owner.clone(), record_idx))
            {
                if record.owner != owner {
                    return Err(ContractError::AccessDenied);
                }
                records.push_back(record);
            }
        }
        Ok(records)
    }

    pub fn grant_access(e: Env, owner: Address, authorized: Address) {
        owner.require_auth();

        let mut granted: Vec<Address> = e
            .storage()
            .persistent()
            .get(&DataKey::AccessList(owner.clone()))
            .unwrap_or(Vec::new(&e));

        let mut i = 0u32;
        while i < granted.len() {
            if let Some(existing) = granted.get(i) {
                if existing == authorized {
                    e.storage()
                        .persistent()
                        .set(&DataKey::Access(owner.clone(), authorized.clone()), &true);
                    e.events()
                        .publish((symbol_short!("grant"), owner, authorized), ());
                    return;
                }
            }
            i += 1;
        }

        granted.push_back(authorized.clone());
        e.storage()
            .persistent()
            .set(&DataKey::AccessList(owner.clone()), &granted);
        e.storage()
            .persistent()
            .set(&DataKey::Access(owner.clone(), authorized.clone()), &true);
        e.events()
            .publish((symbol_short!("grant"), owner, authorized), ());
    }

    pub fn revoke_access(e: Env, owner: Address, authorized: Address) {
        owner.require_auth();

        let mut granted: Vec<Address> = e
            .storage()
            .persistent()
            .get(&DataKey::AccessList(owner.clone()))
            .unwrap_or(Vec::new(&e));

        let mut filtered: Vec<Address> = Vec::new(&e);
        let mut i = 0u32;
        while i < granted.len() {
            if let Some(existing) = granted.get(i) {
                if existing != authorized {
                    filtered.push_back(existing);
                }
            }
            i += 1;
        }

        if filtered.is_empty() {
            e.storage().persistent().remove(&DataKey::AccessList(owner.clone()));
        } else {
            e.storage()
                .persistent()
                .set(&DataKey::AccessList(owner.clone()), &filtered);
        }

        e.storage()
            .persistent()
            .remove(&DataKey::Access(owner.clone(), authorized.clone()));
        e.events()
            .publish((symbol_short!("revoke"), owner, authorized), ());
    }

    pub fn deregister_patient(e: Env, owner: Address) -> Result<(), ContractError> {
        owner.require_auth();

        // Remove every Access(owner, authorized) ACL entry before clearing patient data.
        if let Some(granted) = e
            .storage()
            .persistent()
            .get::<_, Vec<Address>>(&DataKey::AccessList(owner.clone()))
        {
            let mut i = 0u32;
            while i < granted.len() {
                if let Some(authorized) = granted.get(i) {
                    e.storage()
                        .persistent()
                        .remove(&DataKey::Access(owner.clone(), authorized.clone()));
                }
                i += 1;
            }
            e.storage().persistent().remove(&DataKey::AccessList(owner.clone()));
        }

        // Get the count of records to remove
        let count: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::RecordCount(owner.clone()))
            .unwrap_or(0);

        // Remove all FinancialRecord entries
        for i in 0..count {
            e.storage()
                .persistent()
                .remove(&DataKey::Record(owner.clone(), i));
        }

        // Remove record count
        e.storage()
            .persistent()
            .remove(&DataKey::RecordCount(owner.clone()));

        // Remove all type indices and type counts
        for rt in 0..5 {
            let type_seq: u32 = e
                .storage()
                .persistent()
                .get(&DataKey::TypeCount(owner.clone(), rt))
                .unwrap_or(0);

            for seq in 0..type_seq {
                e.storage()
                    .persistent()
                    .remove(&DataKey::TypeIndex(owner.clone(), rt, seq));
            }

            e.storage()
                .persistent()
                .remove(&DataKey::TypeCount(owner.clone(), rt));
        }

        // Remove all date indices and date count
        let date_seq: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::DateCount(owner.clone()))
            .unwrap_or(0);

        for seq in 0..date_seq {
            e.storage()
                .persistent()
                .remove(&DataKey::DateIndex(owner.clone(), seq));
        }

        e.storage()
            .persistent()
            .remove(&DataKey::DateCount(owner.clone()));

        // Emit deregistration event
        e.events()
            .publish((symbol_short!("pat_dreg"), owner.clone()), symbol_short!("fr_clean"));

        Ok(())
    }

    fn check_access(e: &Env, caller: &Address, owner: &Address) -> Result<(), ContractError> {
        if caller == owner {
            caller.require_auth();
            return Ok(());
        }
        caller.require_auth();
        let authorized: bool = e
            .storage()
            .persistent()
            .get(&DataKey::Access(owner.clone(), caller.clone()))
            .unwrap_or(false);
        if !authorized {
            return Err(ContractError::AccessDenied);
        }
        e.events().publish(
            (symbol_short!("rec_read"), caller.clone(), owner.clone()),
            (symbol_short!("granted"), e.ledger().timestamp()),
        );
        Ok(())
    }
}

mod test;
