#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

NETWORK="testnet"
IDENTITY="${STELLAR_IDENTITY:-default}"
DRY_RUN=false
SKIP_BUILD=false
CLI_BIN="${CLI_BIN:-stellar}"
TARGET_DIR="${TARGET_DIR:-${REPO_ROOT}/target/wasm32v1-none/release}"
MANIFEST_DIR="${MANIFEST_DIR:-${REPO_ROOT}/deployments}"

usage() {
    cat <<EOF
Usage: $(basename "$0") [options]

Deploy all Healthy Stellar contracts in dependency order and write
deployments/<network>.json.

Options:
  --network <name>       Stellar network name (default: testnet)
  --identity <name>      Stellar CLI source identity (default: STELLAR_IDENTITY or default)
  --dry-run              Print the deployment plan without building or submitting txs
  --skip-build           Use existing WASM artifacts
  --cli-bin <binary>     Stellar CLI binary (default: stellar)
  -h, --help             Show this help text
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
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
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

    log "deploying ${contract}"
    "$CLI_BIN" contract deploy \
        --network "$NETWORK" \
        --source "$IDENTITY" \
        --wasm "$wasm_path"
}

cd "$REPO_ROOT"
mkdir -p "$MANIFEST_DIR"
MANIFEST="${MANIFEST_DIR}/${NETWORK}.json"

if [[ "$DRY_RUN" == false && "$SKIP_BUILD" == false ]]; then
    log "building workspace WASM artifacts"
    cargo build --target wasm32v1-none --release --workspace
fi

log "network=${NETWORK} identity=${IDENTITY} dry_run=${DRY_RUN}"
write_manifest "$MANIFEST" "INCOMPLETE"

RESULTS=()
for contract in "${CONTRACTS[@]}"; do
    contract_id="$(deploy_contract "$contract")"
    RESULTS+=("$contract" "$contract_id")
    write_manifest "$MANIFEST" "INCOMPLETE" "${RESULTS[@]}"
done

write_manifest "$MANIFEST" "complete" "${RESULTS[@]}"
log "manifest written: ${MANIFEST}"
