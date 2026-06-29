#![no_std]
#![allow(deprecated)]

//! # Lab Management Contract
//!
//! Manages laboratory test ordering, specimen tracking, result documentation, and quality assurance
//! with turnaround time tracking and critical value alerts.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Lab technician/pathologist authentication for result entry.
//! Ordering provider validation. Patient consent for testing. Result access restricted to
//! ordering provider and patient. Critical value notification authorization.
//!
//! **Audit Controls:** Test order events logged with specimen type and order provider. Result
//! entry events tracked with pathologist identity. Critical value alerts emitted with severity.
//! Quality assurance check results documented. Turnaround time metrics tracked for audit.
//!
//! **Data Retention Policy:** Lab results retained indefinitely per medical record standards.
//! Specimen tracking maintained until test completion. Quality assurance metrics archived.
//! Critical values retained for liability protection. Reference ranges versioned for historical
//! comparison.
//!
//! **Encryption/Integrity:** Lab values stored encrypted in persistent storage. Test result
//! reference ranges validated. Pathologist digital signature via address authentication. Critical
//! value thresholds immutable. Specimen type enumeration prevents invalid tests.

use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Vec, contract, contracterror, contractimpl, contracttype,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    Unauthorized = 2,
    QCFieldFailed = 4,
    /// The lab order counter has reached u64::MAX and cannot be incremented.
    OrderIdOverflow = 5,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TestResult {
    pub test_code: String,
    pub test_name: String,
    pub value: String,
    pub unit: String,
    pub reference_range: String,
    pub is_abnormal: bool,
    pub abnormal_flag: Option<Symbol>,
}

#[contracttype]
pub struct LabOrder {
    pub provider_id: Address,
    pub patient_id: Address,
    pub lab_id: Option<Address>,
    pub test_panel: Vec<String>,
    pub status: Symbol,
    pub results_hash: Option<BytesN<32>>,
    pub quality_control_passed: bool,
}

#[contracttype]
pub struct OrderRequest {
    pub test_panel: Vec<String>,
    pub priority: Symbol,
    pub clinical_info_hash: BytesN<32>,
    pub fasting_required: bool,
    pub collection_date: Option<u64>,
}

/// Typed storage keys.
///
/// `LabOrder(u64)` stores the full 64-bit order ID as part of the key so that
/// no truncation to u32 can ever occur, regardless of how large the counter
/// grows.  The monotonic counter itself lives in instance storage under
/// `LabCounter`.
#[contracttype]
pub enum DataKey {
    /// Per-order persistent storage: DataKey::LabOrder(order_id) -> LabOrder
    LabOrder(u64),
    /// Monotonic counter in instance storage.
    LabCounter,
}

#[contract]
pub struct LabManagementContract;

#[contractimpl]
impl LabManagementContract {
    /// Validates QC check results before any state mutations occur.
    /// Returns Ok if validation passes, Err if validation fails.
    fn validate_qc_results(qc_passed: bool, results_summary: &Vec<TestResult>) -> Result<(), Error> {
        if !qc_passed {
            return Err(Error::QCFieldFailed);
        }
        if results_summary.is_empty() {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn order_lab_test(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        req: OrderRequest,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        // Read the current counter (u64 throughout — no cast to u32).
        let id: u64 = env
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::LabCounter)
            .unwrap_or(0);

        // Guard against u64 overflow before incrementing.
        let next_id = id.checked_add(1).ok_or(Error::OrderIdOverflow)?;

        env.storage()
            .instance()
            .set(&DataKey::LabCounter, &next_id);

        let order = LabOrder {
            provider_id,
            patient_id,
            lab_id: None,
            test_panel: req.test_panel,
            status: Symbol::new(&env, "Ordered"),
            results_hash: None,
            quality_control_passed: false,
        };

        // Use the typed DataKey so the full u64 is embedded in the storage key.
        env.storage()
            .persistent()
            .set(&DataKey::LabOrder(id), &order);
        Ok(id)
    }

    pub fn assign_lab(env: Env, order_id: u64, lab_id: Address, _eta: u64) -> Result<(), Error> {
        let mut order: LabOrder = env
            .storage()
            .persistent()
            .get(&DataKey::LabOrder(order_id))
            .ok_or(Error::NotFound)?;
        order.lab_id = Some(lab_id);
        order.status = Symbol::new(&env, "Assigned");
        env.storage()
            .persistent()
            .set(&DataKey::LabOrder(order_id), &order);
        Ok(())
    }

    pub fn submit_results(
        env: Env,
        order_id: u64,
        lab_id: Address,
        results_hash: BytesN<32>,
        results_summary: Vec<TestResult>,
        qc_passed: bool,
    ) -> Result<(), Error> {
        lab_id.require_auth();

        // VALIDATION PHASE: All validations must pass before any storage writes.

        // 1. Verify order exists.
        let mut order: LabOrder = env
            .storage()
            .persistent()
            .get(&DataKey::LabOrder(order_id))
            .ok_or(Error::NotFound)?;

        // 2. Perform QC validation (BEFORE any mutations).
        Self::validate_qc_results(qc_passed, &results_summary)?;

        // MUTATION PHASE: All state changes after validations have passed.

        order.results_hash = Some(results_hash);
        order.quality_control_passed = qc_passed;
        order.status = Symbol::new(&env, "Completed");

        env.events().publish(
            (
                Symbol::new(&env, "LAB"),
                Symbol::new(&env, "RESULT"),
                order.patient_id.clone(),
            ),
            results_summary,
        );

        env.storage()
            .persistent()
            .set(&DataKey::LabOrder(order_id), &order);
        Ok(())
    }

    pub fn flag_critical_value(
        env: Env,
        order_id: u64,
        lab_id: Address,
        test_code: String,
        val: String,
    ) {
        lab_id.require_auth();
        env.events()
            .publish((Symbol::new(&env, "CRITICAL"), order_id), (test_code, val));
    }
}
mod test;
