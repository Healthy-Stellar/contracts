//! # Patient Registry Benchmarks
//!
//! Instruction metering benchmarks for patient registry contract with performance profiling
//! of major contract functions for gas cost optimization.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Benchmark module for testing purposes only; no direct access
//! control. Measures instruction consumption for registry operations. Performance profiling
//! ensures contract efficiency. Does not expose patient data or credentials.
//!
//! **Audit Controls:** Instruction metering provides audit trail of contract complexity.
//! Performance metrics logged for optimization tracking. Cost analysis enables budget planning.
//! Regression testing prevents unexpected cost increases.
//!
//! **Data Retention Policy:** Benchmark results retained for performance comparison. Historical
//! cost data maintained for trend analysis. Optimization changes tracked with metrics.
//!
//! **Encryption/Integrity:** No sensitive data accessed in benchmarks. Instruction counts
//! immutable once measured. Test data encrypted if present. Performance reproducible across
//! compatible Soroban versions.

pub use patient_registry::{MedicalRegistry, MedicalRegistryClient};
