#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

MANIFEST_PATH="${MANIFEST_PATH:-${REPO_ROOT}/Cargo.toml}"
PACKAGE_NAME="${PACKAGE_NAME:-patient-registry}"
WASM_PATH="${WASM_PATH:-${REPO_ROOT}/target/wasm32v1-none/release/patient_registry.wasm}"

NETWORK_NAME="${NETWORK_NAME:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
FRIENDBOT_URL="${FRIENDBOT_URL:-https://friendbot.stellar.org}"
HORIZON_URL="${HORIZON_URL:-https://horizon-testnet.stellar.org}"

CLI_BIN="${CLI_BIN:-}"
ADMIN_IDENTITY="${ADMIN_IDENTITY:-issue95-admin}"
PATIENT_IDENTITY="${PATIENT_IDENTITY:-issue95-patient}"
DOCTOR_IDENTITY="${DOCTOR_IDENTITY:-issue95-doctor}"

PATIENT_NAME="${PATIENT_NAME:-test-patient-95}"
PATIENT_DOB="${PATIENT_DOB:-631152000}"
PATIENT_METADATA="${PATIENT_METADATA:-ipfs://healthy-stellar/test-patient-95}"
RECORD_HASH_HEX="${RECORD_HASH_HEX:-deadbeef95010203}"
RECORD_DESCRIPTION="${RECORD_DESCRIPTION:-smoke-test-record-95}"

TOTAL_STEPS=8
CURRENT_STEP=0

KEYS_ADDRESS_SUBCOMMAND=()
RPC_ARGS=()

usage() {
    cat <<EOF
Usage: $(basename "$0") [options]

Deploys the patient-registry contract to Stellar Testnet and runs a smoke test.

Options:
  --admin-identity <alias>       Identity alias used for deploy + initialize
  --patient-identity <alias>     Identity alias used for patient registration
  --doctor-identity <alias>      Identity alias used for doctor record creation
  --network-name <name>          Network label for log output (default: ${NETWORK_NAME})
  --rpc-url <url>                Soroban RPC URL
  --network-passphrase <value>   Stellar network passphrase
  --friendbot-url <url>          Friendbot base URL
  --horizon-url <url>            Horizon base URL used to detect existing accounts
  --manifest-path <path>         Cargo manifest to build from
  --package <name>               Cargo package to build (default: ${PACKAGE_NAME})
  --wasm-path <path>             Built WASM path to deploy
  --patient-name <value>         Dummy patient name for the smoke test
  --patient-dob <unix-seconds>   Dummy patient DOB for the smoke test
  --patient-metadata <value>     Dummy patient metadata string
  --record-hash-hex <hex>        Hex-encoded Bytes argument for add_medical_record
  --record-description <value>   Dummy record description
  --cli-bin <binary>             Explicit CLI binary to use (soroban or stellar)
  -h, --help                     Show this help text

Environment variables with the same names are also supported.
EOF
}

log() {
    printf '%s\n' "$*"
}

warn() {
    printf 'Warning: %s\n' "$*" >&2
}

die() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

step() {
    CURRENT_STEP=$((CURRENT_STEP + 1))
    log
    log "[${CURRENT_STEP}/${TOTAL_STEPS}] $*"
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --admin-identity)
                ADMIN_IDENTITY="$2"
                shift 2
                ;;
            --patient-identity)
                PATIENT_IDENTITY="$2"
                shift 2
                ;;
            --doctor-identity)
                DOCTOR_IDENTITY="$2"
                shift 2
                ;;
            --network-name)
                NETWORK_NAME="$2"
                shift 2
                ;;
            --rpc-url)
                RPC_URL="$2"
                shift 2
                ;;
            --network-passphrase)
                NETWORK_PASSPHRASE="$2"
                shift 2
                ;;
            --friendbot-url)
                FRIENDBOT_URL="$2"
                shift 2
                ;;
            --horizon-url)
                HORIZON_URL="$2"
                shift 2
                ;;
            --manifest-path)
                MANIFEST_PATH="$2"
                shift 2
                ;;
            --package)
                PACKAGE_NAME="$2"
                shift 2
                ;;
            --wasm-path)
                WASM_PATH="$2"
                shift 2
                ;;
            --patient-name)
                PATIENT_NAME="$2"
                shift 2
                ;;
            --patient-dob)
                PATIENT_DOB="$2"
                shift 2
                ;;
            --patient-metadata)
                PATIENT_METADATA="$2"
                shift 2
                ;;
            --record-hash-hex)
                RECORD_HASH_HEX="$2"
                shift 2
                ;;
            --record-description)
                RECORD_DESCRIPTION="$2"
                shift 2
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
                die "Unknown argument: $1"
                ;;
        esac
    done
}

