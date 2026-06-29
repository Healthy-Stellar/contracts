# On-Chain Data Retention Policies

This document records per-contract storage retention decisions for the Healthy-Stellar healthcare contract workspace. Constants are defined in `contracts/ttl-config/src/lib.rs` and summarized in `TTL_POLICY.md`.

## Retention Classes

| Class | Bump (ledgers) | Threshold (ledgers) | ~Calendar equivalence* | Bump policy |
|-------|----------------|---------------------|------------------------|-------------|
| **Critical** | 535,680 | 518,400 | ~31 days | Bump on every write; bump on read for active clinical records |
| **Operational** | 120,960 | 60,480 | ~7 days | Bump on write; optional bump on read |
| **Ephemeral** | 17,280 | 8,640 | ~1 day | Bump on write only |

*Assumes ~5 seconds per ledger, per `ttl-config` documentation.

## HIPAA Alignment Summary

| Class | Justification |
|-------|---------------|
| Critical | PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment. |
| Operational | Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure. |
| Ephemeral | Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent. |

## Contract Summary

| Contract | Retention class | Bump amount | Threshold | TTL helper | Status |
|----------|-----------------|-------------|-----------|------------|--------|
| `access-control` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `allergy-management` | Critical | 535,680 (~31.0d) | 518,400 | `extend_critical_ttl / extend_critical_ttl_if_exists` | Implemented |
| `allergy-tracking` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `care-plan` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `clinical-guideline` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `clinical-trial` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `dental-records` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `doctor-registry` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `emergency-medical-info` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `financial-records` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `governance-voting` | Ephemeral | 17,280 (~1.0d) | 8,640 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `hai-tracking` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `health-records` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `healthcare-analytics` | Ephemeral | 17,280 (~1.0d) | 8,640 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `healthcare-credentialing` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `hospital-discharge-management` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `hospital-registry` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `imaging-radiology` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `immunization-registry` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `insurer-registry` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `lab-management` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `liquidity-pool` | Ephemeral | 17,280 (~1.0d) | 8,640 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `medical-claims` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `medical-device-tracking` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `mental-health` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `multisig-governance` | Ephemeral | 17,280 (~1.0d) | 8,640 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `nft-badges` | Ephemeral | 17,280 (~1.0d) | 8,640 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `nutrition-care-management` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `pacs-integration` | Critical | 535,680 (~31.0d) | 518,400 | `extend_ttl with critical constants` | Implemented |
| `patient-registry` | Critical + Operational | 535,680 (~31.0d) | 518,400 | `extend_critical_ttl_if_exists / extend_operational_ttl_if_exists` | Implemented |
| `patient-vitals` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `prenatal-pediatric` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `prescription-management` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `prior-authorization` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `provider-registry` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `referral` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `rehabilitation-services` | Critical | 535,680 (~31.0d) | 518,400 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `scholarship-fund` | Ephemeral | 17,280 (~1.0d) | 8,640 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `telemedicine` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `upgrade-governance` | Ephemeral | 17,280 (~1.0d) | 8,640 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `zk-eligibility` | Ephemeral | 17,280 (~1.0d) | 8,640 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |
| `zk-eligibility-verifier` | Operational | 120,960 (~7.0d) | 60,480 | `Not yet implemented — follow TTL_POLICY.md migration guide` | Recommended |

## Per-Contract Persistent Storage Keys

### `access-control`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `Entity(Address)` | Persistent storage entry |
| `AccessList(Address)` | Entity -> Vec<AccessPermission> |
| `ResourceAccess(String)` | Resource -> Vec<Address> (authorized parties) |
| `Did(Address)` | Persistent storage entry |
| `GrantIndex(Address, Address, String)` | Persistent storage entry |
| `OpCounter` | Persistent storage entry |
| `Commit(BytesN<32>)` | Persistent storage entry |
| `Consent(Address, Address, String)` | Persistent storage entry |
| `SubjectConsents(Address)` | Persistent storage entry |
| `RoleAssignment(Address, Role)` | Persistent storage entry |
| `RateLimit(Address, u32)` | Persistent storage entry |
| `EmergencyAccess(Address, Address)` | Persistent storage entry |

