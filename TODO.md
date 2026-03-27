#92 Add record update function with version history (Healthy-Stellar/contracts)

**Status**: In progress

**Target**: contracts/patient-registry/src/lib.rs (existing medical records with IPFS Bytes storage)

**Acceptance Criteria**:
- update_record(record_id: u64, new_ipfs_hash: Bytes) — provider only
- Previous version appended to Vec<RecordVersion> {ipfs_hash: Bytes, updated_by: Address, updated_at: u64}
- get_record_history(record_id: u64) -> Vec<RecordVersion>
- Integrate with existing authorized_doctors auth

## Steps to Complete:

### 1. Update lib.rs types & storage keys ✅
- Add RecordVersion struct
- Add RecordData struct {current_ipfs: Bytes, history: Vec<RecordVersion>, latest_version: u64}
- Add DataKey::RecordCounter, DataKey::PatientRecordIds(Address) -> Vec<u64>
- Add DataKey::MedicalRecord(u64) -> RecordData

**Status**: ✅ Completed

### 2. Refactor add_medical_record ✅
- Generate record_id from global instance counter
- Create initial RecordData {..., history: [initial version], latest_version: 1}
- Append to PatientRecordIds(patient) Vec<u64>
- Store RecordData at MedicalRecord(record_id)
- Return record_id: u64
- Added TTL bump for new keys, event "record_added"

**Status**: ✅ Completed

### 3. Implement update_record
 - Load RecordData by record_id
 - Extract patient from RecordData, validate Self::require_not_on_hold & access_map.contains_key(caller)
 - Validate new_ipfs_hash CID
 - Append RecordVersion {old.current_ipfs, caller, timestamp} to history
 - Set current_ipfs = new_ipfs_hash, latest_version +=1
 - Store, TTL bump, event "record_updated"

**Status**: [ ]

**Status**: [ ]

### 4. Implement get_record_history
- Load RecordData by record_id
- Return history Vec

**Status**: [ ]

### 5. Update existing getters
- get_medical_records(patient): load patient record_ids, collect current_ipfs + metadata from each RecordData
- get_records_by_type: filter by existing record_type field

**Status**: [ ]

### 6. Update test.rs
- Add tests: add_record creates initial version
- update_record auth (success/fail), CID validation, history append
- get_record_history returns correct versions
- Update existing tests for new storage

**Status**: [ ]

### 7. Test & Verify
- cd contracts/patient-registry &amp;&amp; cargo test
- Fix any test failures/linter errors

**Status**: [ ]

### 8. Complete
- Update this TODO with completion note
- attempt_completion

**Status**: [ ]

**Notes**:
- Use global record_id counter for uniqueness
- Keep existing auth (authorized_doctors Map per patient)
- Events for update
- TTL bump on update/read

