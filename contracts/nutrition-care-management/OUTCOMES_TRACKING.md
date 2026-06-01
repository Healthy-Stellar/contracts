# Clinical Outcomes Tracking Feature (#393)

## Overview

The nutrition-care-management contract now links nutrition care plans to measurable clinical outcomes, enabling evaluation of nutritional intervention effectiveness. This feature tracks metrics like weight, lab values (HbA1c, cholesterol), and vitals over time, with outcomes tied to specific care plan versions for correlation analysis.

## Key Features

### 1. Outcome Linking

The `link_outcome` function records clinical measurements tied to nutrition care plans:

```rust
pub fn link_outcome(
    env: Env,
    care_plan_id: u64,
    provider_id: Address,
    outcome_metric: String,
    outcome_value_x100: i64,
    measured_at: u64,
) -> Result<u64, Error>
```

**Features:**
- Links outcomes to specific care plan versions
- Supports extensible outcome metrics
- Tracks measurement timestamp separately from recording timestamp
- Requires provider authorization
- Emits `NutritionOutcomeRecorded` event

**Authorization:**
- Only providers with write access can record outcomes
- Dietitian who created the plan is automatically authorized
- Patient is automatically authorized
- Additional providers can be authorized via `authorize_provider`

### 2. Supported Outcome Metrics

The system supports an extensible set of clinical metrics:

**Weight & Body Composition:**
- `weight_kg` - Body weight in kilograms
- `bmi` - Body Mass Index
- `waist_circumference` - Waist measurement

**Lab Values:**
- `hba1c` - Glycated hemoglobin (diabetes control)
- `cholesterol_total` - Total cholesterol
- `cholesterol_ldl` - LDL cholesterol
- `cholesterol_hdl` - HDL cholesterol
- `triglycerides` - Triglyceride levels
- `glucose_fasting` - Fasting blood glucose
- `albumin` - Serum albumin (nutritional status)
- `prealbumin` - Prealbumin (nutritional status)

**Vitals:**
- `blood_pressure_systolic` - Systolic blood pressure
- `blood_pressure_diastolic` - Diastolic blood pressure

All values are stored as `i64 × 100` for precision (e.g., 70.5 kg → 7050).

### 3. Plan Version Tracking

Care plans maintain version numbers that increment when plans are updated:

```rust
pub fn update_care_plan_version(
    env: Env,
    care_plan_id: u64,
    dietitian_id: Address,
) -> Result<u64, Error>
```

**Features:**
- Outcomes are automatically linked to the current plan version
- Enables correlation between plan changes and outcome improvements
- Supports longitudinal analysis of intervention effectiveness

### 4. Provider Authorization

Control who can record outcomes for a care plan:

```rust
pub fn authorize_provider(
    env: Env,
    care_plan_id: u64,
    dietitian_id: Address,
    provider_id: Address,
) -> Result<(), Error>
```

**Features:**
- Only the original dietitian can authorize providers
- Dietitian and patient are automatically authorized
- Idempotent - can be called multiple times safely
- Enables multi-disciplinary care teams

### 5. Outcome Queries

Retrieve outcomes in chronological order:

```rust
pub fn get_plan_outcomes(
    env: Env,
    care_plan_id: u64,
) -> Vec<ClinicalOutcome>
```

**Features:**
- Returns all outcomes for a care plan
- Chronologically ordered by `recorded_at`
- Includes plan version for each outcome
- Supports filtering and analysis by metric type

## Data Model

### ClinicalOutcome

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

### NutritionOutcomeRecordedEvent

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

## Usage Examples

### Recording Baseline Outcomes

```rust
// Create care plan
let care_plan_id = client.create_nutrition_care_plan(
    &assessment_id,
    &dietitian,
    &diagnoses,
    &goals,
    &interventions,
    &String::from_str(&env, "weekly"),
);

// Record baseline weight
client.link_outcome(
    &care_plan_id,
    &dietitian,
    &String::from_str(&env, "weight_kg"),
    &8500, // 85.00 kg
    &baseline_date,
);

// Record baseline HbA1c
client.link_outcome(
    &care_plan_id,
    &dietitian,
    &String::from_str(&env, "hba1c"),
    &850, // 8.5%
    &baseline_date,
);
```

### Tracking Progress Over Time

```rust
// Week 4 follow-up
client.link_outcome(
    &care_plan_id,
    &dietitian,
    &String::from_str(&env, "weight_kg"),
    &8300, // 83.00 kg - 2kg loss
    &week4_date,
);

// Week 8 follow-up
client.link_outcome(
    &care_plan_id,
    &dietitian,
    &String::from_str(&env, "weight_kg"),
    &8100, // 81.00 kg - 4kg total loss
    &week8_date,
);
```

### Updating Care Plan and Tracking New Outcomes

```rust
// Update care plan based on progress
let new_version = client.update_care_plan_version(
    &care_plan_id,
    &dietitian,
);

// Record outcomes under new plan version
client.link_outcome(
    &care_plan_id,
    &dietitian,
    &String::from_str(&env, "hba1c"),
    &720, // 7.2% - improved
    &followup_date,
);
```

### Authorizing Additional Providers

```rust
// Authorize endocrinologist to record lab values
client.authorize_provider(
    &care_plan_id,
    &dietitian,
    &endocrinologist,
);

// Endocrinologist can now record outcomes
client.link_outcome(
    &care_plan_id,
    &endocrinologist,
    &String::from_str(&env, "glucose_fasting"),
    &9500, // 95 mg/dL
    &lab_date,
);
```

### Querying and Analyzing Outcomes

