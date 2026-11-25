use shared::counters::safe_increment;
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
const ORDERS_CTR: Symbol = Symbol::short("ORD_CTR");
const HH_CTR: Symbol = Symbol::short("HH_CTR");
const DME_CTR: Symbol = Symbol::short("DME_CTR");
const EDU_CTR: Symbol = Symbol::short("EDU_CTR");
const SNF_CTR: Symbol = Symbol::short("SNF_CTR");
const RISK_CTR: Symbol = Symbol::short("RISK_CTR");
const CARE_TEAM: Symbol = Symbol::short("CARE_TEAM");
const OVERRIDE: Symbol = Symbol::short("OVERRIDE");

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

// Discharge Orders storage (append-only history)
pub fn save_discharge_orders(env: &Env, plan_id: u64, orders: &DischargeOrders) -> u64 {
    let seq = safe_increment(env, &(ORDERS_CTR, plan_id));
    env.storage()
        .persistent()
        .set(&(ORDERS, plan_id, seq), orders);
    seq
}

pub fn get_discharge_orders(env: &Env, plan_id: u64, seq: u64) -> Result<DischargeOrders, Error> {
    env.storage()
        .persistent()
        .get(&(ORDERS, plan_id, seq))
        .ok_or(Error::PlanNotFound)
}

pub fn get_latest_discharge_orders(
    env: &Env,
    plan_id: u64,
) -> Result<DischargeOrders, Error> {
    let seq = get_orders_count(env, plan_id);
    if seq == 0 {
        return Err(Error::PlanNotFound);
    }
    get_discharge_orders(env, plan_id, seq)
}

pub fn get_orders_count(env: &Env, plan_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&(ORDERS_CTR, plan_id))
        .unwrap_or(0)
}

// Home Health Arrangement storage (append-only list)
pub fn save_home_health_arrangement(
    env: &Env,
    plan_id: u64,
    arrangement: &HomeHealthArrangement,
) -> u64 {
    let seq = safe_increment(env, &(HH_CTR, plan_id));
    env.storage()
        .persistent()
        .set(&(HOME_HLT, plan_id, seq), arrangement);
    seq
}

pub fn get_home_health_arrangement(
    env: &Env,
    plan_id: u64,
    seq: u64,
) -> Result<HomeHealthArrangement, Error> {
    env.storage()
        .persistent()
        .get(&(HOME_HLT, plan_id, seq))
        .ok_or(Error::PlanNotFound)
}

pub fn get_home_health_count(env: &Env, plan_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&(HH_CTR, plan_id))
        .unwrap_or(0)
}

// DME Order storage (append-only list)
pub fn save_dme_order(env: &Env, plan_id: u64, order: &DMEOrder) -> u64 {
    let seq = safe_increment(env, &(DME_CTR, plan_id));
    env.storage().persistent().set(&(DME, plan_id, seq), order);
    seq
}

pub fn get_dme_order(env: &Env, plan_id: u64, seq: u64) -> Result<DMEOrder, Error> {
    env.storage()
        .persistent()
        .get(&(DME, plan_id, seq))
        .ok_or(Error::PlanNotFound)
}

pub fn get_dme_count(env: &Env, plan_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&(DME_CTR, plan_id))
        .unwrap_or(0)
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

// Discharge Education storage (append-only list)
pub fn save_discharge_education(
    env: &Env,
    plan_id: u64,
    education: &DischargeEducation,
) -> u64 {
    let seq = safe_increment(env, &(EDU_CTR, plan_id));
    env.storage()
        .persistent()
        .set(&(EDU, plan_id, seq), education);
    seq
}

pub fn get_discharge_education(
    env: &Env,
    plan_id: u64,
    seq: u64,
) -> Result<DischargeEducation, Error> {
    env.storage()
        .persistent()
        .get(&(EDU, plan_id, seq))
        .ok_or(Error::PlanNotFound)
}

pub fn get_education_count(env: &Env, plan_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&(EDU_CTR, plan_id))
        .unwrap_or(0)
}