### `allergy-management`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `AllergyCounter` | Persistent storage entry |
| `Allergy(u64)` | Persistent storage entry |
| `PatientAllergies(Address)` | Persistent storage entry |
| `AccessControl(Address, Address)` | (patient, provider) |
| `CrossSensitivity(String, String)` | (allergen1, allergen2) |
| `PatientRegistry` | Persistent storage entry |
| `ProviderRegistry` | Persistent storage entry |
| `HospitalRegistry` | Persistent storage entry |
| `InsurerRegistry` | Persistent storage entry |

### `allergy-tracking`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `AllergyCounter` | Persistent storage entry |
| `Allergy(u64)` | Persistent storage entry |
| `PatientAllergies(Address)` | Persistent storage entry |
| `SeverityHistory(u64)` | Persistent storage entry |
| `DrugCrossSensitivity(String)` | Persistent storage entry |

### `care-plan`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `CarePlanCounter` | Persistent storage entry |
| `GoalCounter` | Persistent storage entry |
| `InterventionCounter` | Persistent storage entry |
| `BarrierCounter` | Persistent storage entry |
| `ReviewCounter` | Persistent storage entry |
| `CarePlan(u64)` | Persistent storage entry |
| `Goal(u64)` | Persistent storage entry |
| `Intervention(u64)` | Persistent storage entry |
| `Barrier(u64)` | Persistent storage entry |
| `Review(u64)` | Persistent storage entry |
| `PlanGoals(u64)` | Persistent storage entry |
| `PlanInterventions(u64)` | Persistent storage entry |
| `PlanBarriers(u64)` | Persistent storage entry |
| `PlanReviews(u64)` | Persistent storage entry |
| `PlanCareTeam(u64)` | Persistent storage entry |
| `PatientPlans(Address)` | Persistent storage entry |

### `clinical-guideline`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

_No `DataKey` enum found; contract may use inline keys or instance storage only._

### `clinical-trial`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `TrialCounter` | Persistent storage entry |
| `EnrollmentCounter` | Persistent storage entry |
| `EventCounter` | Persistent storage entry |
| `Trial(u64)` | Persistent storage entry |
| `Criteria(u64)` | Persistent storage entry |
| `Enrollment(u64)` | Persistent storage entry |
| `TrialEnrollments(u64)` | Persistent storage entry |
| `PatientEnrollments(Address)` | Persistent storage entry |
| `StudyVisit(u64, u32)` | Persistent storage entry |
| `AdverseEvent(u64)` | Persistent storage entry |
| `ProtocolDeviation(u64, u64)` | Persistent storage entry |
| `SafetyReport(u64, u64)` | Persistent storage entry |
| `PatientRegistry` | Persistent storage entry |
| `DsmBoard(u64)` | Persistent storage entry |
| `SafetyHalt(u64)` | Persistent storage entry |
| `SiteCounter(u64)` | Persistent storage entry |
| `Site(u64, u64)` | Persistent storage entry |
| `TrialSites(u64)` | Persistent storage entry |
| `TrialPhaseProtocol(u64)` | Persistent storage entry |

### `dental-records`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `ChartCount` | Persistent storage entry |
| `Chart(u64)` | chart_id |
| `ToothCond(u64, String)` | chart_id, tooth_number |
| `Perio(u64, String, Symbol)` | chart_id, tooth_number, site |
| `PlanCount` | Persistent storage entry |
| `Plan(u64)` | treatment_plan_id |
| `AppointmentCount` | Persistent storage entry |
| `Appt(u64)` | appointment_id |
| `ProcedureLog(u64)` | appointment_id -> log |
| `RadiographCount` | Persistent storage entry |
| `Radio(u64)` | radiograph_id |
| `OrthoCount` | Persistent storage entry |
| `Ortho(u64)` | ortho_treatment_id |
| `OrthoAdj(u64, u64)` | ortho_treatment_id, adjustment_date |
| `RxCount` | Persistent storage entry |
| `Rx(u64)` | rx_id |
| `Consent(BytesN<32>)` | Persistent storage entry |

