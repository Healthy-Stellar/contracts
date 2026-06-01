#!/bin/bash
# Script to measure WASM binary sizes for all contracts

set -e

echo "==================================="
echo "WASM Binary Size Measurement"
echo "==================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Build all contracts first
echo "Building all contracts..."
cargo build --release --target wasm32-unknown-unknown --workspace

echo ""
echo "==================================="
echo "Binary Size Report"
echo "==================================="
echo ""

# Output file
REPORT_FILE="wasm-size-report.md"
CSV_FILE="wasm-sizes.csv"

# Initialize report
cat > "$REPORT_FILE" << 'EOF'
# WASM Binary Size Report

Generated: $(date -u '+%Y-%m-%d %H:%M:%S UTC')

## Summary

| Contract | Unoptimized Size | Optimized Size | Reduction | Status |
|----------|------------------|----------------|-----------|--------|
EOF

# Initialize CSV
echo "Contract,Unoptimized (bytes),Optimized (bytes),Reduction (%),Status" > "$CSV_FILE"

# Target size limit (200 KB)
TARGET_SIZE=$((200 * 1024))

# Counters
TOTAL_UNOPTIMIZED=0
TOTAL_OPTIMIZED=0
CONTRACT_COUNT=0
OVER_LIMIT=0

# Find all WASM files
WASM_DIR="target/wasm32-unknown-unknown/release"

if [ ! -d "$WASM_DIR" ]; then
    echo "Error: WASM directory not found. Please build contracts first."
    exit 1
fi

