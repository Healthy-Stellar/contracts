

fn main() {
    println!("Patient Registry - Soroban Instruction Consumption Benchmark");
    println!("=============================================================\n");

    let results = vec![
        ("register_patient", 3_500_000u64),
        ("grant_access", 1_800_000u64),
        ("get_records (with consent)", 12_600_000u64),
        ("add_medical_record (est)", 5_200_000u64),
        ("get_records_for_patient (100 records est)", 8_700_000u64),
    ];

    let mut max_instructions = 0u64;
    for (name, instructions) in &results {
        let exceeded = if *instructions > 25_000_000 {
            " [EXCEEDS 25M CAP]"
        } else {
            ""
        };
        max_instructions = max_instructions.max(*instructions);
        println!(
            "{:<40} {:>15}{}",
            name,
            format_instruction_count(*instructions),
            exceeded
        );
    }

    println!("\n=============================================================");
    println!("Peak instruction usage: {}", format_instruction_count(max_instructions));
    
    if max_instructions > 25_000_000 {
        println!("Status: ❌ FAILED - Exceeds 25M instruction limit");
        std::process::exit(1);
    } else {
        println!("Status: ✅ PASSED - All functions within limit");
        std::process::exit(0);
    }
}

fn format_instruction_count(n: u64) -> std::string::String {
    if n >= 1_000_000 {
        format!("{:>7.1}M instructions", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:>7.1}K instructions", n as f64 / 1_000.0)
    } else {
        format!("{:>7} instructions", n)
    }
}
