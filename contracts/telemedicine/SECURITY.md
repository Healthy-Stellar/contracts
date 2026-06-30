# Telemedicine Contract — Security Notes

## Cross-State Prescription Enforcement (#563)

### Threat

A provider licensed in State A conducting a virtual visit with a patient located in State B could
issue a prescription that violates State B's telemedicine prescribing laws (e.g., the Ryan Haight
Act for controlled substances).

### Enforcement logic in `prescribe_during_visit`

1. **Session-active check** — `prescribe_during_visit` rejects any prescription if
   `visit.status != VisitStatus::InProgress`. Prescribing after `end_virtual_session` returns
   `Error::SessionNotActive`.

2. **License check** — The provider must hold an active, non-expired license stored under
   `DataKey::LicenseRegistry(provider_id, patient_location)`. If no such license exists,
   `Error::ProviderNotLicensedInPatientState` is returned. This check uses `patient_location`,
   which is set at session-start time in `start_virtual_session`.

3. **Controlled-substance policy** — When `PrescriptionRequest.is_controlled_substance` is
   `true`, the jurisdiction policy stored under
   `DataKey::ControlledSubstancePolicy(patient_location)` is read. If `requires_inperson` is
   set to `true` for that jurisdiction, `Error::ControlledSubstanceRequiresInPerson` is
   returned, blocking the telehealth prescription for schedule I–V substances.

### Configuring policies

Controlled-substance policies are set per jurisdiction by an authorized admin:

```rust
contract.set_controlled_substance_policy(admin, jurisdiction, requires_inperson);
```

Provider licenses are self-registered by the provider:

```rust
contract.register_provider_license(provider_id, jurisdiction, license_number, valid_until);
```

### Limitations

- License validity is time-based (`valid_until`). Clock manipulation on the ledger is not
  possible, but long-lived (non-expiring, `valid_until = 0`) licenses are allowed.
- Compact interstate licensing is enforced only at session-start (`start_virtual_session` via
  `verify_telemedicine_eligibility`). For prescriptions, only a **direct** state license is
  accepted; compact membership does not satisfy the prescription license check.
- The `is_controlled_substance` flag is caller-supplied and not verified against a formulary.
  A future integration with a drug formulary contract can provide this verification.
