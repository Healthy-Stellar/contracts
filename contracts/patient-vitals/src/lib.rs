#![no_std]

mod contract;
mod types;

#[cfg(test)]
mod test;

pub use crate::contract::{PatientVitalsContract, PatientVitalsContractClient};
