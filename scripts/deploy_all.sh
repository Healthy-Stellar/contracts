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
GIT_SHA="$(cd "$REPO_ROOT" && git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
DEPLOYMENT_TIMESTAMP="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"

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

compute_wasm_hash() {
    local wasm_path="$1"
    if [[ -f "$wasm_path" ]]; then
        sha256sum "$wasm_path" | awk '{print $1}'
    else
        printf ''
    fi
}

write_manifest() {
    local manifest="$1"
    local status="$2"
    shift 2
    local -a contracts=("$@")

    {
        printf '{\n'
        printf '  "network": "%s",\n' "$(json_escape "$NETWORK")"
        printf '  "deployed_at": "%s",\n' "$(json_escape "$DEPLOYMENT_TIMESTAMP")"
        printf '  "git_sha": "%s",\n' "$(json_escape "$GIT_SHA")"
        printf '  "status": "%s",\n' "$(json_escape "$status")"
        printf '  "contracts": {\n'

        local first=true
        local i=0
        while [[ $i -lt ${#contracts[@]} ]]; do
            local contract="${contracts[$i]}"
            local contract_id="${contracts[$((i+1))]}"
            local wasm_path
            wasm_path="$(wasm_path_for "$contract")"
            local wasm_hash
            wasm_hash="$(compute_wasm_hash "$wasm_path")"

            if [[ "$first" == true ]]; then
                first=false
            else
                printf ',\n'
            fi

            printf '    "%s": {\n' "$(json_escape "$contract")"
            printf '      "contract_id": "%s",\n' "$(json_escape "$contract_id")"
            printf '      "wasm_hash": "%s",\n' "$(json_escape "$wasm_hash")"
            printf '      "git_sha": "%s"\n' "$(json_escape "$GIT_SHA")"
            printf '    }'

            i=$((i + 2))
        done

        printf '\n  }\n'
        printf '}\n'
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
MANIFEST_SIG="${MANIFEST_DIR}/${NETWORK}.json.sig"

if [[ "$DRY_RUN" == false && "$SKIP_BUILD" == false ]]; then
    log "building workspace WASM artifacts"
    cargo build --target wasm32-unknown-unknown --release --workspace
fi

log "network=${NETWORK} identity=${IDENTITY} dry_run=${DRY_RUN}"
write_manifest "$MANIFEST" "in_progress"

RESULTS=()
for contract in "${CONTRACTS[@]}"; do
    contract_id="$(deploy_contract "$contract")"
    RESULTS+=("$contract" "$contract_id")
    write_manifest "$MANIFEST" "in_progress" "${RESULTS[@]}"
done

write_manifest "$MANIFEST" "COMPLETE" "${RESULTS[@]}"
log "deployment complete: ${MANIFEST}"
