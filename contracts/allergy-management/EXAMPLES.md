# Usage Examples

This document provides practical examples of using the Allergy Management smart contract.

## Table of Contents

- [Setup](#setup)
- [Basic Operations](#basic-operations)
- [Advanced Scenarios](#advanced-scenarios)
- [Integration Examples](#integration-examples)
- [Error Handling](#error-handling)

## Setup

### Initialize Contract

```bash
# Deploy and initialize
CONTRACT_ID="<your-contract-id>"
ADMIN="<admin-address>"

soroban contract invoke \
  --id $CONTRACT_ID \
  --source admin \
  --network testnet \
  -- initialize \
  --admin $ADMIN
```

### Generate Test Accounts

```bash
# Create patient account
soroban keys generate patient --network testnet
PATIENT=$(soroban keys address patient)

# Create provider account
soroban keys generate provider --network testnet
PROVIDER=$(soroban keys address provider)

# Fund accounts (testnet only)
curl "https://friendbot.stellar.org?addr=$PATIENT"
curl "https://friendbot.stellar.org?addr=$PROVIDER"
```

## Basic Operations

### 1. Grant Access

Patient grants provider access to view allergies:

```bash
soroban contract invoke \
  --id $CONTRACT_ID \
  --source patient \
  --network testnet \
  -- grant_access \
  --patient_id $PATIENT \
  --provider_id $PROVIDER
```

### 2. Record a Medication Allergy

Provider records a penicillin allergy:

```bash
ALLERGY_ID=$(soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- record_allergy \
  --patient_id $PATIENT \
  --provider_id $PROVIDER \
  --allergen "Penicillin" \
  --allergen_type "med" \
  --reaction_type '["rash", "hives", "itching"]' \
  --severity "moderate" \
  --onset_date 1640000000 \
  --verified true)

echo "Allergy ID: $ALLERGY_ID"
```

### 3. Record a Food Allergy

```bash
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- record_allergy \
  --patient_id $PATIENT \
  --provider_id $PROVIDER \
  --allergen "Peanuts" \
  --allergen_type "food" \
  --reaction_type '["anaphylaxis", "throat swelling"]' \
  --severity "critical" \
  --onset_date 1620000000 \
  --verified true
```

### 4. Record an Environmental Allergy

```bash
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- record_allergy \
  --patient_id $PATIENT \
  --provider_id $PROVIDER \
  --allergen "Pollen" \
  --allergen_type "env" \
  --reaction_type '["sneezing", "watery eyes", "congestion"]' \
  --severity "mild" \
  --verified false
```

### 5. Get Active Allergies

```bash
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- get_active_allergies \
  --patient_id $PATIENT \
  --requester $PROVIDER
```

### 6. Update Allergy Severity

```bash
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- update_allergy_severity \
  --allergy_id 0 \
  --provider_id $PROVIDER \
  --new_severity "severe" \
  --reason "Patient experienced anaphylactic reaction during recent exposure"
```

### 7. Check Drug Interactions

Before prescribing amoxicillin (similar to penicillin):

```bash
soroban contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- check_drug_allergy_interaction \
  --patient_id $PATIENT \
  --drug_name "Amoxicillin"
```

### 8. Resolve an Allergy

After successful desensitization therapy:

```bash
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- resolve_allergy \
  --allergy_id 0 \
  --provider_id $PROVIDER \
  --resolution_date 1700000000 \
  --resolution_reason "Completed desensitization therapy successfully"
```

### 9. Revoke Access

Patient revokes provider access:

```bash
soroban contract invoke \
  --id $CONTRACT_ID \
  --source patient \
  --network testnet \
  -- revoke_access \
  --patient_id $PATIENT \
  --provider_id $PROVIDER
```

## Advanced Scenarios

### Scenario 1: Emergency Room Visit

Patient arrives at ER, provider needs to check allergies before treatment:

```bash
# 1. Patient grants emergency access
soroban contract invoke \
  --id $CONTRACT_ID \
  --source patient \
  --network testnet \
  -- grant_access \
  --patient_id $PATIENT \
  --provider_id $ER_DOCTOR

# 2. ER doctor checks for drug allergies
INTERACTIONS=$(soroban contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- check_drug_allergy_interaction \
  --patient_id $PATIENT \
  --drug_name "Morphine")

# 3. ER doctor reviews all active allergies
soroban contract invoke \
  --id $CONTRACT_ID \
  --source er_doctor \
  --network testnet \
  -- get_active_allergies \
  --patient_id $PATIENT \
  --requester $ER_DOCTOR

# 4. After treatment, revoke emergency access
soroban contract invoke \
  --id $CONTRACT_ID \
  --source patient \
  --network testnet \
  -- revoke_access \
  --patient_id $PATIENT \
  --provider_id $ER_DOCTOR
```

### Scenario 2: Allergy Testing and Updates

Patient undergoes allergy testing, results show worsening severity:

```bash
# 1. Record initial suspected allergy (unverified)
ALLERGY_ID=$(soroban contract invoke \
  --id $CONTRACT_ID \
  --source allergist \
  --network testnet \
  -- record_allergy \
  --patient_id $PATIENT \
  --provider_id $ALLERGIST \
  --allergen "Shellfish" \
  --allergen_type "food" \
  --reaction_type '["mild rash"]' \
  --severity "mild" \
  --verified false)

# 2. After testing, update severity
soroban contract invoke \
  --id $CONTRACT_ID \
  --source allergist \
  --network testnet \
  -- update_allergy_severity \
  --allergy_id $ALLERGY_ID \
  --provider_id $ALLERGIST \
  --new_severity "severe" \
  --reason "Allergy testing confirmed IgE levels indicating severe reaction risk"

# 3. Get complete allergy record with history
soroban contract invoke \
  --id $CONTRACT_ID \
  --source allergist \
  --network testnet \
  -- get_allergy \
  --allergy_id $ALLERGY_ID \
  --requester $ALLERGIST
```

### Scenario 3: Prescription Safety Check

Pharmacist checks for interactions before dispensing:

```bash
# 1. Grant pharmacist access
soroban contract invoke \
  --id $CONTRACT_ID \
  --source patient \
  --network testnet \
  -- grant_access \
  --patient_id $PATIENT \
  --provider_id $PHARMACIST

# 2. Check multiple medications
for DRUG in "Amoxicillin" "Ibuprofen" "Aspirin"; do
  echo "Checking $DRUG..."
  soroban contract invoke \
    --id $CONTRACT_ID \
    --network testnet \
    -- check_drug_allergy_interaction \
    --patient_id $PATIENT \
    --drug_name "$DRUG"
done

# 3. If safe, dispense medication
# 4. Revoke access after transaction
soroban contract invoke \
  --id $CONTRACT_ID \
  --source patient \
  --network testnet \
  -- revoke_access \
  --patient_id $PATIENT \
  --provider_id $PHARMACIST
```

### Scenario 4: Multiple Providers

Patient sees multiple specialists:

```bash
# Primary care physician
PCP=$(soroban keys address pcp)
# Allergist
ALLERGIST=$(soroban keys address allergist)
# Dermatologist
DERM=$(soroban keys address derm)

# Grant access to all providers
for PROVIDER in $PCP $ALLERGIST $DERM; do
  soroban contract invoke \
    --id $CONTRACT_ID \
    --source patient \
    --network testnet \
    -- grant_access \
    --patient_id $PATIENT \
    --provider_id $PROVIDER
done

# Each provider can now view and update allergies
# PCP records medication allergy
soroban contract invoke \
  --id $CONTRACT_ID \
  --source pcp \
  --network testnet \
  -- record_allergy \
  --patient_id $PATIENT \
  --provider_id $PCP \
  --allergen "Sulfa drugs" \
  --allergen_type "med" \
  --reaction_type '["rash"]' \
  --severity "moderate" \
  --verified true

# Dermatologist records contact allergy
soroban contract invoke \
  --id $CONTRACT_ID \
  --source derm \
  --network testnet \
  -- record_allergy \
  --patient_id $PATIENT \
  --provider_id $DERM \
  --allergen "Latex" \
  --allergen_type "env" \
  --reaction_type '["contact dermatitis"]' \
  --severity "mild" \
  --verified true
```

## Integration Examples

### JavaScript/TypeScript Integration

```typescript
import { Contract, SorobanRpc, TransactionBuilder, Networks } from '@stellar/stellar-sdk';

const CONTRACT_ID = 'your-contract-id';
const RPC_URL = 'https://soroban-testnet.stellar.org';

const server = new SorobanRpc.Server(RPC_URL);

async function recordAllergy(
  patientId: string,
  providerId: string,
  allergen: string,
  allergenType: string,
  reactions: string[],
  severity: string
) {
  const contract = new Contract(CONTRACT_ID);
  
  const tx = new TransactionBuilder(account, {
    fee: '100',
    networkPassphrase: Networks.TESTNET,
  })
    .addOperation(
      contract.call(
        'record_allergy',
        patientId,
        providerId,
        allergen,
        allergenType,
        reactions,
        severity,
        null, // onset_date
        true  // verified
      )
    )
    .setTimeout(30)
    .build();

  const prepared = await server.prepareTransaction(tx);
  prepared.sign(providerKeypair);
  
  const result = await server.sendTransaction(prepared);
  return result;
}

async function checkDrugInteractions(patientId: string, drugName: string) {
  const contract = new Contract(CONTRACT_ID);
  
  const result = await contract.call(
    'check_drug_allergy_interaction',
    patientId,
    drugName
  );
  
  return result;
}
```

### Python Integration

```python
from stellar_sdk import Soroban, Keypair, Network, TransactionBuilder
from stellar_sdk.soroban_rpc import SorobanServer

CONTRACT_ID = "your-contract-id"
RPC_URL = "https://soroban-testnet.stellar.org"

server = SorobanServer(RPC_URL)

def record_allergy(
    patient_keypair: Keypair,
    provider_keypair: Keypair,
    allergen: str,
    allergen_type: str,
    reactions: list,
    severity: str
):
    contract = Soroban(CONTRACT_ID)
    
    tx = (
        TransactionBuilder(
            source_account=provider_keypair.public_key,
            network_passphrase=Network.TESTNET_NETWORK_PASSPHRASE,
            base_fee=100,
        )
        .append_invoke_contract_function_op(
            contract_id=CONTRACT_ID,
            function_name="record_allergy",
            parameters=[
                patient_keypair.public_key,
                provider_keypair.public_key,
                allergen,
                allergen_type,
                reactions,
                severity,
                None,  # onset_date
                True,  # verified
            ],
        )
        .set_timeout(30)
        .build()
    )
    
    prepared = server.prepare_transaction(tx)
    prepared.sign(provider_keypair)
    
    response = server.send_transaction(prepared)
    return response

def get_active_allergies(patient_id: str, requester_keypair: Keypair):
    # Implementation here
    pass
```

### REST API Wrapper Example

```javascript
// Express.js API wrapper
const express = require('express');
const app = express();

app.post('/api/allergies', async (req, res) => {
  try {
    const { patientId, allergen, allergenType, reactions, severity } = req.body;
    
    // Validate input
    if (!patientId || !allergen || !allergenType) {
      return res.status(400).json({ error: 'Missing required fields' });
    }
    
    // Call smart contract
    const allergyId = await recordAllergy(
      patientId,
      req.user.providerId,
      allergen,
      allergenType,
      reactions,
      severity
    );
    
    res.json({ success: true, allergyId });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

app.get('/api/allergies/:patientId/check/:drugName', async (req, res) => {
  try {
    const { patientId, drugName } = req.params;
    
    const interactions = await checkDrugInteractions(patientId, drugName);
    
    res.json({ interactions });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});
```

## Error Handling

### Common Errors and Solutions

#### 1. Access Denied

```bash
# Error: AccessDenied
# Solution: Grant access first
soroban contract invoke \
  --id $CONTRACT_ID \
  --source patient \
  --network testnet \
  -- grant_access \
  --patient_id $PATIENT \
  --provider_id $PROVIDER
```

#### 2. Duplicate Allergy

```bash
# Error: DuplicateAllergy
# Solution: Check existing allergies first
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- get_active_allergies \
  --patient_id $PATIENT \
  --requester $PROVIDER
```

#### 3. Invalid Severity

```bash
# Error: InvalidSeverity
# Valid values: "mild", "moderate", "severe", "critical"
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- record_allergy \
  --severity "moderate"  # Use valid severity
```

#### 4. Already Resolved

```bash
# Error: AlreadyResolved
# Cannot update or resolve an already resolved allergy
# Check allergy status first
soroban contract invoke \
  --id $CONTRACT_ID \
  --source provider \
  --network testnet \
  -- get_allergy \
  --allergy_id 0 \
  --requester $PROVIDER
```

## Best Practices

### 1. Access Management

```bash
# Grant temporary access for specific procedures
soroban contract invoke --id $CONTRACT_ID --source patient -- grant_access ...

# Revoke access after procedure
soroban contract invoke --id $CONTRACT_ID --source patient -- revoke_access ...
```

### 2. Verification

```bash
# Always verify critical allergies
--verified true

# Mark unverified for suspected allergies
--verified false
```

### 3. Detailed Reactions

```bash
# Provide specific reaction details
--reaction_type '["anaphylaxis", "throat swelling", "difficulty breathing"]'

# Not just generic terms
--reaction_type '["allergic reaction"]'  # Less helpful
```

### 4. Regular Updates

```bash
# Update severity if patient's condition changes
soroban contract invoke --id $CONTRACT_ID -- update_allergy_severity ...

# Document reason for changes
--reason "Detailed explanation of why severity changed"
```

### 5. Drug Checking

```bash
# Always check before prescribing
soroban contract invoke --id $CONTRACT_ID -- check_drug_allergy_interaction ...

# Check related drugs (e.g., penicillin family)
for DRUG in "Penicillin" "Amoxicillin" "Ampicillin"; do
  check_interaction $DRUG
done
```

---

For more examples and documentation:
- [README.md](README.md) - Full documentation
- [API Reference](README.md#api-reference) - Complete API details
- [Deployment Guide](DEPLOYMENT.md) - Deployment instructions