detect_cli() {
    if [[ -n "${CLI_BIN}" ]]; then
        require_cmd "${CLI_BIN}"
    elif command -v soroban >/dev/null 2>&1; then
        CLI_BIN="soroban"
    elif command -v stellar >/dev/null 2>&1; then
        CLI_BIN="stellar"
    else
        die "Neither soroban nor stellar CLI is installed."
    fi

    if "${CLI_BIN}" keys public-key --help >/dev/null 2>&1; then
        KEYS_ADDRESS_SUBCOMMAND=(keys public-key)
    else
        KEYS_ADDRESS_SUBCOMMAND=(keys address)
    fi
}

validate_inputs() {
    [[ -f "${MANIFEST_PATH}" ]] || die "Cargo manifest not found: ${MANIFEST_PATH}"
    [[ "${PATIENT_DOB}" =~ ^[0-9]+$ ]] || die "PATIENT_DOB must be an unsigned integer."
    [[ "${RECORD_HASH_HEX}" =~ ^[0-9a-fA-F]+$ ]] || die "RECORD_HASH_HEX must contain only hexadecimal characters."
    (( ${#RECORD_HASH_HEX} % 2 == 0 )) || die "RECORD_HASH_HEX must contain an even number of hexadecimal characters."
}

identity_address() {
    "${CLI_BIN}" "${KEYS_ADDRESS_SUBCOMMAND[@]}" "$1"
}

identity_exists() {
    identity_address "$1" >/dev/null 2>&1
}

create_identity() {
    local alias="$1"

    if identity_exists "${alias}"; then
        log "Reusing existing identity: ${alias}"
        return
    fi

    if "${CLI_BIN}" keys generate "${alias}" >/dev/null 2>&1; then
        log "Created identity: ${alias}"
        return
    fi

    if "${CLI_BIN}" keys generate "${alias}" --network "${NETWORK_NAME}" >/dev/null 2>&1; then
        log "Created identity: ${alias}"
        return
    fi

    if "${CLI_BIN}" keys generate --network "${NETWORK_NAME}" "${alias}" >/dev/null 2>&1; then
        log "Created identity: ${alias}"
        return
    fi

    die "Failed to create identity: ${alias}"
}

account_exists() {
    local address="$1"
    curl -fsS -o /dev/null "${HORIZON_URL%/}/accounts/${address}"
}

fund_identity() {
    local alias="$1"
    local address http_code response_file response_body

    address="$(identity_address "${alias}")"
    response_file="$(mktemp)"

    http_code="$(
        curl -sS -o "${response_file}" -w '%{http_code}' \
            -G --data-urlencode "addr=${address}" "${FRIENDBOT_URL%/}"
    )" || {
        response_body="$(cat "${response_file}" 2>/dev/null || true)"
        rm -f "${response_file}"

        if account_exists "${address}"; then
            warn "Friendbot did not fund ${alias}, but the account already exists on ${NETWORK_NAME}; continuing."
            return
        fi

        die "Friendbot request failed for ${alias} (${address}). Response: ${response_body:-curl exited with a non-zero status}."
    }

    response_body="$(cat "${response_file}" 2>/dev/null || true)"
    rm -f "${response_file}"

    if [[ "${http_code}" == 2* ]]; then
        log "Friendbot funded ${alias}: ${address}"
        return
    fi

    if account_exists "${address}"; then
        warn "Friendbot returned HTTP ${http_code} for ${alias}, but the account already exists on ${NETWORK_NAME}; continuing."
        return
    fi

    die "Friendbot returned HTTP ${http_code} for ${alias} (${address}). Response: ${response_body}."
}

extract_contract_id() {
    printf '%s\n' "$1" | grep -Eo 'C[A-Z2-7]{55}' | tail -n 1
}

invoke_contract() {
    local source="$1"
    shift

    "${CLI_BIN}" contract invoke \
        "${RPC_ARGS[@]}" \
        --id "${CONTRACT_ID}" \
        --source "${source}" \
        -- "$@"
}

parse_args "$@"
detect_cli
validate_inputs
cd "${REPO_ROOT}"

RPC_ARGS=(--rpc-url "${RPC_URL}" --network-passphrase "${NETWORK_PASSPHRASE}")

step "Checking dependencies..."
require_cmd cargo
require_cmd curl
require_cmd rustup
require_cmd "${CLI_BIN}"
log "Using CLI binary: ${CLI_BIN}"
log "Target network: ${NETWORK_NAME}"

step "Preparing identities..."
create_identity "${ADMIN_IDENTITY}"
create_identity "${PATIENT_IDENTITY}"
create_identity "${DOCTOR_IDENTITY}"

ADMIN_ADDRESS="$(identity_address "${ADMIN_IDENTITY}")"
PATIENT_ADDRESS="$(identity_address "${PATIENT_IDENTITY}")"
DOCTOR_ADDRESS="$(identity_address "${DOCTOR_IDENTITY}")"

log "Admin address:   ${ADMIN_ADDRESS}"
log "Patient address: ${PATIENT_ADDRESS}"
log "Doctor address:  ${DOCTOR_ADDRESS}"

step "Funding test identities via Friendbot..."
fund_identity "${ADMIN_IDENTITY}"
fund_identity "${PATIENT_IDENTITY}"
fund_identity "${DOCTOR_IDENTITY}"

step "Building ${PACKAGE_NAME} WASM..."
"${CLI_BIN}" contract build --manifest-path "${MANIFEST_PATH}" --package "${PACKAGE_NAME}"
[[ -f "${WASM_PATH}" ]] || die "Expected WASM was not produced at ${WASM_PATH}"
log "Built WASM: ${WASM_PATH}"

step "Deploying contract to ${NETWORK_NAME}..."
DEPLOY_OUTPUT="$(
    "${CLI_BIN}" contract deploy \
        "${RPC_ARGS[@]}" \
        --source "${ADMIN_IDENTITY}" \
        --wasm "${WASM_PATH}" 2>&1
)"
CONTRACT_ID="$(extract_contract_id "${DEPLOY_OUTPUT}" || true)"
[[ -n "${CONTRACT_ID}" ]] || die "Contract deployment did not return a contract ID. Output: ${DEPLOY_OUTPUT}"
log "Deployed contract ID: ${CONTRACT_ID}"

