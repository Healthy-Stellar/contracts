# Contract Interaction Examples

This document contains copy-pasteable Stellar CLI one-liners for core contract operations on both Testnet and Mainnet.

## Setup

Set the network and contract IDs before running commands:

```bash
NETWORK=testnet
IDENTITY=default
```

For Mainnet:

```bash
NETWORK=mainnet
IDENTITY=mainnet-identity
```

Update these examples with the actual contract IDs from `deployments/$NETWORK.json`.

## Register patient

```bash
stellar contract invoke --network "$NETWORK" --source "$IDENTITY" \
  --id "$PATIENT_REGISTRY_ID" \
  -- register_patient \
  --patient_id "patient-123" \
  --metadata "{\"name\":\"Alice\",\"dob\":\"1980-01-01\"}"
```

## Add record

```bash
stellar contract invoke --network "$NETWORK" --source "$IDENTITY" \
  --id "$HEALTH_RECORDS_ID" \
  -- add_record \
  --patient_id "patient-123" \
  --record_type "lab-result" \
  --payload "{\"test\":\"CBC\",\"value\":\"normal\"}"
```

## Grant consent

```bash
stellar contract invoke --network "$NETWORK" --source "$IDENTITY" \
  --id "$ACCESS_CONTROL_ID" \
  -- grant_consent \
  --patient_id "patient-123" \
  --provider_id "provider-456" \
  --scope "read:health-records"
```

## Submit claim

```bash
stellar contract invoke --network "$NETWORK" --source "$IDENTITY" \
  --id "$MEDICAL_CLAIMS_ID" \
  -- submit_claim \
  --patient_id "patient-123" \
  --provider_id "provider-456" \
  --amount "150.00" \
  --description "Office visit"
```

## Deploy governance contract interaction

```bash
stellar contract invoke --network "$NETWORK" --source "$IDENTITY" \
  --id "$MULTISIG_GOVERNANCE_ID" \
  -- create_proposal \
  --title "Authorize new service" \
  --details "Approve access for new provider"
```

## Read-only contract example

```bash
stellar contract invoke --network "$NETWORK" --source "$IDENTITY" \
  --id "$PATIENT_REGISTRY_ID" \
  -- get_patient \
  --patient_id "patient-123"
```

## Mainnet and Testnet usage notes

- Replace `$NETWORK` with `testnet` or `mainnet`.
- Replace contract ID placeholders with values from `deployments/$NETWORK.json`.
- Use a secure identity on Mainnet.
- If a command is sensitive, verify the contract arguments before submission.