### `doctor-registry`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `Doctor(Address)` | Persistent storage entry |

### `emergency-medical-info`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `EmergencyProfile(Address)` | Persistent storage entry |
| `CriticalAlerts(Address)` | Persistent storage entry |
| `EmergencyAccessLog(Address)` | Persistent storage entry |
| `DNROrder(Address)` | Persistent storage entry |
| `EmergencyNotifications(Address)` | Persistent storage entry |
| `RecoveryConfig(Address)` | Persistent storage entry |
| `RecoveryProposal(Address)` | Persistent storage entry |

### `financial-records`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Record(Address, u32)` | (owner, idx) -> FinancialRecord |
| `RecordCount(Address)` | owner -> u32 |
| `Access(Address, Address)` | (owner, authorized) -> bool |
| `TypeIndex(Address, u32, u32)` | (owner, record_type as u32, seq) -> record idx |
| `TypeCount(Address, u32)` | (owner, record_type as u32) -> u32 |
| `DateIndex(Address, u32)` | (owner, seq) -> record idx  (insertion order) |
| `DateCount(Address)` | owner -> u32 |

### `governance-voting`

- **Retention class:** Ephemeral
- **Bump / threshold:** 17,280 / 8,640 ledgers
- **HIPAA note:** Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `NextId` | Persistent storage entry |
| `Proposal(u64)` | Persistent storage entry |
| `Vote(u64, Address)` | (proposal_id, voter) → VoteChoice |
| `PendingAdmin` | Persistent storage entry |
| `RotationExpiry` | Persistent storage entry |

### `hai-tracking`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `InfectionCase(u64)` | Persistent storage entry |
| `OutbreakCluster(u64)` | Persistent storage entry |
| `IsolationPrecaution(u64)` | Persistent storage entry |
| `HandHygieneRecord(u64)` | Persistent storage entry |
| `StewardshipRecord(u64)` | Persistent storage entry |
| `NhsnReport(u64)` | Persistent storage entry |
| `InfectionIds` | Persistent storage entry |
| `OutbreakIds` | Persistent storage entry |
| `PrecautionIds` | Persistent storage entry |
| `WardRateConfig(Address, String, Symbol)` | Persistent storage entry |

### `health-records`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `Record(u64)` | Persistent storage entry |
| `RecordCounter` | Persistent storage entry |
| `Consent(Address, Address)` | (patient, provider) -> ConsentScope |
| `PatientProviders(Address)` | patient -> Vec<Address> of consented providers |
| `CategoryIndex(RecordCategory)` | category -> Vec<u64> of record ids in that category, for prefix-style queries |
| `ProviderRegistry` | Persistent storage entry |
| `RecordVersion(u64, u32)` | Persistent storage entry |

### `healthcare-analytics`

- **Retention class:** Ephemeral
- **Bump / threshold:** 17,280 / 8,640 ledgers
- **HIPAA note:** Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent.

| Storage key | Notes |
|-------------|-------|
| `MetricCounter` | Persistent storage entry |
| `Metric(u64)` | Persistent storage entry |
| `MetricsByType(Symbol)` | Persistent storage entry |
| `QualityMetricCounter` | Persistent storage entry |
| `QualityMetric(u64)` | Persistent storage entry |
| `QualityMetricsByProvider(Address)` | Persistent storage entry |
| `Admin` | Persistent storage entry |
| `JobResultQuality(u64)` | Persistent storage entry |
| `PendingAdmin` | Persistent storage entry |
| `RotationExpiry` | Persistent storage entry |

