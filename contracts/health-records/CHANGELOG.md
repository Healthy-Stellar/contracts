# Changelog

## [Unreleased]

### Migration note

`record_type: String` on `MedicalRecord` / `RecordInput` has been replaced
with `record_category: RecordCategory` (Lab, Imaging, Consultation,
Prescription, Discharge, Vaccination, Other) plus a separate
`record_description: Option<String>` for the old free-text value.
`create_record`, `create_records_batch`, and `update_record` now take
`record_category` and `record_description` instead of a single
`record_type` string. Callers should map their existing type strings to
the closest `RecordCategory` variant and pass the original string as
`record_description` if it's still needed.
