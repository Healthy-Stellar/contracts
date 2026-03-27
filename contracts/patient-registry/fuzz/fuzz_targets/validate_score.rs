#![no_main]

use libfuzzer_sys::fuzz_target;
use patient_registry::validation::validate_score_i32;

// Must never panic: only Ok / Err.
fuzz_target!(|data: &[u8]| {
    let mut buf = [0u8; 4];
    let n = data.len().min(4);
    if n > 0 {
        buf[..n].copy_from_slice(&data[..n]);
    }
    let score = i32::from_le_bytes(buf);
    let _ = validate_score_i32(score);
});
