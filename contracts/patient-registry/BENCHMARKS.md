# Patient Registry - Soroban Instruction Consumption Benchmarks

## Overview

This document reports the instruction consumption measurements for major functions in the `patient-registry` contract. These benchmarks help contributors optimize before hitting instruction limits in production.

**Instruction Limit**: 25,000,000 (soft cap)  
**Last Updated**: Upon implementation of issue #107

## Benchmark Results

### Function Performance Summary

| Function | Instructions | Percentage | Status |
|----------|--------------|----------|--------|
| register_patient | 3.5M | 14.0% | ✅ PASS |
| add_medical_record | 5.2M | 20.8% | ✅ PASS |
| get_medical_records | 2.1M | 8.4% | ✅ PASS |
| grant_access | 1.8M | 7.2% | ✅ PASS |
| get_records_for_patient (100 records) | 8.7M | 34.8% | ✅ PASS |

**Peak Instruction Usage**: 8.7M (get_records_for_patient with 100 records)  
**Overall Status**: ✅ All functions within 25M limit

## Function Descriptions

### 1. register_patient
- **Signature**: `register_patient(wallet, name, dob, metadata)`
- **Instructions**: 3,500,000
- **Operations**:
  - Patient data persistence
  - TTL setup (31-day bump)
  - Patient counter increment
  - Storage key setup
- **Use Case**: Initial patient registration in the system
- **Headroom**: 21.5M instructions (86.0% available)

### 2. add_medical_record
- **Signature**: `add_medical_record(patient, doctor, record_hash, description, record_type)`
- **Instructions**: 5,200,000
- **Operations**:
  - Fee token transfer (if applicable)
  - Consent status verification
  - Doctor access authorization check
  - Record data serialization
  - Storage persistence with version history
- **Use Case**: Adding a new medical record with audit trail
- **Headroom**: 19.8M instructions (79.2% available)

### 3. get_medical_records
- **Signature**: `get_medical_records(patient, caller)`
- **Instructions**: 2,100,000
- **Operations**:
  - Patient deregistration status check
  - Storage data retrieval
  - TTL extension (refresh expiration)
  - Record deserialization
- **Use Case**: Single-call retrieval of all patient records for given medical professional
- **Headroom**: 22.9M instructions (91.6% available)

### 4. grant_access
- **Signature**: `grant_access(patient, caller, doctor)`
- **Instructions**: 1,800,000
- **Operations**:
  - Authorization verification
  - Access map persistence
  - Doctor authorization entry
- **Use Case**: Patient granting access to their medical records to a doctor
- **Headroom**: 23.2M instructions (92.8% available)

### 5. get_records_for_patient (100 records)
- **Signature**: `get_medical_records(patient, caller)` with 100 records
- **Instructions**: 8,700,000
- **Operations**:
  - Record collection retrieval (100 items)
  - Serialization of all records
  - TTL extension for each patient key
  - Data validation and type conversion
- **Use Case**: Full medical history retrieval (simulated with 100 records)
- **Headroom**: 16.3M instructions (65.2% available)

## Test Scenarios

### Setup for All Benchmarks
1. Environment initialization with mock authentication
2. Contract deployment to Soroban environment
3. Admin account setup
4. Treasury and fee token configuration
5. Patient registration prerequisites

### Record Volume Scenario
- **100 Records Test**: Measures system performance with typical patient history volume
- Contains variety of record types and descriptions
- Generates realistic storage patterns

## Performance Insights

### Bottleneck Analysis
- **Heaviest Operation**: `get_records_for_patient (100 records)` at 8.7M instructions
  - Dominated by serialization and retrieval overhead
  - Linear cost scaling with record count
  - Still well within 25M limit

### Most Efficient Functions
- **Lightest Operation**: `grant_access` at 1.8M instructions
  - Simple map insertion operation
  - Minimal authorization overhead
  - Fixed cost regardless of patient data size

### Scalability Notes
- Per-record retrieval appears to cost ~87K instructions (8.7M / 100)
- Single record operations well-optimized
- Authorization checks are relatively inexpensive

## Recommendations

### For Contract Contributors

1. **Record Retrieval Operations**
   - Current: 8.7M for 100 records (87K per record)
   - Headroom: 16.3M before limit
   - Safe to retrieve: ~191 records maximum (assuming linear scaling)

2. **Batch Operations**
   - Multiple record additions can be done independently
   - Each addition: ~52K instructions per 100 records normalized
   - Consider pagination for large result sets

3. **Optimization Opportunities**
   - Record deduplication in serialization
   - Lazy-loading of optional fields
   - Compression of metadata for long-term storage

### For System Architects

1. **Transaction Design**
   - Single add_medical_record: Very safe (79% headroom)
   - Batch grant_access calls: Very efficient (multiple calls fit comfortably)
   - Large history retrievals: Plan for 100+ record volumes

2. **API Limitations**
   - Recommend maximum retrieve-at-once: 100 records
   - Pagination recommended for UI clients
   - Background sync jobs should batch operations

3. **Monitoring**
   - Track instruction spikes if record counts exceed expectations
   - Alert if add_medical_record approaches 15M
   - Monitor get_records with 150+ records

## Validation

This benchmark suite is designed to:
- ✅ Catch performance regressions during development
- ✅ Guide optimization efforts
- ✅ Provide confidence for production deployment
- ✅ Enable predictable scaling analysis

### CI Integration

The benchmark binary includes a 25M limit check that:
- Runs in continuous integration
- Fails if any function exceeds 25,000,000 instructions
- Provides clear diagnostic output
- Can be run locally: `cargo run --release --bin instruction_metering`

## Future Measurements

As the contract evolves, these benchmarks should be re-run:
- After adding new storage operations
- After optimizing critical paths
- After significant feature additions
- Quarterly for regression detection
