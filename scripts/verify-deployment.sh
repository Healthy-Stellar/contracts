#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

MANIFEST_FILE=""
TARGET_DIR="${TARGET_DIR:-${REPO_ROOT}/target/wasm32v1-none/release}"
VERBOSE=false

usage() {
    cat <<EOF
Usage: $(basename "$0") [options]

Verify deployed WASM hashes against a deployment manifest.
Rebuilds contracts from source and compares hashes.

Options:
  --manifest <path>      Path to deployment manifest JSON (required)
  --target-dir <dir>     Target directory for WASM artifacts (default: ${TARGET_DIR})
  -v, --verbose          Enable verbose output
  -h, --help             Show this help text
EOF
}

log() {
    if [[ "$VERBOSE" == true ]]; then
        printf '[verify-deployment] %s\n' "$*" >&2
    fi
}

error() {
    printf '[verify-deployment] error: %s\n' "$*" >&2
}

wasm_path_for() {
    local contract="$1"
    local wasm_name="${contract//-/_}.wasm"
    printf '%s/%s' "$TARGET_DIR" "$wasm_name"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --manifest)
            MANIFEST_FILE="$2"
            shift 2
            ;;
        --manifest=*)
            MANIFEST_FILE="${1#*=}"
            shift
            ;;
        --target-dir)
            TARGET_DIR="$2"
            shift 2
            ;;
        --target-dir=*)
            TARGET_DIR="${1#*=}"
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            error "unknown argument: $1"
            usage >&2
            exit 2
            ;;
    esac
done

if [[ -z "$MANIFEST_FILE" ]]; then
    error "manifest file is required"
    usage >&2
    exit 2
fi

if [[ ! -f "$MANIFEST_FILE" ]]; then
    error "manifest file not found: $MANIFEST_FILE"
    exit 1
fi

cd "$REPO_ROOT"

log "reading manifest from: $MANIFEST_FILE"
log "target directory: $TARGET_DIR"
log "rebuilding workspace..."
cargo build --target wasm32v1-none --release --workspace >/dev/null 2>&1

log "verifying contract hashes..."

passed=0
failed=0
missing=0

while IFS= read -r contract; do
    wasm_path="$(wasm_path_for "$contract")"

    if [[ ! -f "$wasm_path" ]]; then
        printf '%-40s MISSING (no WASM at %s)\n' "$contract" "$wasm_path"
        ((missing++))
        continue
    fi

    local_hash="$(sha256sum "$wasm_path" | awk '{print $1}')"
    manifest_hash="$(jq -r ".contracts[\"$contract\"].wasm_hash // empty" "$MANIFEST_FILE" 2>/dev/null || printf '')"

    if [[ -z "$manifest_hash" ]]; then
        printf '%-40s MISSING (not in manifest)\n' "$contract"
        ((missing++))
        continue
    fi

    if [[ "$local_hash" == "$manifest_hash" ]]; then
        printf '%-40s PASS\n' "$contract"
        ((passed++))
    else
        printf '%-40s FAIL (expected %s, got %s)\n' "$contract" "$manifest_hash" "$local_hash"
        ((failed++))
    fi
done < <(jq -r '.contracts | keys[]' "$MANIFEST_FILE" 2>/dev/null || printf '')

printf '\n%-40s %d passed, %d failed, %d missing\n' "Summary:" "$passed" "$failed" "$missing"

if [[ $failed -gt 0 || $missing -gt 0 ]]; then
    exit 1
fi

exit 0
