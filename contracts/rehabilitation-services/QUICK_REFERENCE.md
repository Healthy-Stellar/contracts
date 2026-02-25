# Rehabilitation Services - Quick Reference

## Contract Functions

### 1. Conduct PT Evaluation
```rust
conduct_pt_evaluation(
    patient_id: Address,
    therapist_id: Address,
    evaluation_date: u64,
    diagnosis: String,
    chief_complaint: String,
    functional_limitations: Vec<String>,
    prior_level_of_function: String,
    evaluation_findings_hash: BytesN<32>
) -> Result<u64, Error>
```

### 2. Assess Range of Motion
```rust
assess_range_of_motion(
    evaluation_id: u64,
    joint: String,
    movement: String,
    degrees: u32,
    pain_level: Option<u32>,
    limitation: Option<String>
) -> Result<(), Error>
```

### 3. Assess Strength
```rust
assess_strength(
    evaluation_id: u64,
    muscle_group: String,
    manual_muscle_test_grade: String,
    side: Symbol  // left, right, bilateral
) -> Result<(), Error>
```

### 4. Assess Balance & Mobility
```rust
assess_balance_mobility(
    evaluation_id: u64,
    test_type: Symbol,  // berg, tug, gait_speed, six_minute_walk
    score: u32,
    fall_risk: Symbol
) -> Result<(), Error>
```

### 5. Create Treatment Plan
```rust
create_rehab_treatment_plan(
    evaluation_id: u64,
    therapist_id: Address,
    stg_goals: Vec<RehabGoal>,
    ltg_goals: Vec<RehabGoal>,
    interventions: Vec<TherapyIntervention>,
    frequency: String,
    duration_weeks: u32,
    prognosis: Symbol
) -> Result<u64, Error>
```

### 6. Document Therapy Session
```rust
document_therapy_session(
    treatment_plan_id: u64,
    session_date: u64,
    interventions_performed: Vec<TherapyIntervention>,
    session_duration_minutes: u32,
    patient_response: String,
    homework_assigned: Option<String>
) -> Result<(), Error>
```

### 7. Track Pain Level
```rust
track_pain_level(
    treatment_plan_id: u64,
    measurement_date: u64,
    pain_scale_type: Symbol,  // vas, numeric, faces
    pain_score: u32,
    location: String,
    quality: Vec<String>
) -> Result<(), Error>
```

### 8. Measure Functional Outcome
```rust
measure_functional_outcome(
    treatment_plan_id: u64,
    measurement_date: u64,
    outcome_tool: Symbol,  // oswestry, dash, lefs, womac
    score: u32,
    minimal_detectable_change: bool
) -> Result<(), Error>
```

### 9. Request Authorization
```rust
request_therapy_authorization(
    treatment_plan_id: u64,
    requested_visits: u32,
    justification_hash: BytesN<32>
) -> Result<u64, Error>
```

### 10. Document Progress Note
```rust
document_progress_note(
    treatment_plan_id: u64,
    note_date: u64,
    subjective: String,
    objective_findings: Vec<String>,
    assessment: String,
    plan_modifications: Vec<String>
) -> Result<(), Error>
```

### 11. Discharge from Therapy
```rust
discharge_from_therapy(
    treatment_plan_id: u64,
    discharge_date: u64,
    discharge_reason: Symbol,
    goals_met: Vec<u64>,
    final_outcomes_hash: BytesN<32>,
    home_exercise_program_hash: BytesN<32>
) -> Result<(), Error>
```

## Common Symbols

### Test Types
- `berg` - Berg Balance Scale
- `tug` - Timed Up and Go
- `gait_speed` - Gait Speed Test
- `six_minute_walk` - 6-Minute Walk Test

### Pain Scales
- `vas` - Visual Analog Scale
- `numeric` - Numeric Rating Scale
- `faces` - Faces Pain Scale

### Outcome Tools
- `oswestry` - Oswestry Disability Index
- `dash` - Disabilities of Arm, Shoulder, Hand
- `lefs` - Lower Extremity Functional Scale
- `womac` - Western Ontario McMaster Universities

### Goal Types
- `stg` - Short-term goal
- `ltg` - Long-term goal

### Side
- `left` - Left side
- `right` - Right side
- `bilateral` - Both sides

### Fall Risk
- `low` - Low risk
- `moderate` - Moderate risk
- `high` - High risk

## Error Codes
- `1` - NotFound
- `2` - Unauthorized
- `3` - InvalidInput
- `4` - AlreadyExists