# Process each contract
for wasm_file in "$WASM_DIR"/*.wasm; do
    # Skip if no files found
    [ -e "$wasm_file" ] || continue
    
    # Skip optimized files
    if [[ "$wasm_file" == *"_optimized.wasm" ]]; then
        continue
    fi
    
    filename=$(basename "$wasm_file")
    contract_name="${filename%.wasm}"
    
    # Skip if it's a dependency or test file
    if [[ "$contract_name" == "deps" ]] || [[ "$contract_name" == *"test"* ]]; then
        continue
    fi
    
    # Get unoptimized size
    unopt_size=$(stat -f%z "$wasm_file" 2>/dev/null || stat -c%s "$wasm_file" 2>/dev/null)
    
    if [ -z "$unopt_size" ]; then
        echo "Warning: Could not get size for $filename"
        continue
    fi
    
    # Optimize with wasm-opt if available
    opt_file="${wasm_file%.wasm}_optimized.wasm"
    
    if command -v wasm-opt &> /dev/null; then
        wasm-opt -O4 -o "$opt_file" "$wasm_file" 2>/dev/null || {
            echo "Warning: wasm-opt failed for $filename, trying soroban optimize..."
            if command -v soroban &> /dev/null; then
                soroban contract optimize --wasm "$wasm_file" --wasm-out "$opt_file" 2>/dev/null || {
                    echo "Warning: soroban optimize also failed for $filename"
                    cp "$wasm_file" "$opt_file"
                }
            else
                cp "$wasm_file" "$opt_file"
            fi
        }
    elif command -v soroban &> /dev/null; then
        soroban contract optimize --wasm "$wasm_file" --wasm-out "$opt_file" 2>/dev/null || {
            echo "Warning: soroban optimize failed for $filename"
            cp "$wasm_file" "$opt_file"
        }
    else
        echo "Warning: Neither wasm-opt nor soroban found, skipping optimization for $filename"
        cp "$wasm_file" "$opt_file"
    fi
    
    # Get optimized size
    opt_size=$(stat -f%z "$opt_file" 2>/dev/null || stat -c%s "$opt_file" 2>/dev/null)
    
    # Calculate reduction
    reduction=0
    if [ "$unopt_size" -gt 0 ]; then
        reduction=$(( (unopt_size - opt_size) * 100 / unopt_size ))
    fi
    
    # Determine status
    status="✅ OK"
    if [ "$opt_size" -gt "$TARGET_SIZE" ]; then
        status="⚠️ OVER LIMIT"
        OVER_LIMIT=$((OVER_LIMIT + 1))
    fi
    
    # Format sizes
    unopt_kb=$(echo "scale=2; $unopt_size / 1024" | bc)
    opt_kb=$(echo "scale=2; $opt_size / 1024" | bc)
    
    # Add to report
    echo "| $contract_name | ${unopt_kb} KB | ${opt_kb} KB | ${reduction}% | $status |" >> "$REPORT_FILE"
    
    # Add to CSV
    echo "$contract_name,$unopt_size,$opt_size,$reduction,$status" >> "$CSV_FILE"
    
    # Update totals
    TOTAL_UNOPTIMIZED=$((TOTAL_UNOPTIMIZED + unopt_size))
    TOTAL_OPTIMIZED=$((TOTAL_OPTIMIZED + opt_size))
    CONTRACT_COUNT=$((CONTRACT_COUNT + 1))
    
    # Print to console
    printf "%-40s %10s KB -> %10s KB (%3d%% reduction) %s\n" \
        "$contract_name" "$unopt_kb" "$opt_kb" "$reduction" "$status"
done

# Calculate total reduction
TOTAL_REDUCTION=0
if [ "$TOTAL_UNOPTIMIZED" -gt 0 ]; then
    TOTAL_REDUCTION=$(( (TOTAL_UNOPTIMIZED - TOTAL_OPTIMIZED) * 100 / TOTAL_UNOPTIMIZED ))
fi

# Format totals
TOTAL_UNOPT_MB=$(echo "scale=2; $TOTAL_UNOPTIMIZED / 1024 / 1024" | bc)
TOTAL_OPT_MB=$(echo "scale=2; $TOTAL_OPTIMIZED / 1024 / 1024" | bc)

# Add summary to report
cat >> "$REPORT_FILE" << EOF

## Statistics

- **Total Contracts:** $CONTRACT_COUNT
- **Total Unoptimized Size:** ${TOTAL_UNOPT_MB} MB
- **Total Optimized Size:** ${TOTAL_OPT_MB} MB
- **Total Reduction:** ${TOTAL_REDUCTION}%
- **Contracts Over Limit:** $OVER_LIMIT
- **Target Size Limit:** 200 KB

## Optimization Tools Used

- wasm-opt (Binaryen) with -O4 flag
- soroban contract optimize (fallback)

## Recommendations

EOF

if [ "$OVER_LIMIT" -gt 0 ]; then
    cat >> "$REPORT_FILE" << EOF
⚠️ **$OVER_LIMIT contract(s) exceed the 200 KB size limit.**

Recommended actions:
1. Review large contracts for code duplication
2. Extract shared functionality to libraries
3. Remove unused dependencies
4. Use more aggressive optimization flags
5. Consider splitting large contracts into smaller modules

EOF
else
    cat >> "$REPORT_FILE" << EOF
✅ **All contracts are within the 200 KB size limit.**

Continue monitoring binary sizes to prevent regression.

EOF
fi

cat >> "$REPORT_FILE" << EOF
## Size Limit Rationale

The 200 KB limit is based on:
- Stellar network storage costs
- Deployment transaction size limits
- Best practices for smart contract size
- Balance between functionality and efficiency

EOF

echo ""
echo "==================================="
echo "Summary"
echo "==================================="
echo "Total Contracts: $CONTRACT_COUNT"
echo "Total Unoptimized: ${TOTAL_UNOPT_MB} MB"
echo "Total Optimized: ${TOTAL_OPT_MB} MB"
echo "Total Reduction: ${TOTAL_REDUCTION}%"
echo "Contracts Over Limit: $OVER_LIMIT"
echo ""
echo "Report saved to: $REPORT_FILE"
echo "CSV data saved to: $CSV_FILE"
echo ""

# Exit with error if any contracts are over limit
if [ "$OVER_LIMIT" -gt 0 ]; then
    echo -e "${RED}ERROR: $OVER_LIMIT contract(s) exceed the size limit!${NC}"
    exit 1
else
    echo -e "${GREEN}SUCCESS: All contracts are within the size limit.${NC}"
    exit 0
fi
