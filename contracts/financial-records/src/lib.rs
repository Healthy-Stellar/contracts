//! Financial Records Contract
//!
//! Manages patient financial records, billing, payments, and insurance claims
//! on the Stellar blockchain with encryption and access control.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    String, Symbol, Vec,
};

// ============================================================================
// Error Types
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    RecordNotFound = 4,
    InvalidAmount = 5,
    InvalidStatus = 6,
    AccessDenied = 7,
    GrantNotFound = 8,
    GrantExpired = 9,
    GrantRevoked = 10,
    InvalidInput = 11,
    DuplicateRecord = 12,
    InsufficientFunds = 13,
    PaymentFailed = 14,
    ClaimNotFound = 15,
    InvalidClaimStatus = 16,
    EncryptionError = 17,
    Paused = 18,
}

// ============================================================================
// Data Types
// ============================================================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordType {
    Invoice,
    Payment,
    InsuranceClaim,
    Refund,
    Adjustment,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaymentMethod {
    Cash,
    CreditCard,
    DebitCard,
    Insurance,
    BankTransfer,
    Crypto,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinancialRecord {
    pub id: u64,
    pub patient_id: Address,
    pub record_type: RecordType,
    pub amount: i128,
    pub currency: Symbol,
    pub status: RecordStatus,
    pub description: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub encrypted_data: Option<Bytes>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Payment {
    pub id: u64,
    pub record_id: u64,
    pub payer: Address,
    pub amount: i128,
    pub method: PaymentMethod,
    pub paid_at: u64,
    pub transaction_ref: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsuranceClaim {
    pub id: u64,
    pub record_id: u64,
    pub patient_id: Address,
    pub provider_id: Address,
    pub claim_amount: i128,
    pub approved_amount: i128,
    pub status: RecordStatus,
    pub submitted_at: u64,
    pub processed_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessGrant {
    pub grantee: Address,
    pub granted_by: Address,
    pub granted_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractConfig {
    pub admin: Address,
    pub paused: bool,
    pub initialized_at: u64,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
    RecordCounter,
    PaymentCounter,
    ClaimCounter,
    Record(u64),
    Payment(u64),
    Claim(u64),
    PatientRecords(Address),
    AccessGrant(Address, Address),
}

// ============================================================================
// Contract
// ============================================================================

#[contract]
pub struct FinancialRecordsContract;

#[contractimpl]
impl FinancialRecordsContract {
    /// Initialize the contract with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        let config = ContractConfig {
            admin,
            paused: false,
            initialized_at: env.ledger().timestamp(),
        };

        env.storage().instance().set(&DataKey::Config, &config);
        env.storage().instance().set(&DataKey::RecordCounter, &0u64);
        env.storage().instance().set(&DataKey::PaymentCounter, &0u64);
        env.storage().instance().set(&DataKey::ClaimCounter, &0u64);

        env.events()
            .publish((symbol_short!("init"),), config.initialized_at);

        Ok(())
    }

    /// Create a new financial record for a patient.
    pub fn create_record(
        env: Env,
        patient_id: Address,
        record_type: RecordType,
        amount: i128,
        currency: Symbol,
        description: String,
        encrypted_data: Option<Bytes>,
    ) -> Result<u64, Error> {
        Self::require_not_paused(&env)?;
        patient_id.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RecordCounter)
            .unwrap_or(0);
        counter += 1;

        let now = env.ledger().timestamp();
        let record = FinancialRecord {
            id: counter,
            patient_id: patient_id.clone(),
            record_type,
            amount,
            currency,
            status: RecordStatus::Pending,
            description,
            created_at: now,
            updated_at: now,
            encrypted_data,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Record(counter), &record);
        env.storage().instance().set(&DataKey::RecordCounter, &counter);

        let mut patient_records: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientRecords(patient_id.clone()))
            .unwrap_or(Vec::new(&env));
        patient_records.push_back(counter);
        env.storage()
            .persistent()
            .set(&DataKey::PatientRecords(patient_id), &patient_records);

        env.events()
            .publish((symbol_short!("rec_new"),), counter);

        Ok(counter)
    }

    /// Retrieve a financial record by id.
    pub fn get_record(env: Env, record_id: u64) -> Result<FinancialRecord, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)
    }

    /// Update the status of a financial record.
    pub fn update_record_status(
        env: Env,
        record_id: u64,
        status: RecordStatus,
    ) -> Result<(), Error> {
        Self::require_not_paused(&env)?;

        let mut record: FinancialRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)?;

        record.patient_id.require_auth();
        record.status = status;
        record.updated_at = env.ledger().timestamp();

        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id), &record);

        env.events()
            .publish((symbol_short!("rec_upd"),), record_id);

        Ok(())
    }

    /// Record a payment against a financial record.
    pub fn record_payment(
        env: Env,
        record_id: u64,
        payer: Address,
        amount: i128,
        method: PaymentMethod,
        transaction_ref: BytesN<32>,
    ) -> Result<u64, Error> {
        Self::require_not_paused(&env)?;
        payer.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut record: FinancialRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)?;

        let mut counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PaymentCounter)
            .unwrap_or(0);
        counter += 1;

        let payment = Payment {
            id: counter,
            record_id,
            payer,
            amount,
            method,
            paid_at: env.ledger().timestamp(),
            transaction_ref,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Payment(counter), &payment);
        env.storage().instance().set(&DataKey::PaymentCounter, &counter);

        record.status = RecordStatus::Completed;
        record.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Record(record_id), &record);

        env.events()
            .publish((symbol_short!("pay_new"),), counter);

        Ok(counter)
    }

    /// Submit an insurance claim for a record.
    pub fn submit_claim(
        env: Env,
        record_id: u64,
        provider_id: Address,
        claim_amount: i128,
    ) -> Result<u64, Error> {
        Self::require_not_paused(&env)?;

        if claim_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let record: FinancialRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Record(record_id))
            .ok_or(Error::RecordNotFound)?;

        record.patient_id.require_auth();

        let mut counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ClaimCounter)
            .unwrap_or(0);
        counter += 1;

        let claim = InsuranceClaim {
            id: counter,
            record_id,
            patient_id: record.patient_id.clone(),
            provider_id,
            claim_amount,
            approved_amount: 0,
            status: RecordStatus::Pending,
            submitted_at: env.ledger().timestamp(),
            processed_at: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Claim(counter), &claim);
        env.storage().instance().set(&DataKey::ClaimCounter, &counter);

        env.events()
            .publish((symbol_short!("clm_new"),), counter);

        Ok(counter)
    }

    /// Process an insurance claim (admin only).
    pub fn process_claim(
        env: Env,
        claim_id: u64,
        approved_amount: i128,
        status: RecordStatus,
    ) -> Result<(), Error> {
        Self::require_not_paused(&env)?;
        Self::require_admin(&env)?;

        let mut claim: InsuranceClaim = env
            .storage()
            .persistent()
            .get(&DataKey::Claim(claim_id))
            .ok_or(Error::ClaimNotFound)?;

        claim.approved_amount = approved_amount;
        claim.status = status;
        claim.processed_at = Some(env.ledger().timestamp());

        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);

        env.events()
            .publish((symbol_short!("clm_prc"),), claim_id);

        Ok(())
    }

    /// Grant read access to a patient's records.
    pub fn grant_access(
        env: Env,
        patient_id: Address,
        grantee: Address,
        expires_at: u64,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let grant = AccessGrant {
            grantee: grantee.clone(),
            granted_by: patient_id.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at,
            revoked: false,
        };

        env.storage().persistent().set(
            &DataKey::AccessGrant(patient_id.clone(), grantee.clone()),
            &grant,
        );

        env.events().publish(
            (symbol_short!("acc_grn"),),
            (patient_id, grantee),
        );

        Ok(())
    }

    /// Revoke a previously granted access.
    pub fn revoke_access(
        env: Env,
        patient_id: Address,
        grantee: Address,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        let key = DataKey::AccessGrant(patient_id.clone(), grantee.clone());
        let mut grant: AccessGrant = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::GrantNotFound)?;

        grant.revoked = true;
        env.storage().persistent().set(&key, &grant);

        env.events().publish(
            (symbol_short!("acc_rvk"),),
            (patient_id, grantee),
        );

        Ok(())
    }

    /// Check whether a grantee currently has access to a patient's records.
    pub fn has_access(env: Env, patient_id: Address, grantee: Address) -> bool {
        let key = DataKey::AccessGrant(patient_id, grantee);
        match env
            .storage()
            .persistent()
            .get::<DataKey, AccessGrant>(&key)
        {
            Some(grant) => {
                !grant.revoked && grant.expires_at > env.ledger().timestamp()
            }
            None => false,
        }
    }

    /// List all record ids for a patient.
    pub fn get_patient_records(env: Env, patient_id: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::PatientRecords(patient_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Pause or unpause the contract (admin only).
    pub fn set_paused(env: Env, paused: bool) -> Result<(), Error> {
        Self::require_admin(&env)?;

        let mut config: ContractConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;

        config.paused = paused;
        env.storage().instance().set(&DataKey::Config, &config);

        env.events()
            .publish((symbol_short!("paused"),), paused);

        Ok(())
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    fn require_admin(env: &Env) -> Result<Address, Error> {
        let config: ContractConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;
        config.admin.require_auth();
        Ok(config.admin)
    }

    fn require_not_paused(env: &Env) -> Result<(), Error> {
        let config: ContractConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;
        if config.paused {
            return Err(Error::Paused);
        }
        Ok(())
    }
}
