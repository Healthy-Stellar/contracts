# Changelog

All notable changes to contracts in this workspace are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### workspace — `initialize` replaced by `__constructor` (#949)

#### Changed
- **BREAKING:** Contracts no longer expose a public `initialize`/`init` entry point.
  Initial configuration is now bound atomically at contract creation through
  `pub fn __constructor(env: Env, …)`, which runs once as part of `deploy` and
  cannot be called again. This closes the front-running hijack where an
  attacker watched a deploy transaction and called `initialize` first with an
  admin address they controlled — `admin.require_auth()` could not prevent that
  because the attacker simply signed for the address they supplied.
- Deployment commands must now pass constructor arguments after `--`, e.g.
  `stellar contract deploy --wasm … -- --admin G…`.
  `scripts/deploy_all.sh` forwards `DEPLOY_CONSTRUCTOR_ARGS` for this purpose.
- Deleted the `AlreadyInitialized` guard and the double-initialize tests that
  are now unreachable; `NotInitialized` guards remain as defensive checks.

#### Migrated in this change
`nft-badges`, `zk-eligibility-verifier`, `zk-eligibility`, `liquidity-pool`,
`governance-voting`, `scholarship-fund`, `lab-management`, `health-records`,
`mental-health`, `multisig-governance`, `upgrade-governance`,
`hospital-registry`, `healthcare-analytics`, `clinical-guideline`,
`clinical-trial`, `allergy-management`, `allergy-tracking`,
`medical-device-tracking`, `medical-claims`, `prior-authorization`,
`immunization-registry`, `prenatal-pediatric`, and the root `HealthcareRegistry`.

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