### `healthcare-credentialing`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `CaseCounter` | Persistent storage entry |
| `Case(u64)` | Persistent storage entry |
| `ProviderFacilityCase(Address, Address)` | Persistent storage entry |
| `CaseDocuments(u64)` | Persistent storage entry |
| `CaseVerifications(u64)` | Persistent storage entry |
| `CaseSanctions(u64)` | Persistent storage entry |
| `CasePeerReferences(u64)` | Persistent storage entry |
| `PrivilegeCounter` | Persistent storage entry |
| `ProviderFacilityPrivileges(Address, Address)` | Persistent storage entry |
| `ProvisionalCounter` | Persistent storage entry |
| `ProvisionalRequest(u64)` | Persistent storage entry |
| `ProviderFacilityProvisional(Address, Address)` | Persistent storage entry |
| `ProviderFacilityActivities(Address, Address)` | Persistent storage entry |
| `FocusedReviewCounter` | Persistent storage entry |
| `FocusedReview(u64)` | Persistent storage entry |
| `RecredentialingCounter` | Persistent storage entry |
| `Recredentialing(u64)` | Persistent storage entry |
| `ProviderFacilityRecredentialings(Address, Address)` | Persistent storage entry |
| `ProviderFacilitySuspensions(Address, Address)` | Persistent storage entry |
| `ProviderFacilityReinstatements(Address, Address)` | Persistent storage entry |
| `ActiveRecredentialingCases(Address, Address)` | Persistent storage entry |

### `hospital-discharge-management`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

_No `DataKey` enum found; contract may use inline keys or instance storage only._

### `hospital-registry`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Hospital(Address)` | Persistent storage entry |
| `HospitalConfig(Address)` | Persistent storage entry |

### `imaging-radiology`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `OrderCounter` | Persistent storage entry |
| `ImagingOrder(u64)` | Persistent storage entry |
| `ImagingSchedule(u64)` | Persistent storage entry |
| `DicomImages(u64)` | Persistent storage entry |
| `PreliminaryReport(u64)` | Persistent storage entry |
| `FinalReport(u64)` | Persistent storage entry |
| `PeerReview(u64)` | Persistent storage entry |
| `PatientOrdersPage(Address, u32)` | Persistent storage entry |
| `PatientOrdersHead(Address)` | Persistent storage entry |
| `PatientOrdersTotal(Address)` | Persistent storage entry |
| `ProviderOrdersPage(Address, u32)` | Persistent storage entry |
| `ProviderOrdersHead(Address)` | Persistent storage entry |
| `ProviderOrdersTotal(Address)` | Persistent storage entry |

### `immunization-registry`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `ImmunizationCounter` | Persistent storage entry |
| `PatientImmunizations(Address)` | List of IDs (u64) |
| `ImmunizationRecord(u64)` | Persistent storage entry |
| `AdverseEvents(u64)` | List of AdverseEvent |
| `PatientVaccineSeries(Address)` | List of VaccineSeries |

### `insurer-registry`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Insurer(Address)` | Persistent storage entry |
| `ClaimsReviewers(Address)` | Persistent storage entry |
| `CoveragePlans(Address)` | Persistent storage entry |

### `lab-management`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `LabOrder(u64)` | Persistent storage entry |
| `LabCounter` | Persistent storage entry |

### `liquidity-pool`

- **Retention class:** Ephemeral
- **Bump / threshold:** 17,280 / 8,640 ledgers
- **HIPAA note:** Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `ReserveA` | Persistent storage entry |
| `ReserveB` | Persistent storage entry |
| `TotalShares` | Persistent storage entry |
| `Shares(Address)` | Persistent storage entry |
| `PendingAdmin` | Persistent storage entry |
| `RotationExpiry` | Persistent storage entry |

### `medical-claims`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `Insurer(Address)` | insurer_id -> bool |
| `ClaimCounter` | Persistent storage entry |
| `Claim(u64)` | Persistent storage entry |
| `DenialInfos(u64)` | Persistent storage entry |
| `ApprovedLines(u64)` | Persistent storage entry |
| `ProviderClaims(Address)` | Persistent storage entry |
| `PatientClaims(Address)` | Persistent storage entry |
| `ClaimPayment(u64)` | Persistent storage entry |
| `PatientPayment(u64)` | Persistent storage entry |
| `AccessControlId` | Persistent storage entry |
| `FinancialRecordsId` | Persistent storage entry |
| `ReconciliationThreshold` | configurable threshold in seconds for unreconciled claims |
| `InsurerUnreconciledClaims(Address)` | insurer_id -> Vec<u64> of claim_ids |
| `InsurerRegistryId` | Persistent storage entry |

