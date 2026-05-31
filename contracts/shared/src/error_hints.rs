/// Actionable error hint strings for the five most common contract error
/// categories.
///
/// These are static, non-PII strings that integrators can surface to help
/// diagnose failures quickly.  They describe *what permission or constraint
/// was violated* and *how to fix it*, without including any patient or
/// provider identifiers (HIPAA compliance).
///
/// # Usage
///
/// Return the hint alongside your typed error code so callers can log or
/// display it:
///
/// ```ignore
/// return Err((Error::Unauthorized, hints::UNAUTHORIZED_REQUIRES_PATIENT_AUTH));
/// ```
///
/// Or emit it as a diagnostic event:
///
/// ```ignore
/// env.events().publish(
///     (Symbol::new(&env, "error_hint"),),
///     hints::NOT_FOUND_RECORD,
/// );
/// return Err(Error::RecordNotFound);
/// ```
///
/// Closes #400.
pub mod hints {
    // ── Unauthorized ─────────────────────────────────────────────────────────

    /// The caller must be the patient who owns the resource.
    pub const UNAUTHORIZED_REQUIRES_PATIENT_AUTH: &str =
        "Unauthorized: caller must be the patient who owns this resource. \
         Ensure the transaction is signed by the patient's key.";

    /// The caller must be a provider with active patient consent.
    pub const UNAUTHORIZED_REQUIRES_PROVIDER_CONSENT: &str =
        "Unauthorized: caller must be a healthcare provider with active \
         patient consent. Call grant_consent first.";

    /// The caller must be the contract admin.
    pub const UNAUTHORIZED_REQUIRES_ADMIN: &str =
        "Unauthorized: this operation requires admin privileges. \
         Ensure the transaction is signed by the admin key.";

    // ── InvalidInput ─────────────────────────────────────────────────────────

    /// The encrypted envelope URI is malformed.
    pub const INVALID_INPUT_ENVELOPE_URI: &str =
        "InvalidInput: envelope_uri must start with 'enc+ipfs://' or \
         'enc+https://' and be 16–256 ASCII characters with no whitespace.";

    /// The content hash field is all-zero (unset).
    pub const INVALID_INPUT_ZERO_HASH: &str =
        "InvalidInput: content_hash must not be all-zero bytes. \
         Provide the SHA-256 hash of the encrypted content.";

    /// A timestamp is in the future when a past/present value is required.
    pub const INVALID_INPUT_FUTURE_TIMESTAMP: &str =
        "InvalidInput: the supplied timestamp is in the future. \
         Use the current ledger timestamp or a past value for this field.";

    /// A timestamp is not in the future when a future value is required.
    pub const INVALID_INPUT_PAST_TIMESTAMP: &str =
        "InvalidInput: the supplied timestamp must be strictly in the future. \
         Use a scheduled time that has not yet passed.";

    /// A validity window is zero-length or exceeds the allowed maximum.
    pub const INVALID_INPUT_VALIDITY_WINDOW: &str =
        "InvalidInput: validity window is invalid. \
         end must be strictly after start, and the duration must not exceed \
         the contract's maximum (MAX_VALIDITY_WINDOW_SECS or MAX_SCHEDULE_WINDOW_SECS).";

    // ── NotFound ─────────────────────────────────────────────────────────────

    /// A medical record with the given ID does not exist.
    pub const NOT_FOUND_RECORD: &str =
        "NotFound: no medical record exists for the given record_id. \
         Verify the ID was returned by create_record.";

    /// An imaging order with the given ID does not exist.
    pub const NOT_FOUND_IMAGING_ORDER: &str =
        "NotFound: no imaging order exists for the given order_id. \
         Verify the ID was returned by order_imaging_study.";

    /// A patient registration does not exist.
    pub const NOT_FOUND_PATIENT: &str =
        "NotFound: no patient registration found for the given address. \
         The patient must call register_patient before this operation.";

    // ── ConsentNotGranted ─────────────────────────────────────────────────────

    /// The patient has not granted consent to the provider.
    pub const CONSENT_NOT_GRANTED: &str =
        "ConsentNotGranted: the patient has not granted consent to this provider. \
         The patient must call grant_consent(patient, provider) first.";

    // ── AlreadyExists ─────────────────────────────────────────────────────────

    /// The resource already exists and cannot be created again.
    pub const ALREADY_EXISTS: &str =
        "AlreadyExists: this resource has already been created. \
         Use the update or amend operation instead of create.";
}

#[cfg(test)]
mod tests {
    use super::hints;

    #[test]
    fn hints_are_non_empty() {
        assert!(!hints::UNAUTHORIZED_REQUIRES_PATIENT_AUTH.is_empty());
        assert!(!hints::UNAUTHORIZED_REQUIRES_PROVIDER_CONSENT.is_empty());
        assert!(!hints::UNAUTHORIZED_REQUIRES_ADMIN.is_empty());
        assert!(!hints::INVALID_INPUT_ENVELOPE_URI.is_empty());
        assert!(!hints::INVALID_INPUT_ZERO_HASH.is_empty());
        assert!(!hints::INVALID_INPUT_FUTURE_TIMESTAMP.is_empty());
        assert!(!hints::INVALID_INPUT_PAST_TIMESTAMP.is_empty());
        assert!(!hints::INVALID_INPUT_VALIDITY_WINDOW.is_empty());
        assert!(!hints::NOT_FOUND_RECORD.is_empty());
        assert!(!hints::NOT_FOUND_IMAGING_ORDER.is_empty());
        assert!(!hints::NOT_FOUND_PATIENT.is_empty());
        assert!(!hints::CONSENT_NOT_GRANTED.is_empty());
        assert!(!hints::ALREADY_EXISTS.is_empty());
    }

    #[test]
    fn hints_contain_no_pii_markers() {
        // Ensure no hint accidentally contains a placeholder that looks like
        // a real identifier (e.g. an address, a record ID value, a name).
        let all = [
            hints::UNAUTHORIZED_REQUIRES_PATIENT_AUTH,
            hints::UNAUTHORIZED_REQUIRES_PROVIDER_CONSENT,
            hints::UNAUTHORIZED_REQUIRES_ADMIN,
            hints::INVALID_INPUT_ENVELOPE_URI,
            hints::INVALID_INPUT_ZERO_HASH,
            hints::INVALID_INPUT_FUTURE_TIMESTAMP,
            hints::INVALID_INPUT_PAST_TIMESTAMP,
            hints::INVALID_INPUT_VALIDITY_WINDOW,
            hints::NOT_FOUND_RECORD,
            hints::NOT_FOUND_IMAGING_ORDER,
            hints::NOT_FOUND_PATIENT,
            hints::CONSENT_NOT_GRANTED,
            hints::ALREADY_EXISTS,
        ];
        for hint in &all {
            // No hint should contain a raw numeric ID or address-like string.
            assert!(
                !hint.contains("GABC") && !hint.contains("0x"),
                "hint contains potential PII: {hint}"
            );
        }
    }
}
