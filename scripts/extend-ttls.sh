#!/usr/bin/env bash
set -euo pipefail

# Script to extend Soroban contract instance storage TTLs on Stellar Mainnet
# Designed to run as a periodic GitHub Actions cron job (every 90 days)

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

NETWORK="${NETWORK:-mainnet}"
IDENTITY="${STELLAR_IDENTITY:-default}"
CLI_BIN="${CLI_BIN:-stellar}"
MANIFEST_DIR="${MANIFEST_DIR:-${REPO_ROOT}/deployments}"
LEDGERS_TO_EXTEND="${LEDGERS_TO_EXTEND:-535680}"  # ~1 year at 5s per ledger
CRITICAL_THRESHOLD="${CRITICAL_THRESHOLD:-86400}"  # Alert if TTL < 1 day remaining
DRY_RUN="${DRY_RUN:-false}"

usage() {
    cat <<EOF
Usage: $(basename "$0") [options]

Extend instance storage TTLs for all deployed contracts on Stellar Mainnet.

Options:
  --network <name>              Stellar network name (default: mainnet)
  --identity <name>             Stellar CLI source identity (default: STELLAR_IDENTITY or default)
  --ledgers-to-extend <count>   Ledgers to extend TTL (default: 535680, ~1 year)
  --critical-threshold <ledgers> Alert threshold (default: 86400, ~1 day)
  --dry-run                     Show what would be extended without executing
  --cli-bin <binary>            Stellar CLI binary (default: stellar)
  -h, --help                    Show this help text
EOF
}

log() {
    printf '[extend-ttls] %s\n' "$*" >&2
}

warn() {
    printf '[extend-ttls] WARNING: %s\n' "$*" >&2
}

die() {
    printf '[extend-ttls] ERROR: %s\n' "$*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --network)
            NETWORK="$2"
            shift 2
            ;;
        --network=*)
            NETWORK="${1#*=}"
            shift
            ;;
        --identity)
            IDENTITY="$2"
            shift 2
            ;;
        --identity=*)
            IDENTITY="${1#*=}"
            shift
            ;;
        --ledgers-to-extend)
            LEDGERS_TO_EXTEND="$2"
            shift 2
            ;;
        --ledgers-to-extend=*)
            LEDGERS_TO_EXTEND="${1#*=}"
            shift
            ;;
        --critical-threshold)
            CRITICAL_THRESHOLD="$2"
            shift 2
            ;;
        --critical-threshold=*)
            CRITICAL_THRESHOLD="${1#*=}"
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --cli-bin)
            CLI_BIN="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"
}

extract_contract_ids() {
    local manifest="$1"
    [[ -f "$manifest" ]] || die "Manifest not found: $manifest"

    # Extract all contract IDs (format: C<55 chars> from Stellar contract addresses)
    grep -Eo '"([^"]+)": "[C][A-Z2-7]{55}"' "$manifest" | sed 's/.*": "\([^"]*\)".*/\1/'
}

extend_contract_ttl() {
    local contract_id="$1"

    if [[ "$DRY_RUN" == true ]]; then
        log "[DRY RUN] Would extend TTL for: $contract_id (+$LEDGERS_TO_EXTEND ledgers)"
        return 0
    fi

    log "Extending TTL for: $contract_id (+$LEDGERS_TO_EXTEND ledgers)"
    "$CLI_BIN" contract extend \
        --network "$NETWORK" \
        --source "$IDENTITY" \
        --id "$contract_id" \
        --ledgers-to-extend "$LEDGERS_TO_EXTEND"
}

check_ttl_threshold() {
    local contract_id="$1"
    local current_ttl

    # Note: This is a placeholder. Actual TTL check would require:
    # - Stellar SDK to query ledger entry
    # - Or a separate RPC call to inspect contract TTL
    # For now, we log that manual checks are recommended

    log "Recommend manual verification of TTL for: $contract_id"
}

require_cmd "$CLI_BIN"

cd "$REPO_ROOT"
MANIFEST="${MANIFEST_DIR}/${NETWORK}.json"

log "Extending TTLs for $NETWORK"
log "Network: $NETWORK"
log "Identity: $IDENTITY"
log "Ledgers to extend: $LEDGERS_TO_EXTEND"
log "Dry run: $DRY_RUN"

if [[ ! -f "$MANIFEST" ]]; then
    die "Manifest not found at: $MANIFEST"
fi

CONTRACT_IDS=($(extract_contract_ids "$MANIFEST"))

if [[ ${#CONTRACT_IDS[@]} -eq 0 ]]; then
    die "No contract IDs found in manifest: $MANIFEST"
fi

log "Found ${#CONTRACT_IDS[@]} contracts to extend"

EXTENDED_COUNT=0
FAILED_COUNT=0

for contract_id in "${CONTRACT_IDS[@]}"; do
    if extend_contract_ttl "$contract_id"; then
        EXTENDED_COUNT=$((EXTENDED_COUNT + 1))
        check_ttl_threshold "$contract_id"
    else
        FAILED_COUNT=$((FAILED_COUNT + 1))
        warn "Failed to extend TTL for: $contract_id"
    fi
done

log
log "TTL Extension Summary:"
log "  Extended: $EXTENDED_COUNT"
log "  Failed: $FAILED_COUNT"
log "  Total: ${#CONTRACT_IDS[@]}"

if [[ $FAILED_COUNT -gt 0 ]]; then
    warn "Some contracts failed to extend. Please review and retry."
    exit 1
fi

log "All TTLs extended successfully."