### `medical-device-tracking`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `Regulator` | Persistent storage entry |
| `DeviceCounter` | Persistent storage entry |
| `ImplantCounter` | Persistent storage entry |
| `DmeCounter` | Persistent storage entry |
| `RecallCounter` | Persistent storage entry |
| `MaintenanceCounter` | Persistent storage entry |
| `WarrantyCounter` | Persistent storage entry |
| `DeviceRecord(u64)` | Persistent storage entry |
| `ImplantRecord(u64)` | Persistent storage entry |
| `DmeRecord(u64)` | Persistent storage entry |
| `RecallInfo(u64)` | Persistent storage entry |
| `MaintenanceRecord(u64)` | Persistent storage entry |
| `WarrantyRecord(u64)` | Persistent storage entry |
| `PatientImplants(Address)` | Persistent storage entry |
| `DeviceImplants(u64)` | Persistent storage entry |
| `DeviceRecalls(u64)` | Persistent storage entry |
| `DeviceWarranties(u64)` | Persistent storage entry |
| `PerformanceReports(u64)` | Persistent storage entry |

### `mental-health`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `AssessmentCounter` | Persistent storage entry |
| `PlanCounter` | Persistent storage entry |
| `HospitalizationCounter` | Persistent storage entry |
| `ScreeningCounter` | Persistent storage entry |
| `Assessment(u64)` | Persistent storage entry |
| `TreatmentPlan(u64)` | Persistent storage entry |
| `Hospitalization(u64)` | Persistent storage entry |
| `SafetyPlan(u64)` | Persistent storage entry |
| `Screening(u64)` | Persistent storage entry |
| `PrivacyFlag(Address, Symbol)` | Persistent storage entry |
| `Session(u64, u64)` | Persistent storage entry |
| `Symptom(Address, Symbol, u64)` | Persistent storage entry |
| `Outcomes(u64, u64)` | Persistent storage entry |
| `Consent(Address, Symbol, Address)` | Persistent storage entry |

### `multisig-governance`

- **Retention class:** Ephemeral
- **Bump / threshold:** 17,280 / 8,640 ledgers
- **HIPAA note:** Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent.

| Storage key | Notes |
|-------------|-------|
| `Initialized` | Persistent storage entry |
| `Signers` | Persistent storage entry |
| `Threshold` | Persistent storage entry |
| `Ttl` | Persistent storage entry |
| `QuorumMin` | Persistent storage entry |
| `Proposal(Symbol)` | Persistent storage entry |
| `SignerProposal` | Persistent storage entry |
| `ProposalIds` | Persistent storage entry |

### `nft-badges`

- **Retention class:** Ephemeral
- **Bump / threshold:** 17,280 / 8,640 ledgers
- **HIPAA note:** Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `NextId` | Persistent storage entry |
| `Badge(u64)` | Persistent storage entry |
| `OwnerBadges(Address)` | Persistent storage entry |

### `nutrition-care-management`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `AssessmentCounter` | Persistent storage entry |
| `CarePlanCounter` | Persistent storage entry |
| `DietOrderCounter` | Persistent storage entry |
| `OutcomeCounter` | Persistent storage entry |
| `Assessment(u64)` | Persistent storage entry |
| `ComputedNeeds(u64)` | Persistent storage entry |
| `CarePlan(u64)` | Persistent storage entry |
| `DietOrder(u64)` | Persistent storage entry |
| `Interventions(u64)` | Persistent storage entry |
| `FoodIntake(Address)` | Persistent storage entry |
| `WeightHistory(Address)` | Persistent storage entry |
| `MalnutritionScreening(u64)` | Persistent storage entry |
| `Supplements(u64)` | Persistent storage entry |
| `OutcomeEvaluation(u64)` | Persistent storage entry |
| `PatientAssessments(Address)` | Persistent storage entry |
| `PatientDietOrders(Address)` | Persistent storage entry |
| `PlanOutcomes(u64)` | Persistent storage entry |
| `ClinicalOutcome(u64)` | Persistent storage entry |
| `PlanVersion(u64)` | Persistent storage entry |
| `AuthorizedProviders(u64)` | Persistent storage entry |

