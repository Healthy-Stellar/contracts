# Changelog

All notable changes to contracts in this workspace are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### prior-authorization

#### Added
- `initialize(insurer_registry_id)` stores the insurer-registry contract used for coverage validation (#526).
- `ServiceNotCovered` error when requested procedure codes are absent from all active insurer coverage plans.

#### Changed
- **BREAKING:** `submit_prior_authorization` now requires `insurer_wallet: Address` and must be called only after `initialize`.
- Authorization submission cross-checks `InsurerRegistryInterface::get_coverage_plans` before accepting a request (#526).

### medical-claims

#### Added
- `InsurerNotActive` error when submitting a claim against an insurer with expired or revoked credentials (#527).
- `initialize` now accepts `insurer_registry_id` for on-chain credential verification.

#### Changed
- **BREAKING:** `initialize` signature extended with `insurer_registry_id: Address` (#527).
- `submit_claim` calls `InsurerRegistryInterface::is_insurer_active` before creating a claim record.

### insurer-registry

#### Added
- `CoveragePlan` struct and `CoveragePlans` persistent storage key.
- `set_coverage_plans` / `get_coverage_plans` for structured benefit allowlists consumed by prior-authorization (#526).

### patient-registry

#### Changed
- TTL extension delegates to `ttl-config` critical/operational helpers based on `RetentionClass` (Clinical/Financial → critical, Administrative → operational).

### pacs-integration

#### Changed
- All persistent imaging keys use critical-class TTL bump constants from `ttl-config`.

### allergy-management

#### Changed
- All allergy and access-control persistent keys use `extend_critical_ttl` helpers from `ttl-config`.

## [2026-03-01]

### access-control

#### Added
- `check_consent` cross-contract API consumed by medical-claims for HIPAA consent verification (#300).

### medical-claims

#### Added
- Payment reconciliation with financial-records contract (#392).
- Consent gate on `submit_claim` via access-control integration (#300).