```rust
// Get all outcomes for analysis
let outcomes = client.get_plan_outcomes(&care_plan_id);

// Filter by metric type
for outcome in outcomes.iter() {
    if outcome.outcome_metric == String::from_str(&env, "weight_kg") {
        // Analyze weight trend
    }
}

// Compare outcomes by plan version
for outcome in outcomes.iter() {
    if outcome.plan_version == 1 {
        // Baseline outcomes
    } else if outcome.plan_version == 2 {
        // Post-intervention outcomes
    }
}
```

### Patient Self-Reporting

```rust
// Patient records their own weight
client.link_outcome(
    &care_plan_id,
    &patient, // Patient is automatically authorized
    &String::from_str(&env, "weight_kg"),
    &8250,
    &measurement_date,
);
```

## Error Handling

New error codes:
- `OutcomeNotFound`: Outcome ID does not exist
- `InvalidOutcomeMetric`: Metric name not in supported list
- `ProviderNotAuthorized`: Provider lacks write access to record outcomes

## Acceptance Criteria ✓

- [x] Outcomes are linked to specific plan versions (supports plan updates)
- [x] Query returns chronologically ordered outcomes
- [x] Outcome metrics are extensible (weight, HbA1c, cholesterol, etc.)
- [x] Only providers with write access to the patient can record outcomes

## Testing

The implementation includes comprehensive tests (64 total tests, 24 new for outcomes):

**Outcome Linking:**
- `test_link_outcome_success` - Basic outcome recording
- `test_link_outcome_multiple_metrics` - Multiple metric types
- `test_link_outcome_invalid_metric` - Validation
- `test_link_outcome_unauthorized_provider` - Authorization checks
- `test_link_outcome_patient_authorized` - Patient self-reporting

**Outcome Queries:**
- `test_get_plan_outcomes_chronological_order` - Ordering verification
- `test_get_plan_outcomes_linked_to_plan_version` - Version tracking
- `test_get_clinical_outcome_success` - Individual outcome retrieval

**Plan Versioning:**
- `test_update_care_plan_version_success` - Version incrementing
- `test_update_care_plan_version_multiple_times` - Multiple updates
- `test_update_care_plan_version_unauthorized` - Authorization

**Provider Authorization:**
- `test_authorize_provider_success` - Provider authorization
- `test_authorize_provider_can_link_outcome` - Authorized recording
- `test_is_provider_authorized_dietitian_always_authorized` - Default auth
- `test_is_provider_authorized_patient_always_authorized` - Patient auth

**Workflow Tests:**
- `test_full_outcome_tracking_workflow` - End-to-end scenario
- `test_outcome_tracking_all_valid_metrics` - All metric types

## Integration Points

### With Existing Functions

The outcome tracking integrates seamlessly with existing functionality:

1. **Care Plan Creation** - Automatically initializes version 1 and authorizes dietitian/patient
2. **Weight Monitoring** - Complements `monitor_weight_trend` with structured outcome tracking
3. **Outcome Evaluation** - Works alongside `evaluate_nutrition_outcomes` for comprehensive assessment

### Event Emission

Events are emitted for audit trails and external system integration:
- `nutrition_outcome_recorded` - When an outcome is linked
- `plan_version_updated` - When care plan version changes
- `provider_authorized` - When provider access is granted

## Use Cases

### Diabetes Management

Track HbA1c, fasting glucose, and weight to evaluate dietary intervention effectiveness:

```rust
// Baseline
link_outcome(plan_id, provider, "hba1c", 850, date1);
link_outcome(plan_id, provider, "glucose_fasting", 14000, date1);
link_outcome(plan_id, provider, "weight_kg", 9500, date1);

// 3-month follow-up
link_outcome(plan_id, provider, "hba1c", 720, date2);
link_outcome(plan_id, provider, "glucose_fasting", 11000, date2);
link_outcome(plan_id, provider, "weight_kg", 9000, date2);
```

### Cardiovascular Health

Monitor cholesterol and blood pressure response to dietary changes:

```rust
link_outcome(plan_id, provider, "cholesterol_total", 24000, date);
link_outcome(plan_id, provider, "cholesterol_ldl", 16000, date);
link_outcome(plan_id, provider, "blood_pressure_systolic", 14500, date);
```

### Malnutrition Recovery

Track albumin and prealbumin to assess nutritional repletion:

```rust
link_outcome(plan_id, provider, "albumin", 280, date);
link_outcome(plan_id, provider, "prealbumin", 150, date);
link_outcome(plan_id, provider, "weight_kg", 5500, date);
```

## Future Enhancements

Potential improvements:
- Outcome target ranges and alerts
- Automated trend analysis and reporting
- Integration with wearable devices for continuous monitoring
- Outcome-based care plan recommendations
- Statistical correlation analysis between interventions and outcomes
- Export functionality for research and quality improvement
- Graphical outcome visualization support
- Outcome-based reimbursement tracking

## Migration Notes

This is a new feature with no breaking changes to existing functionality. Existing care plans will:
- Default to version 1
- Have dietitian and patient automatically authorized
- Support outcome linking immediately

## Performance Considerations

- Outcomes are stored individually for granular tracking
- Chronological ordering is maintained by insertion order
- Plan version lookups are O(1)
- Provider authorization checks are O(n) where n = number of authorized providers
- Consider pagination for care plans with hundreds of outcomes

## Security & Privacy

- Provider authorization prevents unauthorized outcome recording
- Outcomes inherit care plan access controls
- Events enable audit trail for compliance
- Measurement timestamps separate from recording timestamps for data integrity
