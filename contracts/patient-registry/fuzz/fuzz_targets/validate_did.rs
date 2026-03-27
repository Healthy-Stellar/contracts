#![no_main]

use libfuzzer_sys::fuzz_target;
use patient_registry::validation::validate_did_bytes;

// Must never panic: only Ok / Err.
fuzz_target!(|data: &[u8]| {
    let _ = validate_did_bytes(data);
});