### `pacs-integration`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `StudyCounter` | Persistent storage entry |
| `CdCounter` | Persistent storage entry |
| `Study(u64)` | Persistent storage entry |
| `SeriesList(u64)` | Persistent storage entry |
| `Report(u64)` | Persistent storage entry |
| `AccessList(u64)` | Persistent storage entry |
| `PatientStudies(Address)` | Persistent storage entry |
| `ViewLog(u64)` | Persistent storage entry |
| `ViewerLastViewTs(u64, Address)` | Persistent storage entry |
| `ViewerViewChainHead(u64, Address)` | Persistent storage entry |
| `QcReview(u64)` | Persistent storage entry |
| `AnonymizedStudy(u64, u32)` | Persistent storage entry |
| `CdRecord(u64)` | Persistent storage entry |
| `AnonSalt` | Persistent storage entry |

### `patient-registry`

- **Retention class:** Critical + Operational
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** Clinical identifiers use Critical class; administrative indexes use Operational class per patient-registry policy.

| Storage key | Notes |
|-------------|-------|
| `Admin` | Persistent storage entry |
| `Patient(Address)` | Persistent storage entry |
| `Doctor(Address)` | Persistent storage entry |
| `Institution(Address)` | Persistent storage entry |
| `MedicalRecords(Address)` | Persistent storage entry |
| `AuthorizedDoctors(Address)` | Persistent storage entry |
| `RegulatoryHold(Address)` | Persistent storage entry |
| `ConsentVersion` | Persistent storage entry |
| `ConsentAck(Address)` | Persistent storage entry |
| `Guardian(Address)` | Persistent storage entry |
| `PatientList` | Persistent storage entry |
| `DoctorList` | Persistent storage entry |
| `LastSnapshotLedger` | Persistent storage entry |
| `RecordFee` | Persistent storage entry |
| `Treasury` | Persistent storage entry |
| `FeeToken` | Persistent storage entry |
| `TotalPatients` | Persistent storage entry |
| `TotalRecordsCreated` | Persistent storage entry |
| `TotalProviders` | Persistent storage entry |
| `TotalAccessGrants` | Persistent storage entry |
| `ShareNonce(Address)` | Persistent storage entry |
| `ExportNonce(Address)` | Persistent storage entry |
| `ShareLink(BytesN<32>)` | Persistent storage entry |
| `Deregistered(Address)` | Persistent storage entry |
| `Frozen` | Persistent storage entry |
| `RecordCounter` | Persistent storage entry |
| `PatientRecordIds(Address)` | Persistent storage entry |
| `MedicalRecord(u64)` | Persistent storage entry |
| `FieldAccess(Address, Address, u64)` | Persistent storage entry |
| `GlobalTypeIndex(Symbol)` | Persistent storage entry |
| `DeletedRecord(u64)` | Persistent storage entry |
| `ArchivedRecord(u64)` | Persistent storage entry |
| `MerkleRoot(Address)` | Persistent storage entry |
| `ProviderRegistry` | Persistent storage entry |

### `patient-vitals`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `VitalsHistory(Address)` | map to Vec<VitalReading> |
| `MonitoringParams(Address, Symbol)` | map to MonitoringParameters |
| `DeviceReg(Address, String)` | map to DeviceRegistration |
| `VitalsAlerts(Address, Symbol)` | map to Vec<VitalAlert> |
| `RawWindow(Address, u64)` | Persistent storage entry |
| `AggWindow(Address, u64)` | Persistent storage entry |
| `LatestRawWindow(Address)` | Persistent storage entry |
| `LastAlertTime(Address, Symbol)` | Persistent storage entry |

