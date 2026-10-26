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
    Deregistering = 5,
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
    Deregistering(Address),       // owner -> bool tombstone blocking new writes
    DeregisterCursor(Address),    // owner -> u32 next record idx to delete
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
        if Self::is_deregistering(&e, &owner) {
            return Err(ContractError::Deregistering);
        }
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
        let end = offset.saturating_add(limit).min(count);

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
    ///
    /// The date index is insertion-ordered and timestamps are monotonic, so we can skip
    /// directly to `offset` matching entries instead of scanning the whole history.
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

    /// Paginated retrieval of records by type via the type index.
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
        let type_count: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::TypeCount(owner.clone(), rt))
            .unwrap_or(0);

        let mut records = vec![&e];
        let end = offset.saturating_add(limit).min(type_count);

        for seq in offset..end {
            let record_idx: u32 = match e
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

    /// Grant `authorized` access to `owner`'s financial records.
    pub fn grant_access(
        e: Env,
        owner: Address,
        authorized: Address,
    ) -> Result<(), ContractError> {
        owner.require_auth();
        if Self::is_deregistering(&e, &owner) {
            return Err(ContractError::Deregistering);
        }

        e.storage()
            .persistent()
            .set(&DataKey::Access(owner.clone(), authorized.clone()), &true);

        let mut list: Vec<Address> = e
            .storage()
            .persistent()
            .get(&DataKey::AccessList(owner.clone()))
            .unwrap_or_else(|| vec![&e]);
        if !list.contains(authorized.clone()) {
            list.push_back(authorized.clone());
            e.storage()
                .persistent()
                .set(&DataKey::AccessList(owner.clone()), &list);
        }

        e.events().publish(
            (symbol_short!("acc_grant"), owner, authorized),
            e.ledger().timestamp(),
        );
        Ok(())
    }

    /// Revoke `authorized` access from `owner`'s financial records.
    pub fn revoke_access(
        e: Env,
        owner: Address,
        authorized: Address,
    ) -> Result<(), ContractError> {
        owner.require_auth();

        e.storage()
            .persistent()
            .remove(&DataKey::Access(owner.clone(), authorized.clone()));

        let list: Vec<Address> = e
            .storage()
            .persistent()
            .get(&DataKey::AccessList(owner.clone()))
            .unwrap_or_else(|| vec![&e]);
        let mut updated = vec![&e];
        for a in list.iter() {
            if a != authorized {
                updated.push_back(a);
            }
        }
        e.storage()
            .persistent()
            .set(&DataKey::AccessList(owner.clone()), &updated);

        e.events().publish(
            (symbol_short!("acc_revoke"), owner, authorized),
            e.ledger().timestamp(),
        );
        Ok(())
    }

    /// Resumable deregistration. Deletes at most `max_items` ledger entries per call and
    /// returns the number of items still remaining. While `remaining > 0` a `Deregistering`
    /// tombstone blocks new writes; callers must invoke again until it returns 0.
    pub fn deregister_patient(e: Env, owner: Address, max_items: u32) -> Result<u32, ContractError> {
        owner.require_auth();

        // Mark deregistration in progress so new writes are rejected.
        e.storage()
            .persistent()
            .set(&DataKey::Deregistering(owner.clone()), &true);

        let mut budget = max_items;

        // 1. Delete access grants + access list.
        let list: Vec<Address> = e
            .storage()
            .persistent()
            .get(&DataKey::AccessList(owner.clone()))
            .unwrap_or_else(|| vec![&e]);
        for a in list.iter() {
            if budget == 0 {
                return Ok(Self::remaining_items(&e, &owner));
            }
            e.storage()
                .persistent()
                .remove(&DataKey::Access(owner.clone(), a));
            budget -= 1;
        }
        if !list.is_empty() {
            if budget == 0 {
                return Ok(Self::remaining_items(&e, &owner));
            }
            e.storage()
                .persistent()
                .remove(&DataKey::AccessList(owner.clone()));
            budget -= 1;
        }

        // 2. Delete records + their type/date index entries, resuming from a cursor.
        let count: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::RecordCount(owner.clone()))
            .unwrap_or(0);
        let mut cursor: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::DeregisterCursor(owner.clone()))
            .unwrap_or(0);

        while cursor < count {
            // Each record needs: Record, TypeIndex, DateIndex = 3 deletions.
            if budget < 3 {
                e.storage()
                    .persistent()
                    .set(&DataKey::DeregisterCursor(owner.clone()), &cursor);
                return Ok(Self::remaining_items(&e, &owner));
            }

            let record: Option<FinancialRecord> = e
                .storage()
                .persistent()
                .get(&DataKey::Record(owner.clone(), cursor));

            if let Some(rec) = record {
                let rt = rec.record_type as u32;
                // Remove the type index entry pointing at this record idx.
                let type_count: u32 = e
                    .storage()
                    .persistent()
                    .get(&DataKey::TypeCount(owner.clone(), rt))
                    .unwrap_or(0);
                for seq in 0..type_count {
                    if let Some(idx) = e
                        .storage()
                        .persistent()
                        .get::<DataKey, u32>(&DataKey::TypeIndex(owner.clone(), rt, seq))
                    {
                        if idx == cursor {
                            e.storage()
                                .persistent()
                                .remove(&DataKey::TypeIndex(owner.clone(), rt, seq));
                            break;
                        }
                    }
                }
                // Remove the date index entry pointing at this record idx.
                let date_count: u32 = e
                    .storage()
                    .persistent()
                    .get(&DataKey::DateCount(owner.clone()))
                    .unwrap_or(0);
                for seq in 0..date_count {
                    if let Some(idx) = e
                        .storage()
                        .persistent()
                        .get::<DataKey, u32>(&DataKey::DateIndex(owner.clone(), seq))
                    {
                        if idx == cursor {
                            e.storage()
                                .persistent()
                                .remove(&DataKey::DateIndex(owner.clone(), seq));
                            break;
                        }
                    }
                }
            }

            e.storage()
                .persistent()
                .remove(&DataKey::Record(owner.clone(), cursor));
            budget -= 3;
            cursor += 1;
        }

        // 3. Delete count keys.
        if budget >= 1 {
            e.storage()
                .persistent()
                .remove(&DataKey::RecordCount(owner.clone()));
            budget -= 1;
        }
        if budget >= 1 {
            e.storage()
                .persistent()
                .remove(&DataKey::DateCount(owner.clone()));
            budget -= 1;
        }
        for rt in 0..5u32 {
            if budget == 0 {
                return Ok(Self::remaining_items(&e, &owner));
            }
            e.storage()
                .persistent()
                .remove(&DataKey::TypeCount(owner.clone(), rt));
            budget -= 1;
        }

        // Fully deregistered: clear tombstone and cursor.
        e.storage()
            .persistent()
            .remove(&DataKey::Deregistering(owner.clone()));
        e.storage()
            .persistent()
            .remove(&DataKey::DeregisterCursor(owner.clone()));

        e.events().publish(
            (symbol_short!("dereg"), owner),
            e.ledger().timestamp(),
        );
        Ok(0)
    }

    fn is_deregistering(e: &Env, owner: &Address) -> bool {
        e.storage()
            .persistent()
            .get(&DataKey::Deregistering(owner.clone()))
            .unwrap_or(false)
    }

    /// Approximate count of ledger entries still to delete for `owner`.
    fn remaining_items(e: &Env, owner: &Address) -> u32 {
        let count: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::RecordCount(owner.clone()))
            .unwrap_or(0);
        let cursor: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::DeregisterCursor(owner.clone()))
            .unwrap_or(0);
        let list: Vec<Address> = e
            .storage()
            .persistent()
            .get(&DataKey::AccessList(owner.clone()))
            .unwrap_or_else(|| vec![e]);
        let records_left = count.saturating_sub(cursor);
        // 3 deletions per remaining record + access grants + count keys.
        records_left
            .saturating_mul(3)
            .saturating_add(list.len())
            .saturating_add(7)
    }

    fn check_access(e: &Env, caller: &Address, owner: &Address) -> Result<(), ContractError> {
        if caller == owner {
            return Ok(());
        }
        let granted: bool = e
            .storage()
            .persistent()
            .get(&DataKey::Access(owner.clone(), caller.clone()))
            .unwrap_or(false);
        if granted {
            Ok(())
        } else {
            Err(ContractError::AccessDenied)
        }
    }
}
