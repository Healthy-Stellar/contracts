#!/bin/bash
# Script to profile WASM binaries and identify optimization opportunities

set -e

echo "==================================="
echo "WASM Binary Profiling"
echo "==================================="
echo ""

WASM_DIR="target/wasm32-unknown-unknown/release"
PROFILE_DIR="wasm-profiles"

mkdir -p "$PROFILE_DIR"

if [ ! -d "$WASM_DIR" ]; then
    echo "Error: WASM directory not found. Please build contracts first."
    exit 1
fi

# Check for required tools
TWIGGY_AVAILABLE=false
WASM_OBJDUMP_AVAILABLE=false

if command -v twiggy &> /dev/null; then
    TWIGGY_AVAILABLE=true
    echo "✓ twiggy found (for code size profiling)"
else
    echo "⚠ twiggy not found. Install with: cargo install twiggy"
fi

if command -v wasm-objdump &> /dev/null; then
    WASM_OBJDUMP_AVAILABLE=true
    echo "✓ wasm-objdump found (for binary inspection)"
else
    echo "⚠ wasm-objdump not found. Install WABT toolkit."
fi

echo ""

# Create summary report
REPORT_FILE="$PROFILE_DIR/profile-summary.md"

cat > "$REPORT_FILE" << 'EOF'
# WASM Binary Profile Report

Generated: $(date -u '+%Y-%m-%d %H:%M:%S UTC')

## Contract Profiles

EOF

for wasm_file in "$WASM_DIR"/*.wasm; do
    [ -e "$wasm_file" ] || continue
    
    if [[ "$wasm_file" == *"_optimized.wasm" ]] || [[ "$wasm_file" == *"_stripped.wasm" ]]; then
        continue
    fi
    
    filename=$(basename "$wasm_file")
    contract_name="${filename%.wasm}"
    
    echo "Profiling $contract_name..."
    
    # Get basic info
    size=$(stat -f%z "$wasm_file" 2>/dev/null || stat -c%s "$wasm_file" 2>/dev/null)
    size_kb=$(echo "scale=2; $size / 1024" | bc)
    
    cat >> "$REPORT_FILE" << EOF

### $contract_name

- **Size:** ${size_kb} KB
- **File:** $filename

EOF
    
    # Run twiggy if available
    if [ "$TWIGGY_AVAILABLE" = true ]; then
        echo "  Running twiggy top..."
        twiggy top -n 20 "$wasm_file" > "$PROFILE_DIR/${contract_name}_top.txt" 2>/dev/null || true
        
        echo "  Running twiggy dominators..."
        twiggy dominators -n 20 "$wasm_file" > "$PROFILE_DIR/${contract_name}_dominators.txt" 2>/dev/null || true
        
        echo "  Running twiggy paths..."
        twiggy paths "$wasm_file" > "$PROFILE_DIR/${contract_name}_paths.txt" 2>/dev/null || true
        
        # Add top items to report
        cat >> "$REPORT_FILE" << EOF
#### Top 10 Size Contributors

\`\`\`
EOF
        head -n 11 "$PROFILE_DIR/${contract_name}_top.txt" >> "$REPORT_FILE" 2>/dev/null || echo "N/A" >> "$REPORT_FILE"
        cat >> "$REPORT_FILE" << EOF
\`\`\`

EOF
    fi
    
    # Run wasm-objdump if available
    if [ "$WASM_OBJDUMP_AVAILABLE" = true ]; then
        echo "  Running wasm-objdump..."
        wasm-objdump -h "$wasm_file" > "$PROFILE_DIR/${contract_name}_sections.txt" 2>/dev/null || true
        
        # Count functions
        func_count=$(wasm-objdump -x "$wasm_file" 2>/dev/null | grep -c "func\[" || echo "0")
        
        cat >> "$REPORT_FILE" << EOF
#### Binary Structure

- **Function Count:** $func_count
- **Sections:** See \`${contract_name}_sections.txt\`

EOF
    fi
    
    # Analyze imports/exports
    if command -v wasm2wat &> /dev/null; then
        echo "  Analyzing imports/exports..."
        wasm2wat "$wasm_file" 2>/dev/null | grep -E "(import|export)" | head -n 20 > "$PROFILE_DIR/${contract_name}_imports_exports.txt" || true
    fi
done

# Add recommendations
cat >> "$REPORT_FILE" << 'EOF'

## Optimization Recommendations

### General Strategies

1. **Remove Unused Code**
   - Use `cargo-udeps` to find unused dependencies
   - Remove dead code with `#[cfg(not(test))]` for test-only code
   - Enable LTO (Link Time Optimization) in Cargo.toml

2. **Optimize Dependencies**
   - Review dependency tree with `cargo tree`
   - Replace heavy dependencies with lighter alternatives
   - Use feature flags to include only needed functionality

3. **Code Sharing**
   - Extract common code to shared libraries
   - Avoid code duplication across contracts
   - Use workspace dependencies efficiently

4. **Compiler Optimizations**
   - Use `opt-level = "z"` for size optimization
   - Enable `lto = true` for link-time optimization
   - Use `codegen-units = 1` for better optimization
   - Enable `strip = true` to remove debug symbols

5. **WASM-Specific Optimizations**
   - Use `wasm-opt -O4` for aggressive optimization
   - Strip debug symbols with `wasm-strip`
   - Use `wasm-snip` to remove specific functions

### Contract-Specific Actions

Review the profile data in `wasm-profiles/` directory for each contract:
- `*_top.txt` - Largest code contributors
- `*_dominators.txt` - Functions that dominate size
- `*_paths.txt` - Call paths to large functions
- `*_sections.txt` - WASM section sizes
- `*_imports_exports.txt` - External dependencies

### Tools Used

- **twiggy** - Code size profiler for WASM
- **wasm-objdump** - Binary inspection tool
- **wasm2wat** - WASM to WAT converter

### Installation

```bash
# Install twiggy
cargo install twiggy

# Install WABT (includes wasm-objdump, wasm2wat, wasm-strip)
# macOS: brew install wabt
# Linux: apt-get install wabt
# Or build from source: https://github.com/WebAssembly/wabt
```

EOF

echo ""
echo "Profiling complete!"
echo "Report saved to: $REPORT_FILE"
echo "Detailed profiles in: $PROFILE_DIR/"
echo ""

# Generate size comparison chart
echo "Generating size comparison..."

cat > "$PROFILE_DIR/size-comparison.txt" << EOF
Contract Size Comparison
========================

EOF

for wasm_file in "$WASM_DIR"/*.wasm; do
    [ -e "$wasm_file" ] || continue
    
    if [[ "$wasm_file" == *"_optimized.wasm" ]] || [[ "$wasm_file" == *"_stripped.wasm" ]]; then
        continue
    fi
    
    filename=$(basename "$wasm_file")
    contract_name="${filename%.wasm}"
    size=$(stat -f%z "$wasm_file" 2>/dev/null || stat -c%s "$wasm_file" 2>/dev/null)
    size_kb=$(echo "scale=2; $size / 1024" | bc)
    
    # Create bar chart
    bar_length=$(echo "scale=0; $size / 2048" | bc)
    bar=$(printf '█%.0s' $(seq 1 $bar_length))
    
    printf "%-40s %8s KB %s\n" "$contract_name" "$size_kb" "$bar" >> "$PROFILE_DIR/size-comparison.txt"
done

cat "$PROFILE_DIR/size-comparison.txt"

echo ""
echo "Size comparison saved to: $PROFILE_DIR/size-comparison.txt"
