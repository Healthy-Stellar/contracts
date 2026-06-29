/// Test to verify that all #[contracterror] enums have unique and sequential discriminants starting from 1
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use regex::Regex;

#[test]
fn validate_error_enum_discriminants() {
    let workspace_root = env!("CARGO_MANIFEST_DIR");
    let contracts_dir = Path::new(workspace_root).parent().unwrap();

    let mut errors = Vec::new();

    // Find all lib.rs files in contracts
    for entry in fs::read_dir(contracts_dir).expect("Failed to read contracts dir") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.is_dir() && !path.file_name().map_or(false, |n| n == "shared") {
            let lib_path = path.join("src").join("lib.rs");

            if lib_path.exists() {
                if let Err(e) = validate_lib_file(&lib_path) {
                    errors.push(format!("{}: {}", lib_path.display(), e));
                }
            }
        }
    }

    if !errors.is_empty() {
        panic!("Error enum validation failed:\n{}", errors.join("\n"));
    }
}

fn validate_lib_file(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Find all #[contracterror] enum blocks
    let contracterror_regex = Regex::new(
        r#"#\[contracterror\]\s*\n(?:[^\n]*\n)*?pub enum (\w+)\s*\{([^}]+)\}"#
    ).unwrap();

    for caps in contracterror_regex.captures_iter(&content) {
        let enum_name = caps.get(1).unwrap().as_str();
        let enum_body = caps.get(2).unwrap().as_str();

        validate_enum_variants(enum_name, enum_body, path)?;
    }

    Ok(())
}

fn validate_enum_variants(enum_name: &str, body: &str, path: &Path) -> Result<(), String> {
    // Extract all discriminant values
    let discriminant_regex = Regex::new(r"(\w+)\s*=\s*(\d+)").unwrap();

    let mut discriminants: Vec<(String, u32)> = Vec::new();
    let mut seen = HashSet::new();

    for caps in discriminant_regex.captures_iter(body) {
        let variant_name = caps.get(1).unwrap().as_str();
        let value_str = caps.get(2).unwrap().as_str();

        let value: u32 = value_str.parse()
            .map_err(|_| format!("Invalid discriminant value: {}", value_str))?;

        // Check for duplicates
        if !seen.insert(value) {
            return Err(format!(
                "Duplicate discriminant {} in enum {} at {}",
                value, enum_name, path.display()
            ));
        }

        discriminants.push((variant_name.to_string(), value));
    }

    if discriminants.is_empty() {
        return Ok(()); // No enum variants with explicit discriminants
    }

    // Check if sequential starting from 1
    discriminants.sort_by_key(|k| k.1);

    for (i, (_variant, value)) in discriminants.iter().enumerate() {
        let expected = (i + 1) as u32;
        if *value != expected {
            return Err(format!(
                "Non-sequential discriminant in enum {}: expected {}, got {} at {}",
                enum_name, expected, value, path.display()
            ));
        }
    }

    Ok(())
}
