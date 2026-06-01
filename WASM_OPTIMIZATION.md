# WASM Binary Size Optimization Guide

## Overview

This document describes the WASM binary size optimization strategy for the Healthy-Stellar contracts repository. Soroban contracts are charged storage fees proportional to their on-chain footprint, making binary size optimization critical for cost efficiency.

## Size Limit

**Target Size Limit:** 200 KB per contract (optimized)

### Rationale

The 200 KB limit is based on:
- **Stellar Network Storage Costs:** Smaller binaries reduce deployment and hosting costs
- **Transaction Size Limits:** Deployment transactions have size constraints
- **Best Practices:** Industry standard for smart contract efficiency
- **Performance:** Smaller binaries load and execute faster
- **Network Efficiency:** Reduces bandwidth requirements

## Current Status

Run `make measure-sizes` to generate a current size report for all contracts.

## Optimization Tools

### 1. wasm-opt (Binaryen)

Primary optimization tool with aggressive size reduction.

**Installation:**
```bash
# macOS
brew install binaryen

# Linux
apt-get install binaryen

# Or download from: https://github.com/WebAssembly/binaryen/releases
```

**Usage:**
```bash
wasm-opt -O4 -o output.wasm input.wasm
```

**Optimization Levels:**
- `-O0`: No optimization
- `-O1`: Basic optimization
- `-O2`: More optimization
- `-O3`: Aggressive optimization
- `-O4`: Maximum optimization (size-focused)
- `-Oz`: Optimize for size

### 2. soroban contract optimize

Soroban's built-in optimization tool.

**Installation:**
```bash
cargo install soroban-cli --features opt
```

**Usage:**
```bash
soroban contract optimize --wasm input.wasm --wasm-out output.wasm
```

### 3. wasm-strip

Removes debug symbols and metadata.

**Installation:**
```bash
# Part of WABT toolkit
# macOS
brew install wabt

# Linux
apt-get install wabt
```

**Usage:**
```bash
wasm-strip input.wasm -o output.wasm
```

### 4. twiggy

Code size profiler for identifying large functions.

**Installation:**
```bash
cargo install twiggy
```

**Usage:**
```bash
# Show top size contributors
twiggy top input.wasm

# Show dominator tree
twiggy dominators input.wasm

# Show paths to a function
twiggy paths input.wasm
```

## Makefile Targets

### Build and Optimize

```bash
# Build WASM binaries
make build-wasm

# Optimize all binaries
make optimize

# Measure sizes
make measure-sizes

# Check against limits (CI)
make check-sizes

# Profile binaries
make profile-wasm

# Strip debug symbols
make strip-wasm
```

## Optimization Strategies

### 1. Compiler Optimizations

Add to `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"        # Optimize for size
lto = true             # Enable Link Time Optimization
codegen-units = 1      # Better optimization (slower compile)
strip = true           # Strip symbols
panic = "abort"        # Smaller panic handler
overflow-checks = false # Disable overflow checks (use carefully)
```

### 2. Dependency Management

**Review Dependencies:**
```bash
# Show dependency tree
cargo tree

# Find unused dependencies
cargo install cargo-udeps
cargo +nightly udeps
```

**Optimize Dependencies:**
- Use feature flags to include only needed functionality
- Replace heavy dependencies with lighter alternatives
- Avoid dependencies with large transitive dependency trees

**Example:**
```toml
[dependencies]
# Instead of full serde
serde = { version = "1.0", default-features = false, features = ["derive"] }

# Use soroban-sdk types instead of external types
soroban-sdk = "23.0.0"
```

### 3. Code Sharing

**Extract Common Code:**
- Create shared libraries for common functionality
- Avoid code duplication across contracts
- Use workspace dependencies

**Example Structure:**
```
contracts/
  shared/           # Shared utilities
  contract-a/       # Uses shared
  contract-b/       # Uses shared
```

### 4. Remove Dead Code

**Conditional Compilation:**
```rust
#[cfg(not(test))]
fn test_only_function() {
    // Only compiled in tests
}

#[cfg(feature = "debug")]
fn debug_function() {
    // Only with debug feature
}
```

**Remove Unused Imports:**
```bash
cargo clippy -- -W unused-imports
```

### 5. Optimize Data Structures

**Use Efficient Types:**
```rust
// Instead of String
use soroban_sdk::String;

// Instead of Vec
use soroban_sdk::Vec;

// Use compact representations
use u32 instead of u64 where possible
```

### 6. Function Inlining

**Strategic Inlining:**
```rust
#[inline(always)]
fn small_hot_function() {
    // Frequently called small function
}

#[inline(never)]
fn large_cold_function() {
    // Rarely called large function
}
```

### 7. Macro Usage

**Avoid Heavy Macros:**
- Macros can generate significant code
- Use functions instead where possible
- Be selective with derive macros

### 8. Error Handling

**Compact Error Types:**
```rust
#[contracterror]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    Unauthorized = 2,
    // Use u32 error codes instead of strings
}
```

## Profiling Workflow

### 1. Measure Baseline

```bash
make build-wasm
make measure-sizes
```

This generates:
- `wasm-size-report.md` - Detailed size report
- `wasm-sizes.csv` - CSV data for analysis

### 2. Profile Large Contracts

```bash
make profile-wasm
```

This generates:
- `wasm-profiles/profile-summary.md` - Overview
- `wasm-profiles/*_top.txt` - Top size contributors
- `wasm-profiles/*_dominators.txt` - Dominator analysis
- `wasm-profiles/*_paths.txt` - Call paths
- `wasm-profiles/size-comparison.txt` - Visual comparison

### 3. Identify Optimization Opportunities

