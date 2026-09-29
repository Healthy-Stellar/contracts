use shared_contracts::safe_increment;
use soroban_sdk::{Address, Env, Symbol};

use crate::types::*;

// Storage keys
const COUNTER: Symbol = Symbol::short("COUNTER");
const APPT_CTR: Symbol = Symbol::short("APPT_CTR");
const PLAN: Symbol = Symbol::short("PLAN");
const ASSESS: Symbol = Symbol::short("ASSESS");
const ORDERS: Symbol = Symbol::short("ORDERS");
const HOME_HLT: Symbol = Symbol::short("HOME_HLT");
const DME: Symbol = Symbol::short("DME");
const APPT: Symbol = Symbol::short("APPT");
const EDU: Symbol = Symbol::short("EDU");
const SNF: Symbol = Symbol::short("SNF");
const COMPLETE: Symbol = Symbol::short("COMPLETE");
const RISK: Symbol = Symbol::short("RISK");

// Counter management
pub fn get_and_increment_counter(env: &Env) -> u64 {
    safe_increment(env, &COUNTER)
}

pub fn get_and_increment_appointment_counter(env: &Env) -> u64 {
    safe_increment(env, &APPT_CTR)
}

// Discharge Plan storage
pub fn save_discharge_plan(env: &Env, plan_id: u64, plan: &DischargePlan) {
    env.storage().persistent().set(&(PLAN, plan_id), plan);
}

pub fn get_discharge_plan(env: &Env, plan_id: u64) -> Result<DischargePlan, Error> {
    env.storage()
        .persistent()
        .get(&(PLAN, plan_id))
        .ok_or(Error::PlanNotFound)
}

// Readiness Assessment storage
pub fn save_readiness_assessment(env: &Env, plan_id: u64, assessment: &ReadinessScore) {
    env.storage()
        .persistent()
        .set(&(ASSESS, plan_id), assessment);
}

pub fn get_readiness_assessment(env: &Env, plan_id: u64) -> Result<ReadinessScore, Error> {
    env.storage()
        .persistent()
        .get(&(ASSESS, plan_id))
        .ok_or(Error::PlanNotFound)
}

// Discharge Orders storage
pub fn save_discharge_orders(env: &Env, plan_id: u64, orders: &DischargeOrders) {
    env.storage().persistent().set(&(ORDERS, plan_id), orders);
}

// Home Health Arrangement storage
pub fn save_home_health_arrangement(env: &Env, plan_id: u64, arrangement: &HomeHealthArrangement) {
    env.storage()
        .persistent()
        .set(&(HOME_HLT, plan_id), arrangement);
}

// DME Order storage
pub fn save_dme_order(env: &Env, plan_id: u64, order: &DMEOrder) {
    env.storage().persistent().set(&(DME, plan_id), order);
}

// Follow-up Appointment storage
pub fn save_followup_appointment(
    env: &Env,
    plan_id: u64,
    appt_id: u64,
    appointment: &FollowUpAppointment,
) {
    env.storage()
        .persistent()
        .set(&(APPT, plan_id, appt_id), appointment);
}

// Discharge Education storage
pub fn save_discharge_education(env: &Env, plan_id: u64, education: &DischargeEducation) {
    env.storage().persistent().set(&(EDU, plan_id), education);
}

// SNF Coordination storage
pub fn save_snf_coordination(env: &Env, plan_id: u64, coordination: &SNFCoordination) {
    env.storage()
        .persistent()
        .set(&(SNF, plan_id), coordination);
}

// Discharge Completion storage
pub fn save_discharge_completion(env: &Env, plan_id: u64, completion: &DischargeCompletion) {
    env.storage()
        .persistent()
        .set(&(COMPLETE, plan_id), completion);
}

// Readmission Risk storage
pub fn save_readmission_risk(env: &Env, plan_id: u64, risk: &ReadmissionRisk) {
    env.storage().persistent().set(&(RISK, plan_id), risk);
}

// Hospital Registry storage
pub fn set_hospital_registry(env: &Env, registry: &Address) {
    env.storage().instance().set(&DataKey::HospitalRegistry, registry);
}

pub fn get_hospital_registry(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::HospitalRegistry)
        .ok_or(Error::Unauthorized)
}

// Initialization guard
pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::HospitalRegistry)
}
