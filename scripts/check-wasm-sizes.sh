#!/bin/bash
# Script to check WASM binary sizes against limits (for CI)

set -e

# Target size limit (200 KB)
TARGET_SIZE=$((200 * 1024))

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Checking WASM binary sizes against limit (200 KB)..."
echo ""

WASM_DIR="target/wasm32-unknown-unknown/release/optimized"

if [ ! -d "$WASM_DIR" ]; then
    echo -e "${RED}Error: Optimized WASM directory not found.${NC}"
    echo "Please run 'make optimize' first."
    exit 1
fi

OVER_LIMIT=0
TOTAL_SIZE=0
CONTRACT_COUNT=0

for wasm_file in "$WASM_DIR"/*.wasm; do
    [ -e "$wasm_file" ] || continue
    
    filename=$(basename "$wasm_file")
    contract_name="${filename%.wasm}"
    
    # Get file size
    size=$(stat -f%z "$wasm_file" 2>/dev/null || stat -c%s "$wasm_file" 2>/dev/null)
    
    if [ -z "$size" ]; then
        continue
    fi
    
    size_kb=$(echo "scale=2; $size / 1024" | bc)
    limit_kb=$(echo "scale=2; $TARGET_SIZE / 1024" | bc)
    
    TOTAL_SIZE=$((TOTAL_SIZE + size))
    CONTRACT_COUNT=$((CONTRACT_COUNT + 1))
    
    if [ "$size" -gt "$TARGET_SIZE" ]; then
        echo -e "${RED}✗ $contract_name: ${size_kb} KB (exceeds ${limit_kb} KB limit)${NC}"
        OVER_LIMIT=$((OVER_LIMIT + 1))
    else
        echo -e "${GREEN}✓ $contract_name: ${size_kb} KB${NC}"
    fi
done

echo ""
echo "==================================="
echo "Summary"
echo "==================================="
echo "Total Contracts: $CONTRACT_COUNT"
echo "Contracts Over Limit: $OVER_LIMIT"

TOTAL_MB=$(echo "scale=2; $TOTAL_SIZE / 1024 / 1024" | bc)
echo "Total Size: ${TOTAL_MB} MB"
echo ""

if [ "$OVER_LIMIT" -gt 0 ]; then
    echo -e "${RED}FAILED: $OVER_LIMIT contract(s) exceed the size limit!${NC}"
    exit 1
else
    echo -e "${GREEN}PASSED: All contracts are within the size limit.${NC}"
    exit 0
fi
