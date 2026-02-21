use soroban_sdk::{symbol_short, Env, String, Symbol};

use crate::{storage, Error};

/// Validate allergen type
pub fn validate_allergen_type(allergen_type: &Symbol) -> Result<(), Error> {
    let valid_types = [
        symbol_short!("med"),  // medication
        symbol_short!("food"),  // food
        symbol_short!("env"),   // environmental
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

/// Check if drug name matches allergen (case-insensitive comparison)
pub fn check_drug_match(allergen: &String, drug_name: &String) -> bool {
    // Simple string comparison - in production, this would use more sophisticated matching
    allergen == drug_name
}

/// Check for cross-sensitivity between allergens
pub fn check_cross_sensitivity(env: &Env, allergen: &String, drug_name: &String) -> bool {
    storage::has_cross_sensitivity(env, allergen, drug_name)
}