### `prenatal-pediatric`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `Pregnancy(u64)` | Persistent storage entry |
| `PrenatalVisit(u64)` | Persistent storage entry |
| `PrenatalScreening(u64)` | Persistent storage entry |
| `Ultrasound(u64)` | Persistent storage entry |
| `Labor(u64)` | Persistent storage entry |
| `Delivery(u64)` | Persistent storage entry |
| `Newborn(Address)` | Persistent storage entry |
| `NewbornScreening(u64)` | Persistent storage entry |
| `Growth(u64)` | Persistent storage entry |
| `GrowthByAge(Address, u32)` | Persistent storage entry |
| `Milestone(Address, u32)` | Persistent storage entry |
| `WellChildVisit(Address, u64)` | Persistent storage entry |

### `prescription-management`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `Medication(String)` | Persistent storage entry |
| `MedicationCatalog` | Persistent storage entry |
| `InteractionCounter` | Persistent storage entry |
| `InteractionById(u64)` | Persistent storage entry |
| `InteractionPair(String, String)` | Persistent storage entry |
| `InteractionCatalog` | Persistent storage entry |
| `PatientAllergies(Address)` | Persistent storage entry |
| `PatientConditions(Address)` | Persistent storage entry |
| `MedicationContraindications(String)` | Persistent storage entry |
| `InteractionOverride(u64, Address)` | Persistent storage entry |
| `RegistryAdmin` | Persistent storage entry |
| `RegistryWriter(Address)` | Persistent storage entry |
| `RegistryProposalCounter` | Persistent storage entry |
| `RegistryProposal(u64)` | Persistent storage entry |
| `SnapshotCounter` | Persistent storage entry |
| `CatalogSnapshot(u64)` | Persistent storage entry |
| `ProviderRegistry` | Persistent storage entry |
| `AllergyRegistry` | Persistent storage entry |
| `Admin` | Persistent storage entry |
| `AllergyStrictMode` | Persistent storage entry |
| `RecallCounter` | Persistent storage entry |
| `RecallRecord(u64)` | Persistent storage entry |
| `PrescriptionRecall(u64)` | Persistent storage entry |
| `PatientPrescriptions(Address)` | Persistent storage entry |

### `prior-authorization`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `AuthCounter` | Persistent storage entry |
| `AppealCounter` | Persistent storage entry |
| `ReviewCounter` | Persistent storage entry |
| `AuthRequest(u64)` | Persistent storage entry |
| `Documents(u64)` | Persistent storage entry |
| `PeerToPeer(u64)` | Persistent storage entry |
| `Appeals(u64)` | Persistent storage entry |
| `Appeal(u64)` | Persistent storage entry |
| `ReviewHistory(u64)` | Persistent storage entry |
| `Review(u64)` | Persistent storage entry |
| `Extension(u64)` | Persistent storage entry |
| `UsageRecords(u64)` | Persistent storage entry |
| `ProviderAuths(Address)` | Persistent storage entry |
| `PatientAuths(Address)` | Persistent storage entry |
| `Reviewer(Address)` | Persistent storage entry |
| `InsurerReviewers(Address)` | Persistent storage entry |
| `SLAConfig(Symbol)` | Persistent storage entry |
| `OverdueAuths` | Persistent storage entry |
| `InsurerRegistryId` | Persistent storage entry |

### `provider-registry`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Initialized` | Persistent storage entry |
| `Admin` | Persistent storage entry |
| `Provider(Address)` | Persistent storage entry |
| `Record(String)` | Persistent storage entry |
| `ProviderRecords(Address)` | Persistent storage entry |
| `ProviderRecordCount(Address)` | Persistent storage entry |
| `RateLimitConfig` | Persistent storage entry |
| `ProviderRate(Address)` | Persistent storage entry |
| `ProviderReputation(Address)` | Persistent storage entry |
| `ProviderRatingByPatient(Address, Address)` | Persistent storage entry |
| `PendingAdmin` | Persistent storage entry |
| `RotationExpiry` | Persistent storage entry |

### `referral`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `Referral(u64)` | Persistent storage entry |
| `ReferralCount` | Persistent storage entry |

### `rehabilitation-services`

- **Retention class:** Critical
- **Bump / threshold:** 535,680 / 518,400 ledgers
- **HIPAA note:** PHI/ePHI and clinical artifacts require rolling retention to satisfy HIPAA minimum necessary and 6-year record-keeping expectations without silent ledger expiry during active treatment.

