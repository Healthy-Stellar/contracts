#![no_std]

//! # Patient Vitals Contract
//!
//! Tracks patient vital signs (heart rate, blood pressure, temperature, etc.) with continuous
//! monitoring, abnormal value alerts, and clinical decision support.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Patient or authorized provider authentication for vital recording.
//! Real-time monitoring devices validated before data acceptance. Vital access restricted to patient
//! and authorized clinicians. Alert notifications sent to appropriate providers. Privacy controls on
//! continuous monitoring.
//!
//! **Audit Controls:** Vital sign recording events logged with measurement type, value, and timestamp.
//! Alert events emitted when abnormal values detected. Continuous monitoring events tracked.
//! Data validation failures logged for troubleshooting. Temporal ordering maintained for audit trail.
//!
//! **Data Retention Policy:** Vital sign history retained indefinitely for trend analysis. Abnormal
//! values flagged for clinical review. Alert records retained with response tracking. Continuous
//! monitoring logs archived. Deregistration removes patient vital records.
//!
//! **Encryption/Integrity:** Vital sign values stored encrypted. Timestamp immutable once recorded.
//! Value range validation prevents invalid measurements. Reference ranges enforced per vital type.
//! Abnormal thresholds configured and immutable.

mod contract;
mod types;

#[cfg(test)]
mod test;

pub use crate::contract::{PatientVitalsContract, PatientVitalsContractClient};
