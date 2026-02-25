# Rehabilitation Services Smart Contract

Soroban smart contract for managing physical therapy and rehabilitation services on the Stellar blockchain.

## Features

- **PT Evaluations**: Conduct initial physical therapy evaluations with diagnosis, chief complaints, and functional limitations
- **ROM Assessment**: Track range of motion measurements for joints with pain levels and limitations
- **Strength Assessment**: Document manual muscle testing with grades and laterality
- **Balance & Mobility**: Record standardized tests (Berg, TUG, gait speed, six-minute walk) with fall risk assessment
- **Treatment Plans**: Create comprehensive rehab plans with short-term and long-term goals, interventions, frequency, and prognosis
- **Therapy Sessions**: Document each session with interventions performed, duration, patient response, and homework
- **Pain Tracking**: Monitor pain levels using various scales (VAS, numeric, faces) with location and quality descriptors
- **Functional Outcomes**: Measure progress using standardized tools (Oswestry, DASH, LEFS, WOMAC)
- **Authorization Management**: Request therapy visit authorizations with justification
- **Progress Notes**: Document SOAP-style progress notes with subjective, objective, assessment, and plan
- **Discharge Planning**: Complete discharge documentation with goals met, final outcomes, and home exercise programs

## Data Structures

### RehabGoal
- Goal ID, type (STG/LTG), description
- Target date, measurement method
- Achievement status

### TherapyIntervention
- Intervention type, description
- Sets, reps, duration, resistance

### PTEvaluation
- Patient and therapist IDs
- Diagnosis, chief complaint
- Functional limitations, prior function level
- Evaluation findings hash

### Treatment Plan
- Evaluation reference
- STG and LTG goals
- Interventions list
- Frequency, duration, prognosis

## Functions

### Core Functions

- `conduct_pt_evaluation()` - Initial evaluation documentation
- `assess_range_of_motion()` - ROM measurements
- `assess_strength()` - Strength testing
- `assess_balance_mobility()` - Balance and mobility assessments
- `create_rehab_treatment_plan()` - Treatment plan creation
- `document_therapy_session()` - Session documentation
- `track_pain_level()` - Pain monitoring
- `measure_functional_outcome()` - Outcome measures
- `request_therapy_authorization()` - Authorization requests
- `document_progress_note()` - Progress documentation
- `discharge_from_therapy()` - Discharge planning

### Query Functions

- `get_evaluation()` - Retrieve evaluation by ID
- `get_treatment_plan()` - Retrieve treatment plan
- `get_rom_assessments()` - Get ROM data
- `get_strength_assessments()` - Get strength data
- `get_balance_mobility_assessments()` - Get balance/mobility data
- `get_therapy_sessions()` - Get session history
- `get_pain_measurements()` - Get pain tracking data
- `get_functional_outcomes()` - Get outcome measures
- `get_progress_notes()` - Get progress notes
- `get_discharge_record()` - Get discharge documentation

## Testing

All functions have comprehensive test coverage (>85%):

```bash
cargo test
```

Tests include:
- Individual function tests for each operation
- Complete workflow test covering the full rehab cycle
- Authorization and authentication checks
- Data retrieval and validation

## Usage Example

```rust
// 1. Conduct initial evaluation
let eval_id = client.conduct_pt_evaluation(
    &patient_id,
    &therapist_id,
    &timestamp,
    &diagnosis,
    &chief_complaint,
    &limitations,
    &prior_function,
    &findings_hash,
);

// 2. Assess ROM
client.assess_range_of_motion(
    &eval_id,
    &joint,
    &movement,
    &degrees,
    &pain_level,
    &limitation,
);

// 3. Create treatment plan
let plan_id = client.create_rehab_treatment_plan(
    &eval_id,
    &therapist_id,
    &stg_goals,
    &ltg_goals,
    &interventions,
    &frequency,
    &duration_weeks,
    &prognosis,
);

// 4. Document sessions
client.document_therapy_session(
    &plan_id,
    &session_date,
    &interventions,
    &duration,
    &response,
    &homework,
);

// 5. Track outcomes
client.measure_functional_outcome(
    &plan_id,
    &date,
    &outcome_tool,
    &score,
    &mdc,
);

// 6. Discharge
client.discharge_from_therapy(
    &plan_id,
    &discharge_date,
    &reason,
    &goals_met,
    &outcomes_hash,
    &hep_hash,
);
```

## Security

- Therapist authentication required for all write operations
- Evaluation and treatment plan validation
- Immutable audit trail of all assessments and interventions
- Encrypted storage of sensitive clinical data via hash references

## Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Deploy

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/rehabilitation_services.wasm \
  --source therapist \
  --network testnet
```
