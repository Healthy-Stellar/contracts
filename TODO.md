#120 Get Records by Provider TODO

**Issue**: Implement `get_records_by_provider(provider: Address, page: u32, page_size: u32) -> Vec<MedicalRecord>` for whitelisted providers (doctor/admin).

**Status**: Not implemented (no index, no fn, no soft-delete).

**Plan**:
**lib.rs**:
- `#[contracttype] struct ProviderIndex { patient: Address, timestamp: u64 }`
- DataKey::ProviderRecords(Address) -> Vec<ProviderIndex>
- MedicalRecord add `deleted: bool`
- `add_medical_record`: append ProviderRecords(doctor).push({patient, timestamp})
- `get_records_by_provider(provider, page, page_size)`: 
  - auth: caller == provider || admin
  - cap page_size <=20
  - index = ProviderRecords(provider), sort desc timestamp, slice page*size..(page+1)*size
  - for each index, fetch MedicalRecords(patient)[timestamp match], !deleted, collect Vec
  - empty if none
- Update `get_medical_records`, `get_records_by_type`: skip deleted

**test.rs**: tests auth (provider/admin ok, stranger fail), pagination (0-20 page0, page1, empty), deleted exclude.

**Followup**: cargo test, CI.

**Steps**:
- [ ] 1. Edit lib.rs add structs/keys/indexing/fn
- [ ] 2. Edit test.rs add tests
- [ ] 3. cargo test verify
- [ ] 4. Update TODO, complete