step "Initializing contract..."
invoke_contract "${ADMIN_IDENTITY}" initialize --admin "${ADMIN_ADDRESS}" >/dev/null
log "Initialized contract with admin ${ADMIN_ADDRESS}"

step "Registering test patient..."
invoke_contract \
    "${PATIENT_IDENTITY}" \
    register_patient \
    --wallet "${PATIENT_ADDRESS}" \
    --name "${PATIENT_NAME}" \
    --dob "${PATIENT_DOB}" \
    --metadata "${PATIENT_METADATA}" >/dev/null
log "Registered patient ${PATIENT_ADDRESS}"

step "Granting doctor access for record creation..."
invoke_contract \
    "${PATIENT_IDENTITY}" \
    grant_access \
    --patient "${PATIENT_ADDRESS}" \
    --doctor "${DOCTOR_ADDRESS}" >/dev/null
log "Granted ${DOCTOR_ADDRESS} access to ${PATIENT_ADDRESS}"

step "Creating and verifying a test medical record..."
invoke_contract \
    "${DOCTOR_IDENTITY}" \
    add_medical_record \
    --patient "${PATIENT_ADDRESS}" \
    --doctor "${DOCTOR_ADDRESS}" \
    --record_hash "${RECORD_HASH_HEX}" \
    --description "${RECORD_DESCRIPTION}" >/dev/null

RECORDS_OUTPUT="$(
    invoke_contract \
        "${ADMIN_IDENTITY}" \
        get_medical_records \
        --patient "${PATIENT_ADDRESS}"
)"

[[ "${RECORDS_OUTPUT}" == *"${RECORD_DESCRIPTION}"* ]] || die "Smoke test failed: the created medical record was not returned by get_medical_records."
log "Smoke test passed."

log
log "Contract ID: ${CONTRACT_ID}"
