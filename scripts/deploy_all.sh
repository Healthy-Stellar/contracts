#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

NETWORK="testnet"
IDENTITY="${STELLAR_IDENTITY:-default}"
DRY_RUN=false
SKIP_BUILD=false
SKIP_OPTIMIZE=false
SKIP_INIT=false
CLI_BIN="${CLI_BIN:-stellar}"
TARGET_DIR="${TARGET_DIR:-${REPO_ROOT}/target/wasm32-unknown-unknown/release}"
MANIFEST_DIR="${MANIFEST_DIR:-${REPO_ROOT}/deployments}"
ADMIN_ADDRESS="${ADMIN_ADDRESS:-}"

usage() {
    cat <<EOF
Usage: $(basename "$0") [options]

Deploy all Healthy Stellar contracts in dependency order and write
deployments/<network>.json.

Options:
  --network <name>           Stellar network name (default: testnet)
  --identity <name>          Stellar CLI source identity (default: STELLAR_IDENTITY or default)
  --admin-address <address>  Admin address for initialize calls
  --dry-run                  Print the deployment plan without building or submitting txs
  --skip-build               Use existing WASM artifacts
  --skip-optimize            Skip WASM optimization step
  --skip-init                Skip initialize calls on deployed contracts
  --cli-bin <binary>         Stellar CLI binary (default: stellar)
  -h, --help                 Show this help text
EOF
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
        --admin-address)
            ADMIN_ADDRESS="$2"
            shift 2
            ;;
        --admin-address=*)
            ADMIN_ADDRESS="${1#*=}"
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --skip-optimize)
            SKIP_OPTIMIZE=true
            shift
            ;;
        --skip-init)
            SKIP_INIT=true
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

CONTRACTS=(
    ttl-config
    shared
    access-control
    provider-registry
    doctor-registry
    hospital-registry
    insurer-registry
    patient-registry
    health-records
    zk-eligibility
    zk-eligibility-verifier
    prescription-management
    emergency-medical-info
    medical-claims
    referral
    lab-management
    allergy-tracking
    allergy-management
    immunization-registry
    imaging-radiology
    clinical-guideline
    clinical-trial
    hospital-discharge-management
    care-plan
    pacs-integration
    healthcare-analytics
    healthcare-credentialing
    nutrition-care-management
    dental-records
    mental-health
    rehabilitation-services
    prenatal-pediatric
    hai-tracking
    medical-device-tracking
    telemedicine
    financial-records
    multisig-governance
    upgrade-governance
)

log() {
    printf '[deploy-all] %s\n' "$*" >&2
}

die() {
    printf '[deploy-all] error: %s\n' "$*" >&2
    exit 1
}

wasm_path_for() {
    local contract="$1"
    local wasm_name="${contract//-/_}.wasm"
    printf '%s/%s' "$TARGET_DIR" "$wasm_name"
}

json_escape() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

write_manifest() {
    local manifest="$1"
    shift

    {
        printf '{\n'
        printf '  "_network": "%s",\n' "$(json_escape "$NETWORK")"
        printf '  "_status": "%s"' "$(json_escape "$1")"
        shift
        while [[ $# -gt 0 ]]; do
            printf ',\n  "%s": "%s"' "$(json_escape "$1")" "$(json_escape "$2")"
            shift 2
        done
        printf '\n}\n'
    } > "$manifest"
}

contract_already_deployed() {
    local contract="$1"
    local manifest="$2"

    # Check if contract ID exists in manifest
    grep -q "\"${contract}\"" "$manifest" 2>/dev/null && \
    grep -q '^[A-Z][A-Z0-9]*' "$manifest" 2>/dev/null
}

optimize_wasm() {
    local wasm_path="$1"
    local optimized_path="${wasm_path%.wasm}.optimized.wasm"

    if [[ "$SKIP_OPTIMIZE" == true ]]; then
        return 0
    fi

    if [[ ! -f "$optimized_path" ]]; then
        log "optimizing WASM: $(basename "$wasm_path")"
        "$CLI_BIN" contract optimize --wasm "$wasm_path" --wasm-out "$optimized_path" || {
            log "warning: optimization failed, using unoptimized WASM"
            return 0
        }
    fi
}

deploy_contract() {
    local contract="$1"
    local wasm_path
    wasm_path="$(wasm_path_for "$contract")"

    if [[ "$DRY_RUN" == true ]]; then
        log "would deploy ${contract} from ${wasm_path}"
        printf 'DRY_RUN_%s\n' "${contract//-/_}"
        return
    fi

    [[ -f "$wasm_path" ]] || die "missing WASM for ${contract}: ${wasm_path}"

    # Optimize WASM if not skipped
    optimize_wasm "$wasm_path"

    # Use optimized version if it exists
    local deploy_wasm="$wasm_path"
    local optimized_path="${wasm_path%.wasm}.optimized.wasm"
    [[ -f "$optimized_path" ]] && deploy_wasm="$optimized_path"

    log "deploying ${contract}"
    "$CLI_BIN" contract deploy \
        --network "$NETWORK" \
        --source "$IDENTITY" \
        --wasm "$deploy_wasm"
}

cd "$REPO_ROOT"
mkdir -p "$MANIFEST_DIR"
MANIFEST="${MANIFEST_DIR}/${NETWORK}.json"

if [[ "$DRY_RUN" == false && "$SKIP_BUILD" == false ]]; then
    log "building workspace WASM artifacts"
    cargo build --target wasm32-unknown-unknown --release --workspace
fi

log "network=${NETWORK} identity=${IDENTITY} dry_run=${DRY_RUN} skip_build=${SKIP_BUILD} skip_optimize=${SKIP_OPTIMIZE}"
write_manifest "$MANIFEST" "IN_PROGRESS"

RESULTS=()
for contract in "${CONTRACTS[@]}"; do
    # Check if already deployed and skip if exists (idempotency)
    if [[ -f "$MANIFEST" ]] && contract_already_deployed "$contract" "$MANIFEST"; then
        contract_id="$(grep "\"${contract}\"" "$MANIFEST" | sed 's/.*": "\([^"]*\)".*/\1/')"
        log "contract ${contract} already deployed: ${contract_id}"
        RESULTS+=("$contract" "$contract_id")
    else
        contract_id="$(deploy_contract "$contract")"
        RESULTS+=("$contract" "$contract_id")
    fi
    write_manifest "$MANIFEST" "IN_PROGRESS" "${RESULTS[@]}"
done

write_manifest "$MANIFEST" "COMPLETE" "${RESULTS[@]}"
log "deployment complete: ${MANIFEST}"