Review profile data to find:
- Large functions that can be optimized
- Duplicated code across contracts
- Heavy dependencies
- Unused code

### 4. Apply Optimizations

Implement optimization strategies based on profile data.

### 5. Measure Results

```bash
make measure-sizes
```

Compare before/after sizes to verify improvements.

## CI Integration

### Automatic Size Checks

The `wasm-size-check.yml` workflow runs on every PR:

1. Builds all contracts
2. Optimizes binaries
3. Checks sizes against 200 KB limit
4. **Fails CI if any contract exceeds limit**
5. Posts size report as PR comment
6. Uploads size report as artifact

### Viewing Results

- **PR Comments:** Size report posted automatically
- **GitHub Actions:** Check "WASM Size Check" workflow
- **Artifacts:** Download detailed reports

## Troubleshooting

### Contract Exceeds Size Limit

**Steps to resolve:**

1. **Profile the contract:**
   ```bash
   make profile-wasm
   ```

2. **Review top contributors:**
   ```bash
   cat wasm-profiles/CONTRACT_NAME_top.txt
   ```

3. **Check dependencies:**
   ```bash
   cargo tree -p CONTRACT_NAME
   ```

4. **Apply optimizations:**
   - Remove unused dependencies
   - Extract shared code
   - Optimize data structures
   - Use more aggressive compiler flags

5. **Measure improvement:**
   ```bash
   make measure-sizes
   ```

### Optimization Not Working

**Check:**
- wasm-opt is installed and in PATH
- Cargo.toml has release optimizations
- No debug features enabled
- LTO is enabled

### Build Failures

**Common issues:**
- Missing wasm32-unknown-unknown target
- Incompatible dependencies
- Feature flag conflicts

**Solutions:**
```bash
# Add WASM target
rustup target add wasm32-unknown-unknown

# Clean and rebuild
cargo clean
make build-wasm
```

## Best Practices

### Development

1. **Monitor Size Early:** Check sizes during development, not just before release
2. **Profile Regularly:** Run profiling after major changes
3. **Review Dependencies:** Audit dependencies before adding
4. **Share Code:** Extract common functionality to shared libraries
5. **Test Optimizations:** Ensure optimizations don't break functionality

### Code Review

1. **Check Size Impact:** Review size changes in PRs
2. **Question Large Additions:** Investigate significant size increases
3. **Verify Optimization:** Ensure new code follows optimization guidelines
4. **Document Decisions:** Explain trade-offs between size and functionality

### Deployment

1. **Always Optimize:** Never deploy unoptimized binaries
2. **Verify Sizes:** Check sizes before deployment
3. **Document Sizes:** Record deployed binary sizes
4. **Monitor Costs:** Track storage costs over time

## Size Reduction Examples

### Example 1: Dependency Optimization

**Before:**
```toml
[dependencies]
serde = "1.0"
serde_json = "1.0"
```

**After:**
```toml
[dependencies]
# Use soroban-sdk types instead
soroban-sdk = "23.0.0"
```

**Result:** 50 KB reduction

### Example 2: Code Sharing

**Before:**
- Contract A: 180 KB (includes validation logic)
- Contract B: 190 KB (includes same validation logic)

**After:**
- Shared library: 30 KB (validation logic)
- Contract A: 150 KB (uses shared)
- Contract B: 160 KB (uses shared)

**Result:** 50 KB total reduction

### Example 3: Compiler Optimization

**Before:**
```toml
[profile.release]
opt-level = 3
```

**After:**
```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

**Result:** 20-30% size reduction

## Monitoring and Reporting

### Weekly Review

1. Run `make measure-sizes`
2. Review size trends
3. Identify growing contracts
4. Plan optimization work

### Monthly Audit

1. Full profiling of all contracts
2. Dependency audit
3. Shared code opportunities
4. Update optimization strategies

### Quarterly Goals

1. Reduce total binary size by X%
2. Bring all contracts under limit
3. Improve optimization tooling
4. Update best practices

## Tools Reference

### Installation Commands

```bash
# Rust and Cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# WASM target
rustup target add wasm32-unknown-unknown

# Binaryen (wasm-opt)
brew install binaryen  # macOS
apt-get install binaryen  # Linux

# WABT (wasm-strip, wasm-objdump)
brew install wabt  # macOS
apt-get install wabt  # Linux

# Soroban CLI
cargo install soroban-cli --features opt

# Twiggy
cargo install twiggy

# cargo-udeps
cargo install cargo-udeps
```

### Quick Reference

| Tool | Purpose | Command |
|------|---------|---------|
| wasm-opt | Optimize binary | `wasm-opt -O4 -o out.wasm in.wasm` |
| wasm-strip | Remove symbols | `wasm-strip in.wasm -o out.wasm` |
| twiggy | Profile size | `twiggy top in.wasm` |
| soroban | Optimize | `soroban contract optimize --wasm in.wasm` |
| cargo-udeps | Find unused deps | `cargo +nightly udeps` |

## Additional Resources

- [Soroban Documentation](https://soroban.stellar.org/)
- [Binaryen GitHub](https://github.com/WebAssembly/binaryen)
- [WABT GitHub](https://github.com/WebAssembly/wabt)
- [Twiggy GitHub](https://github.com/rustwasm/twiggy)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)
- [Cargo Profile Documentation](https://doc.rust-lang.org/cargo/reference/profiles.html)

## Support

For questions or issues:
1. Check this documentation
2. Review profile data
3. Consult team members
4. Create an issue with profile data attached

---

**Remember:** Binary size optimization is an ongoing process. Regular monitoring and proactive optimization prevent size creep and keep deployment costs low.
