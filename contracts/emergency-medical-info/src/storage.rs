use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, RecoveryConfig, RecoveryProposal};

pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_recovery_config(env: &Env) -> Option<RecoveryConfig> {
    env.storage().instance().get(&DataKey::RecoveryConfig)
}

pub fn set_recovery_config(env: &Env, config: &RecoveryConfig) {
    env.storage().instance().set(&DataKey::RecoveryConfig, config);
}

pub fn get_recovery_proposal(env: &Env, patient: &Address) -> Option<RecoveryProposal> {
    env.storage()
        .persistent()
        .get(&DataKey::RecoveryProposal(patient.clone()))
}

pub fn set_recovery_proposal(env: &Env, patient: &Address, proposal: &RecoveryProposal) {
    env.storage()
        .persistent()
        .set(&DataKey::RecoveryProposal(patient.clone()), proposal);
}

pub fn remove_recovery_proposal(env: &Env, patient: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::RecoveryProposal(patient.clone()));
}

pub fn get_guardians(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::Guardians)
        .unwrap_or(Vec::new(env))
}

pub fn set_guardians(env: &Env, guardians: &Vec<Address>) {
    env.storage().instance().set(&DataKey::Guardians, guardians);
}
