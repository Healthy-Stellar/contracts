use soroban_sdk::{symbol_short, Bytes, Env, String, Symbol};

use crate::{storage, Error};

/// Validate allergen type
pub fn validate_allergen_type(allergen_type: &Symbol) -> Result<(), Error> {
    let valid_types = [
        symbol_short!("med"),  // medication
        symbol_short!("food"), // food
        symbol_short!("env"),  // environmental
    ];

    if valid_types.contains(allergen_type) {
        Ok(())
    } else {
        Err(Error::InvalidAllergenType)
    }
}

/// Validate severity level
pub fn validate_severity(severity: &Symbol) -> Result<(), Error> {
    let valid_severities = [
        symbol_short!("mild"),
        symbol_short!("moderate"),
        symbol_short!("severe"),
        symbol_short!("critical"),
    ];

    if valid_severities.contains(severity) {
        Ok(())
    } else {
        Err(Error::InvalidSeverity)
    }
}

/// Check if a drug name matches an allergen.
///
/// Allergens are stored as encrypted envelopes, so the plaintext allergen is
/// never available on-chain. Instead, callers supply the keyed hash
/// (`HMAC(drug_code)`) of the normalized allergen code that was stored next to
/// the envelope at record time. The interaction check compares that stored
/// hash against the keyed hash of the incoming drug code, so no plaintext
/// comparison happens on-chain.
pub fn check_drug_match(allergen_hash: &Bytes, drug_hash: &Bytes) -> bool {
    allergen_hash == drug_hash
}

/// Normalize a Soroban `String` to ASCII lowercase (A-Z => a-z).
///
/// Byte-level normalization; non-ASCII bytes are left untouched.
fn to_lowercase(value: &String) -> String {
    let mut bytes = value.to_bytes();
    for i in 0..bytes.len() {
        if let Some(byte) = bytes.get(i) {
            bytes.set(i, byte.to_ascii_lowercase());
        }
    }
    String::from(&bytes)
}

/// Compute the keyed hash used to compare a drug code against a stored
/// allergen without exposing plaintext on-chain.
///
/// The input is normalized to ASCII lowercase before hashing so that
/// "Penicillin" and "penicillin" produce the same keyed hash, preserving the
/// case-insensitive matching behaviour of the previous plaintext comparison.
pub fn drug_code_hash(env: &Env, drug_code: &String) -> Bytes {
    let normalized = to_lowercase(drug_code);
    env.crypto().sha256(&normalized.to_bytes()).to_bytes()
}

/// Check for cross-sensitivity between allergens
pub fn check_cross_sensitivity(env: &Env, allergen: &String, drug_name: &String) -> bool {
    storage::has_cross_sensitivity(env, allergen, drug_name)
}
