# Deployment Guide

This repository contains Healthy Stellar contracts and deployment tooling for both Testnet and Mainnet.

## Supported networks

- `testnet` — development and pre-production validation.
- `mainnet` — production deployment.

## Deployment workflow

### Prerequisites

- Install the Stellar CLI and make sure it is on your `PATH`.
- Confirm `cargo test` passes for all contracts in the workspace:

  ```bash
  cargo test --workspace
  ```

- Build all contracts for the Stellar WASM target:

  ```bash
  cargo build --target wasm32v1-none --release --workspace
  ```

- Verify the `stellar` CLI can connect to the target network and that your deployment identity is configured.

### Identity and key management

For Mainnet, never use a plain secret key in CI or automation.

- Prefer a hardware wallet or multi-sig Stellar account for admin operations.
- Use `STELLAR_IDENTITY` or the Stellar CLI local identity configuration to reference the signer.
- Protect any private signing keys and avoid embedding them in repository or pipeline secrets.

### Deployment order

The repository deploys contracts in dependency order, but Mainnet must deploy governance contracts first:

1. `multisig-governance`
2. `upgrade-governance`
3. All other contracts

The governance contracts must be deployed before any other contract is used in production.

### Deployment script

The repository includes `scripts/deploy_all.sh` for network-aware deployment and manifest generation.

- To deploy to Testnet:

  ```bash
  ./scripts/deploy_all.sh --network testnet
  ```

- To deploy to Mainnet:

  ```bash
  ./scripts/deploy_all.sh --network mainnet
  ```

- To dry-run and validate the plan without submitting transactions:

  ```bash
  ./scripts/deploy_all.sh --network mainnet --dry-run
  ```

- To skip rebuilding if WASM artifacts already exist:

  ```bash
  ./scripts/deploy_all.sh --network mainnet --skip-build
  ```

### Manifest and contract IDs

- The script writes deployed contract IDs to `deployments/<network>.json`.
- For Mainnet, record every deployed contract ID in `deployments/mainnet.json` before any production interaction.
- Do not proceed with administrative or application transactions until the manifest is complete and verified.

### Mainnet deployment checklist

1. `cargo test --workspace` passes.
2. `cargo build --target wasm32v1-none --release --workspace` completes with no warnings or errors.
3. Admin identity is a hardware wallet or multi-sig account.
4. Contracts `upgrade-governance` and `multisig-governance` are deployed first.
5. `deployments/mainnet.json` contains all contract IDs before any interaction.
6. After each contract deploy, verify the on-chain WASM hash matches the local build hash using Horizon or Stellar Expert.
7. Run a post-deployment smoke test against Mainnet before announcing availability.

### Post-deployment verification

- Confirm deployed contract IDs are accurate in `deployments/mainnet.json`.
- Use Stellar Explorer / Horizon to compare the contract WASM hash with the local artifact.
- Run a simple transaction against each deployed contract to ensure the contract is callable.

### Smoke test suggestions

- Invoke a no-op or read-only query on each deployed contract.
- Confirm governance contracts can be invoked by the configured admin identity.
- Check that shared and registry contracts return expected state values.

## Upgrade guide

This section describes the upgrade lifecycle for an already-deployed contract using `upgrade-governance`.

### 1. Build the new WASM

Build the updated contract locally and calculate the WASM hash.

```bash
cargo build --target wasm32v1-none --release -p <contract-package>
NEW_HASH=$(sha256sum target/wasm32v1-none/release/<contract_name>.wasm | awk '{print $1}')
```

### 2. Propose an upgrade

Submit an upgrade proposal to the deployed `upgrade-governance` contract.

```bash
stellar contract invoke --id "$UPGRADE_GOVERNANCE_ID" -- propose_upgrade \
  --new_wasm_hash "$NEW_HASH" \
  --target_contract "$TARGET_ID"
```

### 3. Vote on the proposal

Each governance signer must vote for the proposal.

```bash
stellar contract invoke --id "$UPGRADE_GOVERNANCE_ID" -- vote --proposal_id "$PID"
```

### 4. Execute the upgrade after quorum

After the required votes are collected, execute the upgrade:

```bash
stellar contract invoke --id "$UPGRADE_GOVERNANCE_ID" -- execute_upgrade --proposal_id "$PID"
```

### 5. Verify the upgrade

- Confirm the target contract is now updated by checking its WASM hash on-chain.
- Run a smoke test against the upgraded contract.
- Update deployment records if needed.

### End-to-end upgrade lifecycle example

1. Build the new contract and compute `NEW_HASH`.
2. Propose the upgrade with `propose_upgrade`.
3. Collect votes from each authorized governance signer using `vote`.
4. Execute the upgrade once quorum is reached.
5. Verify the on-chain contract hash matches the new build and run a post-upgrade test.

## Network-specific notes

### Testnet

- Use Testnet for development, validation, and smoke-testing changes before Mainnet.
- Testnet deployments can be performed with the same script and manifest pattern.

### Mainnet

- Use a production-grade signer (hardware wallet or multi-sig).
- Verify every contract deployment with on-chain tooling.
- Maintain `deployments/mainnet.json` as the authoritative source of deployed contract IDs.
- Run Mainnet smoke tests before marking the deployment as available.