// SNF Coordination storage (append-only list)
pub fn save_snf_coordination(
    env: &Env,
    plan_id: u64,
    coordination: &SNFCoordination,
) -> u64 {
    let seq = safe_increment(env, &(SNF_CTR, plan_id));
    env.storage()
        .persistent()
        .set(&(SNF, plan_id, seq), coordination);
    seq
}

pub fn get_snf_coordination(
    env: &Env,
    plan_id: u64,
    seq: u64,
) -> Result<SNFCoordination, Error> {
    env.storage()
        .persistent()
        .get(&(SNF, plan_id, seq))
        .ok_or(Error::PlanNotFound)
}

pub fn get_snf_count(env: &Env, plan_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&(SNF_CTR, plan_id))
        .unwrap_or(0)
}

// Discharge Completion storage
pub fn save_discharge_completion(env: &Env, plan_id: u64, completion: &DischargeCompletion) {
    env.storage()
        .persistent()
        .set(&(COMPLETE, plan_id), completion);
}

pub fn get_discharge_completion(
    env: &Env,
    plan_id: u64,
) -> Result<DischargeCompletion, Error> {
    env.storage()
        .persistent()
        .get(&(COMPLETE, plan_id))
        .ok_or(Error::PlanNotFound)
}

// Readmission Risk storage (append-only history)
pub fn save_readmission_risk(env: &Env, plan_id: u64, risk: &ReadmissionRisk) -> u64 {
    let seq = safe_increment(env, &(RISK_CTR, plan_id));
    env.storage().persistent().set(&(RISK, plan_id, seq), risk);
    seq
}

pub fn get_readmission_risk(
    env: &Env,
    plan_id: u64,
    seq: u64,
) -> Result<ReadmissionRisk, Error> {
    env.storage()
        .persistent()
        .get(&(RISK, plan_id, seq))
        .ok_or(Error::PlanNotFound)
}

pub fn get_latest_readmission_risk(
    env: &Env,
    plan_id: u64,
) -> Result<ReadmissionRisk, Error> {
    let seq = get_risk_count(env, plan_id);
    if seq == 0 {
        return Err(Error::PlanNotFound);
    }
    get_readmission_risk(env, plan_id, seq)
}

pub fn get_risk_count(env: &Env, plan_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get(&(RISK_CTR, plan_id))
        .unwrap_or(0)
}

// Care team storage (per-plan list of authorized providers)
pub fn save_care_team(env: &Env, plan_id: u64, team: &soroban_sdk::Vec<Address>) {
    env.storage().persistent().set(&(CARE_TEAM, plan_id), team);
}

pub fn get_care_team(env: &Env, plan_id: u64) -> soroban_sdk::Vec<Address> {
    env.storage()
        .persistent()
        .get(&(CARE_TEAM, plan_id))
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

pub fn is_care_team_member(env: &Env, plan_id: u64, caller: &Address) -> bool {
    get_care_team(env, plan_id).contains(caller)
}

// Admin override records (reassignment / cancellation of stuck plans)
pub fn save_override(env: &Env, plan_id: u64, record: &OverrideRecord) {
    env.storage()
        .persistent()
        .set(&(OVERRIDE, plan_id), record);
}

pub fn get_override(env: &Env, plan_id: u64) -> Option<OverrideRecord> {
    env.storage().persistent().get(&(OVERRIDE, plan_id))
}

// Hospital Registry storage
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn set_hospital_registry(env: &Env, registry: &Address) {
    env.storage().instance().set(&DataKey::HospitalRegistry, registry);
}

pub fn get_hospital_registry(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::HospitalRegistry)
        .ok_or(Error::Unauthorized)
}

pub fn set_provider_registry(env: &Env, registry: &Address) {
    env.storage().instance().set(&DataKey::ProviderRegistry, registry);
}

pub fn get_provider_registry(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::ProviderRegistry)
        .ok_or(Error::Unauthorized)
}

