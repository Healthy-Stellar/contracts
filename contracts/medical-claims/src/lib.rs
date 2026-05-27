#![no_std]
#![allow(clippy::too_many_arguments)]

mod test;
mod types;

use shared::privacy::{validate_policy_metadata, PolicyMetadata};
use soroban_sdk::{contract, contractclient, contractimpl, Address, BytesN, Env, String, Vec};
use types::{
    ClaimRecord, ClaimStatus, DataKey, DenialInfo, Error, InsurerPaymentRecord,
    PatientPaymentRecord, ReconciliationStatus, ServiceLine,
};

// ── Cross-contract interface for consent verification (#300) ──────────────────
//
// Defines only the one method we need; the generated `AccessControlClient`
// calls the access-control contract by function name at runtime.
#[contractclient(name = "AccessControlClient")]
pub trait AccessControlInterface {
    /// Returns `()` on success; panics / traps when consent is absent,
    /// expired, or revoked.  The `try_` variant is used below to convert
    /// those failures into `Error::ConsentNotVerified`.
    fn check_consent(
        env: Env,
        subject: Address,
        grantee: Address,
        purpose_code: String,
        required_scope: u32,
    );
}

#[contract]
pub struct MedicalClaimsSystem;

#[contractimpl]
impl MedicalClaimsSystem {
    /// One-time setup: register the contract admin and the access-control
    /// contract that will be queried for patient → provider consent.
    ///
    /// `access_control_id` must be the address of the deployed access-control
    /// contract.  Every `submit_claim` call will cross-contract-call its
    /// `check_consent` function before creating a claim record (#300).
    pub fn initialize(
        env: Env,
        admin: Address,
        access_control_id: Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::AccessControlId, &access_control_id);
        Ok(())
    }

    /// Admin-only: authorize an insurer address to adjudicate and pay claims.
    pub fn register_insurer(env: Env, admin: Address, insurer: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::NotAuthorized);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Insurer(insurer), &true);
        Ok(())
    }

    fn require_insurer(env: &Env, insurer: &Address) -> Result<(), Error> {
        let registered: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Insurer(insurer.clone()))
            .unwrap_or(false);
        if !registered {
            return Err(Error::InsurerNotRegistered);
        }
        Ok(())
    }

    /// Submit a claim bound to a specific registered insurer.
    pub fn submit_claim(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        insurer_id: Address,
        policy_id: u64,
        service_date: u64,
        service_codes: Vec<ServiceLine>,
        diagnosis_hashes: Vec<BytesN<32>>,
        claim_details_hash: BytesN<32>,
        policy: PolicyMetadata,
        total_amount: i128,
    ) -> Result<u64, Error> {
        provider_id.require_auth();
        Self::require_insurer(&env, &insurer_id)?;
        validate_policy_metadata(&policy).map_err(|_| Error::InvalidPolicyMetadata)?;

        // #300: Verify that the patient has granted consent to this provider
        // before creating a claim record (HIPAA compliance).
        //
        // We look up the access-control contract stored at initialization time,
        // then call check_consent(patient, provider, "treatment", 0x01 = read).
        // Any failure (ConsentNotFound / ConsentRevoked / ConsentExpired /
        // ConsentDenied) is surfaced as Error::ConsentNotVerified so callers
        // receive a clear, auditable rejection.
        let access_control_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::AccessControlId)
            .ok_or(Error::NotInitialized)?;

        let ac_client = AccessControlClient::new(&env, &access_control_id);
        ac_client
            .try_check_consent(
                &patient_id,
                &provider_id,
                &String::from_str(&env, "treatment"),
                &1u32, // scope bit 0x01 = read
            )
            .map_err(|_| Error::ConsentNotVerified)?;

        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ClaimCounter)
            .unwrap_or(0);
        let claim_id = count + 1;
        env.storage()
            .instance()
            .set(&DataKey::ClaimCounter, &claim_id);

        let claim = ClaimRecord {
            claim_id,
            provider_id: provider_id.clone(),
            patient_id: patient_id.clone(),
            insurer_id,
            policy_id,
            service_date,
            service_codes,
            diagnosis_hashes,
            details_hash: claim_details_hash,
            policy,
            total_amount,
            status: ClaimStatus::Submitted,
            approved_amount: None,
            patient_responsibility: None,
            appeal_level: 0,
            insurer_paid_amount: 0,
            patient_paid_amount: 0,
            reconciliation_status: ReconciliationStatus::Unreconciled,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);

        let mut p_claims: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderClaims(provider_id.clone()))
            .unwrap_or(Vec::new(&env));
        p_claims.push_back(claim_id);
        env.storage()
            .persistent()
            .set(&DataKey::ProviderClaims(provider_id), &p_claims);

        let mut pat_claims: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientClaims(patient_id.clone()))
            .unwrap_or(Vec::new(&env));
        pat_claims.push_back(claim_id);
        env.storage()
            .persistent()
            .set(&DataKey::PatientClaims(patient_id), &pat_claims);

        Ok(claim_id)
    }

    /// Adjudicate a claim. Caller must be the registered insurer bound to this claim.
    pub fn adjudicate_claim(
        env: Env,
        claim_id: u64,
        insurer_id: Address,
        approved_lines: Vec<u64>,
        denied_lines: Vec<DenialInfo>,
        approved_amount: i128,
        patient_responsibility: i128,
    ) -> Result<(), Error> {
        insurer_id.require_auth();
        Self::require_insurer(&env, &insurer_id)?;

        let mut claim: ClaimRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Claim(claim_id))
            .ok_or(Error::ClaimNotFound)?;

        if claim.insurer_id != insurer_id {
            return Err(Error::NotAuthorized);
        }

        if claim.status != ClaimStatus::Submitted && claim.status != ClaimStatus::Appealed {
            return Err(Error::InvalidStateTransition);
        }
        Self::validate_adjudication_amounts(
            claim.total_amount,
            approved_amount,
            patient_responsibility,
        )?;

        claim.status = ClaimStatus::Adjudicated;
        claim.approved_amount = Some(approved_amount);
        claim.patient_responsibility = Some(patient_responsibility);
        claim.insurer_paid_amount = 0;
        claim.patient_paid_amount = 0;
        claim.reconciliation_status = if approved_amount == 0 && patient_responsibility == 0 {
            ReconciliationStatus::Reconciled
        } else {
            ReconciliationStatus::Unreconciled
        };

        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);
        env.storage()
            .persistent()
            .set(&DataKey::ApprovedLines(claim_id), &approved_lines);
        env.storage()
            .persistent()
            .set(&DataKey::DenialInfos(claim_id), &denied_lines);
        env.storage().persistent().set(
            &DataKey::ClaimPayment(claim_id),
            &Vec::<InsurerPaymentRecord>::new(&env),
        );
        env.storage().persistent().set(
            &DataKey::PatientPayment(claim_id),
            &Vec::<PatientPaymentRecord>::new(&env),
        );

        Ok(())
    }

    pub fn appeal_denial(
        env: Env,
        claim_id: u64,
        provider_id: Address,
        appeal_level: u32,
        _appeal_details_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let mut claim = Self::load_claim(&env, claim_id)?;
        if claim.provider_id != provider_id {
            return Err(Error::NotAuthorized);
        }
        if claim.status != ClaimStatus::Adjudicated {
            return Err(Error::InvalidStateTransition);
        }
        if appeal_level <= claim.appeal_level || appeal_level > 3 {
            return Err(Error::InvalidAppealLevel);
        }

        claim.status = ClaimStatus::Appealed;
        claim.appeal_level = appeal_level;
        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);

        Ok(claim_id)
    }

    /// Process payment. Caller must be the registered insurer bound to this claim.
    pub fn process_payment(
        env: Env,
        claim_id: u64,
        insurer_id: Address,
        payment_amount: i128,
        payment_date: u64,
        payment_reference_hash: BytesN<32>,
    ) -> Result<(), Error> {
        insurer_id.require_auth();
        Self::require_insurer(&env, &insurer_id)?;
        let mut claim = Self::load_claim(&env, claim_id)?;

        if payment_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        if claim.insurer_id != insurer_id {
            return Err(Error::NotAuthorized);
        }

        if claim.status != ClaimStatus::Adjudicated {
            return Err(Error::InvalidStateTransition);
        }

        let approved_amount = claim.approved_amount.ok_or(Error::InvalidStateTransition)?;
        let insurer_outstanding = Self::checked_sub(approved_amount, claim.insurer_paid_amount)?;
        if payment_amount > insurer_outstanding {
            return Err(Error::InvalidAmount);
        }

        claim.insurer_paid_amount = Self::checked_add(claim.insurer_paid_amount, payment_amount)?;
        let (insurer_due, patient_due) = Self::refresh_reconciliation_status(&mut claim)?;
        if insurer_due == 0 {
            claim.status = if patient_due == 0 {
                ClaimStatus::Closed
            } else {
                ClaimStatus::Paid
            };
        }

        let mut payments: Vec<InsurerPaymentRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::ClaimPayment(claim_id))
            .unwrap_or(Vec::new(&env));
        payments.push_back(InsurerPaymentRecord {
            payment_date,
            payment_amount,
            payment_reference_hash,
        });

        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);
        env.storage()
            .persistent()
            .set(&DataKey::ClaimPayment(claim_id), &payments);

        Ok(())
    }

    pub fn apply_patient_payment(
        env: Env,
        claim_id: u64,
        patient_id: Address,
        payment_amount: i128,
        payment_date: u64,
    ) -> Result<(), Error> {
        patient_id.require_auth();

        if payment_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut claim = Self::load_claim(&env, claim_id)?;
        if claim.patient_id != patient_id {
            return Err(Error::NotAuthorized);
        }

        if claim.status != ClaimStatus::Paid && claim.status != ClaimStatus::Adjudicated {
            return Err(Error::InvalidStateTransition);
        }

        claim.patient_paid_amount = Self::checked_add(claim.patient_paid_amount, payment_amount)?;
        let (insurer_due, patient_due) = Self::refresh_reconciliation_status(&mut claim)?;
        if insurer_due == 0 && patient_due == 0 {
            claim.status = ClaimStatus::Closed;
        } else if insurer_due == 0 {
            claim.status = ClaimStatus::Paid;
        }

        let mut payments: Vec<PatientPaymentRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::PatientPayment(claim_id))
            .unwrap_or(Vec::new(&env));
        payments.push_back(PatientPaymentRecord {
            payment_date,
            payment_amount,
        });

        env.storage()
            .persistent()
            .set(&DataKey::Claim(claim_id), &claim);
        env.storage()
            .persistent()
            .set(&DataKey::PatientPayment(claim_id), &payments);

        Ok(())
    }

    pub fn get_claim(env: Env, claim_id: u64) -> Result<ClaimRecord, Error> {
        Self::load_claim(&env, claim_id)
    }

    pub fn get_insurer_payments(env: Env, claim_id: u64) -> Vec<InsurerPaymentRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::ClaimPayment(claim_id))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_patient_payments(env: Env, claim_id: u64) -> Vec<PatientPaymentRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::PatientPayment(claim_id))
            .unwrap_or(Vec::new(&env))
    }

    fn load_claim(env: &Env, claim_id: u64) -> Result<ClaimRecord, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Claim(claim_id))
            .ok_or(Error::ClaimNotFound)
    }

    fn validate_claim_amounts(
        service_codes: &Vec<ServiceLine>,
        total_amount: i128,
    ) -> Result<(), Error> {
        if total_amount < 0 || service_codes.is_empty() {
            return Err(Error::InvalidAmount);
        }

        let mut computed_total = 0_i128;
        for line in service_codes.iter() {
            if line.quantity == 0 || line.charge_amount < 0 {
                return Err(Error::InvalidAmount);
            }
            computed_total = Self::checked_add(computed_total, line.charge_amount)?;
        }

        if computed_total != total_amount {
            return Err(Error::InvalidAmount);
        }
        Ok(())
    }

    fn validate_adjudication_amounts(
        total_amount: i128,
        approved_amount: i128,
        patient_responsibility: i128,
    ) -> Result<(), Error> {
        if total_amount < 0 || approved_amount < 0 || patient_responsibility < 0 {
            return Err(Error::InvalidAmount);
        }

        let allocated = Self::checked_add(approved_amount, patient_responsibility)?;
        if allocated > total_amount {
            return Err(Error::InvalidAmount);
        }
        Ok(())
    }

    fn refresh_reconciliation_status(claim: &mut ClaimRecord) -> Result<(i128, i128), Error> {
        let approved_amount = claim.approved_amount.unwrap_or(0);
        let patient_responsibility = claim.patient_responsibility.unwrap_or(0);

        let insurer_due = Self::checked_sub(approved_amount, claim.insurer_paid_amount)?;
        let patient_due = Self::checked_sub(patient_responsibility, claim.patient_paid_amount)?;

        claim.reconciliation_status = if insurer_due == 0 && patient_due == 0 {
            ReconciliationStatus::Reconciled
        } else if claim.insurer_paid_amount > 0 || claim.patient_paid_amount > 0 {
            ReconciliationStatus::PartiallyReconciled
        } else {
            ReconciliationStatus::Unreconciled
        };

        Ok((insurer_due, patient_due))
    }

    fn checked_add(lhs: i128, rhs: i128) -> Result<i128, Error> {
        lhs.checked_add(rhs).ok_or(Error::AmountOverflow)
    }

    fn checked_sub(lhs: i128, rhs: i128) -> Result<i128, Error> {
        lhs.checked_sub(rhs)
            .filter(|value| *value >= 0)
            .ok_or(Error::InvalidAmount)
    }
}
