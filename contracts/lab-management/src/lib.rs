#![no_std]

use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Vec, contract, contracterror, contractimpl, contracttype,
    panic_with_error,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    Unauthorized = 2,
    QCFieldFailed = 4,
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

#[contract]
pub struct LabManagementContract;

#[contractimpl]
impl LabManagementContract {
    pub fn order_lab_test(
        env: Env,
        provider_id: Address,
        patient_id: Address,
        req: OrderRequest,
    ) -> u64 {
        provider_id.require_auth();
        let id = env
            .storage()
            .instance()
            .get::<_, u64>(&Symbol::new(&env, "LAB_ID"))
            .unwrap_or(0);

        let order = LabOrder {
            provider_id,
            patient_id,
            lab_id: None,
            test_panel: req.test_panel,
            status: Symbol::new(&env, "Ordered"),
            results_hash: None,
            quality_control_passed: false,
        };

        env.storage().persistent().set(&id, &order);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "LAB_ID"), &(id + 1));
        id
    }

    pub fn assign_lab(env: Env, order_id: u64, lab_id: Address, _eta: u64) {
        let mut order: LabOrder = env
            .storage()
            .persistent()
            .get(&order_id)
            .expect("Order not found");
        order.lab_id = Some(lab_id);
        order.status = Symbol::new(&env, "Assigned");
        env.storage().persistent().set(&order_id, &order);
    }

    pub fn submit_results(
        env: Env,
        order_id: u64,
        lab_id: Address,
        results_hash: BytesN<32>,
        results_summary: Vec<TestResult>,
        qc_passed: bool,
    ) {
        lab_id.require_auth();
        let mut order: LabOrder = env.storage().persistent().get(&order_id).expect("No Order");

        if !qc_passed {
            panic_with_error!(&env, Error::QCFieldFailed);
        }

        order.results_hash = Some(results_hash);
        order.quality_control_passed = qc_passed;
        order.status = Symbol::new(&env, "Completed");

        // Event for result submission (LOINC support)
        env.events().publish(
            (
                Symbol::new(&env, "LAB"),
                Symbol::new(&env, "RESULT"),
                order.patient_id.clone(),
            ),
            results_summary,
        );

        env.storage().persistent().set(&order_id, &order);
    }

    pub fn flag_critical_value(
        env: Env,
        order_id: u64,
        lab_id: Address,
        test_code: String,
        val: String,
    ) {
        lab_id.require_auth();
        // Event for critical alerting
        env.events()
            .publish((Symbol::new(&env, "CRITICAL"), order_id), (test_code, val));
    }
}
mod test;
