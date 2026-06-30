#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Telemedicine Contract
//!
//! Manages virtual healthcare visits with appointment scheduling, video session management, digital
//! prescriptions, and remote monitoring for telehealth delivery.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Patient and provider authentication for visits. Secure video
//! session routing via HIPAA-compliant endpoint. Patient consent for telehealth record creation.
//! Provider credentials validated. Session encryption enforced. Access limited to participants.
//!
//! **Audit Controls:** Appointment scheduling events logged with provider and visit type.
//! Session start/end events tracked with duration. Prescription event logging. Recording consent
//! and actual recording tracked. Billing event capture. Failed connection attempts logged.
//!
//! **Data Retention Policy:** Visit records retained indefinitely for continuity. Session
//! duration logged for billing. Prescriptions issued during visits linked to visit. Recording
//! metadata retained (consent, access, deletion). Patient deregistration removes visit records.
//!
//! **Encryption/Integrity:** Video session data encrypted end-to-end. Prescription data encrypted
//! in persistent storage. Patient-provider linkage encrypted. Session endpoint validated.
//! Recording consent immutable. Patient authorization required for data retention.

pub mod contract;
pub mod test;
pub mod types;
#[cfg(test)]
pub mod integration_test;
