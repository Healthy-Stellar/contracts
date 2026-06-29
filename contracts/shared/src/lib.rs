#![no_std]

//! # Shared Library
//!
//! Common utilities and modules shared across healthcare smart contracts including incident tracking,
//! privacy controls, pagination, temporal validation, and actor verification.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Actor verification module validates authorization across contracts.
//! Privacy controls enforce encryption policies. Role-based access control validation. Address
//! validation prevents unauthorized access. Pause mechanisms for emergency contract stopping.
//!
//! **Audit Controls:** Incident tracking with severity levels and correlation IDs. Error hints
//! for diagnostics. Event versioning for schema identification. Temporal utilities for audit trail
//! sequencing. All cross-contract incident linking via correlation IDs.
//!
//! **Data Retention Policy:** Incident records retained indefinitely per contract requirements.
//! Error logging for troubleshooting. Event history maintained. Pagination cursors for scalable
//! audit queries. Temporal data archived with timestamps.
//!
//! **Encryption/Integrity:** Privacy policy metadata enforces encryption per record type.
//! Encrypted envelope references with content hashing. Address validation via nonzero checks.
//! Incident correlation IDs enable secure cross-contract audit trails. Cryptographic hashing
//! for integrity verification.

pub mod actor_verification;
#[cfg(test)]
pub mod test_utils;
pub mod error_hints;
pub mod events;
pub mod incident_tracking;
pub mod pagination;
#[cfg(test)]
mod pagination_stability_tests;
pub mod pause;
pub mod privacy;
pub mod resource_management;
pub mod temporal;
#[cfg(test)]
mod temporal_tests;