| Storage key | Notes |
|-------------|-------|
| `EvaluationCounter` | Persistent storage entry |
| `Evaluation(u64)` | Persistent storage entry |
| `ROMAssessments(u64)` | Persistent storage entry |
| `StrengthAssessments(u64)` | Persistent storage entry |
| `BalanceMobilityAssessments(u64)` | Persistent storage entry |
| `TreatmentPlanCounter` | Persistent storage entry |
| `TreatmentPlan(u64)` | Persistent storage entry |
| `TherapySessions(u64)` | Persistent storage entry |
| `PainMeasurements(u64)` | Persistent storage entry |
| `FunctionalOutcomes(u64)` | Persistent storage entry |
| `AuthorizationCounter` | Persistent storage entry |
| `Authorization(u64)` | Persistent storage entry |
| `ProgressNotes(u64)` | Persistent storage entry |
| `Discharge(u64)` | Persistent storage entry |
| `GoalCounter` | Persistent storage entry |
| `MeasurableGoal(u64)` | Persistent storage entry |
| `GoalProgressList(u64)` | Persistent storage entry |

### `scholarship-fund`

- **Retention class:** Ephemeral
- **Bump / threshold:** 17,280 / 8,640 ledgers
- **HIPAA note:** Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent.

| Storage key | Notes |
|-------------|-------|
| `Admin,PoolBalance,Deposit(Address)` | Persistent storage entry |

### `telemedicine`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `VirtualVisit(u64)` | Persistent storage entry |
| `VisitCount` | Persistent storage entry |
| `SessionNonce` | Persistent storage entry |
| `Session(u64)` | Persistent storage entry |
| `LicenseRegistry(Address, String)` | Persistent storage entry |
| `JurisdictionPolicy(String)` | Persistent storage entry |

### `upgrade-governance`

- **Retention class:** Ephemeral
- **Bump / threshold:** 17,280 / 8,640 ledgers
- **HIPAA note:** Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent.

| Storage key | Notes |
|-------------|-------|
| `Initialized` | Persistent storage entry |
| `Signers` | Persistent storage entry |
| `Threshold` | Persistent storage entry |
| `NextId` | Persistent storage entry |
| `Proposal(u64)` | Persistent storage entry |
| `ApprovedArtifactMetadata(BytesN<32>)` | Persistent storage entry |
| `SignerProposal` | Persistent storage entry |
| `SchemaVersion` | Persistent storage entry |

### `zk-eligibility`

- **Retention class:** Ephemeral
- **Bump / threshold:** 17,280 / 8,640 ledgers
- **HIPAA note:** Non-clinical counters and caches; no PHI; minimal retention reduces attack surface and storage rent.

| Storage key | Notes |
|-------------|-------|
| `Initialized` | Persistent storage entry |
| `Admin` | Persistent storage entry |
| `VerifierKey(u32)` | Persistent storage entry |
| `Nullifier(BytesN<32>)` | Persistent storage entry |
| `Eligibility(Address)` | Persistent storage entry |
| `PendingAdmin` | Persistent storage entry |
| `RotationExpiry` | Persistent storage entry |

### `zk-eligibility-verifier`

- **Retention class:** Operational
- **Bump / threshold:** 120,960 / 60,480 ledgers
- **HIPAA note:** Workflow, billing, and audit-adjacent records: retain through adjudication cycles; shorter rolling window limits stale operational PHI exposure.

| Storage key | Notes |
|-------------|-------|
| `ZkEligibilityContract` | Persistent storage entry |


## Compliance Checklist

- [ ] Critical PHI contracts use `ttl-config::critical` helpers
- [ ] Operational workflow contracts use `ttl-config::operational` helpers
- [ ] Ephemeral counters use `ttl-config::ephemeral` helpers
- [ ] Every new persistent key is listed in this document
- [ ] Retention class matches the `ttl-config` constants in code

## References

- [TTL_POLICY.md](TTL_POLICY.md)
- [contracts/ttl-config/src/lib.rs](contracts/ttl-config/src/lib.rs)
