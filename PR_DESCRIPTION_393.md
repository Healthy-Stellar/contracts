# Add clinical outcomes tracking to nutrition care plans (#393)

## Summary

This PR implements clinical outcomes tracking for nutrition care plans, enabling evaluation of nutritional intervention effectiveness by linking measurable outcomes (weight, lab values, vitals) to care plans with version-based correlation tracking.

## Changes

### Core Functionality

**`link_outcome()`** - Record clinical measurements tied to care plans:
- Links outcomes to specific plan versions for correlation analysis
- Supports extensible outcome metrics (13 predefined types)
- Tracks measurement timestamp separately from recording timestamp
- Requires provider authorization
- Emits `NutritionOutcomeRecorded` event

**`get_plan_outcomes()`** - Query all outcomes for a care plan:
- Returns outcomes in chronological order
- Includes plan version for each outcome
- Supports filtering and analysis by metric type

**`update_care_plan_version()`** - Increment plan version:
- Tracks when care plans are updated
- Enables correlation between plan changes and outcome improvements
- Only dietitian can update version

**`authorize_provider()`** - Grant provider write access:
- Enables multi-disciplinary care teams
- Only original dietitian can authorize
- Idempotent operation

**`is_provider_authorized()`** - Check provider authorization:
- Dietitian and patient automatically authorized
- Additional providers require explicit authorization

**`get_clinical_outcome()`** - Retrieve specific outcome by ID

**`get_plan_version()`** - Get current care plan version

### Supported Outcome Metrics

**Weight & Body Composition:**
- weight_kg, bmi, waist_circumference

**Lab Values:**
- hba1c, cholesterol_total, cholesterol_ldl, cholesterol_hdl
- triglycerides, glucose_fasting, albumin, prealbumin

**Vitals:**
- blood_pressure_systolic, blood_pressure_diastolic

All values stored as `i64 × 100` for precision (e.g., 70.5 kg → 7050)

### Data Model

**ClinicalOutcome:**
```rust
pub struct ClinicalOutcome {
    pub outcome_id: u64,
    pub care_plan_id: u64,
    pub plan_version: u64,
    pub provider_id: Address,
    pub outcome_metric: String,
    pub outcome_value_x100: i64,
    pub measured_at: u64,
    pub recorded_at: u64,
}
```

**NutritionOutcomeRecordedEvent:**
```rust
pub struct NutritionOutcomeRecordedEvent {
    pub outcome_id: u64,
    pub care_plan_id: u64,
    pub plan_version: u64,
    pub outcome_metric: String,
    pub outcome_value_x100: i64,
    pub measured_at: u64,
}
```

### Storage Updates

Added storage keys:
- `OutcomeCounter` - Auto-increment for outcome IDs
- `PlanOutcomes(u64)` - List of outcome IDs per care plan
- `ClinicalOutcome(u64)` - Individual outcome records
- `PlanVersion(u64)` - Current version per care plan
- `AuthorizedProviders(u64)` - Authorized providers per care plan

### Integration

**Care Plan Creation:**
- Automatically initializes plan version to 1
- Automatically authorizes dietitian and patient

**Events Emitted:**
- `nutrition_outcome_recorded` - When outcome is linked
- `plan_version_updated` - When plan version changes
- `provider_authorized` - When provider access granted

## Acceptance Criteria ✓

- [x] Outcomes are linked to specific plan versions (supports plan updates)
- [x] Query returns chronologically ordered outcomes
- [x] Outcome metrics are extensible (weight, HbA1c, cholesterol, etc.)
- [x] Only providers with write access to the patient can record outcomes

## Testing

Added comprehensive test coverage:
- **24 new tests** for outcome tracking functionality
- **64 total tests** (all passing)

**Test Categories:**
- Outcome linking (6 tests)
- Outcome queries (3 tests)
- Plan versioning (4 tests)
- Provider authorization (6 tests)
- Workflow tests (5 tests)

**Key Tests:**
- `test_link_outcome_success` - Basic outcome recording
- `test_link_outcome_unauthorized_provider` - Authorization enforcement
- `test_get_plan_outcomes_chronological_order` - Ordering verification
- `test_get_plan_outcomes_linked_to_plan_version` - Version tracking
- `test_full_outcome_tracking_workflow` - End-to-end scenario
- `test_outcome_tracking_all_valid_metrics` - All 13 metric types

## Documentation

Added `OUTCOMES_TRACKING.md` with:
- Feature overview and capabilities
- Supported outcome metrics
- Data model details
- Usage examples for common scenarios
- Error handling
- Integration points
- Use cases (diabetes, cardiovascular, malnutrition)
- Future enhancement suggestions
- Performance and security considerations

## Use Case Examples

### Diabetes Management
```rust
// Baseline
link_outcome(plan_id, provider, "hba1c", 850, date1);
link_outcome(plan_id, provider, "weight_kg", 9500, date1);

// 3-month follow-up
link_outcome(plan_id, provider, "hba1c", 720, date2);
link_outcome(plan_id, provider, "weight_kg", 9000, date2);
```

### Multi-Provider Care Team
```rust
// Authorize endocrinologist
authorize_provider(plan_id, dietitian, endocrinologist);

// Endocrinologist records lab values
link_outcome(plan_id, endocrinologist, "glucose_fasting", 9500, date);
```

### Patient Self-Reporting
```rust
// Patient records own weight
link_outcome(plan_id, patient, "weight_kg", 8250, date);
```

## Breaking Changes

None. This is a new feature that extends existing functionality without modifying existing APIs.

## Files Changed

- `contracts/nutrition-care-management/src/lib.rs` - Core implementation
- `contracts/nutrition-care-management/src/types.rs` - New data structures
- `contracts/nutrition-care-management/src/storage.rs` - Storage helpers
- `contracts/nutrition-care-management/src/test.rs` - Comprehensive tests
- `contracts/nutrition-care-management/OUTCOMES_TRACKING.md` - Documentation

## Performance Considerations

- Outcomes stored individually for granular tracking
- Chronological ordering maintained by insertion order
- Plan version lookups are O(1)
- Provider authorization checks are O(n) where n = authorized providers
- Consider pagination for plans with hundreds of outcomes

## Security & Privacy

- Provider authorization prevents unauthorized recording
- Outcomes inherit care plan access controls
- Events enable audit trail for compliance
- Separate measurement and recording timestamps for data integrity

## Future Enhancements

- Outcome target ranges and alerts
- Automated trend analysis
- Integration with wearable devices
- Statistical correlation analysis
- Outcome-based care plan recommendations
- Export for research and quality improvement

## Related Issues

Closes #393
