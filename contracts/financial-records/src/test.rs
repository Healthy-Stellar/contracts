#![cfg(test)]
use super::*;
use shared::privacy::{EncryptedEnvelopeRef, PolicyMetadata};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{BytesN, Env, String, Symbol};

fn encrypted_ref(env: &Env, seed: u8) -> EncryptedEnvelopeRef {
    EncryptedEnvelopeRef {
        content_hash: BytesN::from_array(env, &[seed; 32]),
        envelope_uri: String::from_str(env, "enc+ipfs://bafyvalidfinancialref"),
        key_version_id: String::from_str(env, "kv:v01"),
    }
}

fn policy(env: &Env) -> PolicyMetadata {
    PolicyMetadata {
        retention_class: Symbol::new(env, "financial"),
        access_policy_hash: BytesN::from_array(env, &[9u8; 32]),
        purpose: Symbol::new(env, "billing"),
    }
}

#[test]
fn test_add_and_get_records() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let reference = encrypted_ref(&e, 1);
    let policy = policy(&e);

    client.add_financial_record(&owner, &RecordType::TaxDocument, &reference, &policy);

    let records = client.get_financial_records(&owner, &owner, &0, &10);
    assert_eq!(records.len(), 1);
    let record = records.get(0).unwrap();
    assert_eq!(record.owner, owner);
    assert_eq!(record.record_type, RecordType::TaxDocument);
    assert_eq!(record.encrypted_ref, reference);
    assert_eq!(record.policy, policy);
}

#[test]
fn test_access_granted() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let auditor = Address::generate(&e);

    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    client.grant_access(&owner, &auditor);

    let records = client.get_financial_records(&auditor, &owner, &0, &10);
    assert_eq!(records.len(), 1);
}

#[test]
fn test_unauthorized_access_returns_typed_error() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let stranger = Address::generate(&e);

    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    let result = client.try_get_financial_records(&stranger, &owner, &0, &10);
    assert_eq!(result, Err(Ok(ContractError::AccessDenied)));
}

#[test]
fn test_revoked_access_returns_typed_error() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let auditor = Address::generate(&e);

    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    client.grant_access(&owner, &auditor);
    let records = client.get_financial_records(&auditor, &owner, &0, &10);
    assert_eq!(records.len(), 1);

    client.revoke_access(&owner, &auditor);
    let result = client.try_get_financial_records(&auditor, &owner, &0, &10);
    assert_eq!(result, Err(Ok(ContractError::AccessDenied)));
}

#[test]
fn test_type_index_filtering() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);

    e.ledger().set_timestamp(100);
    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    e.ledger().set_timestamp(200);
    client.add_financial_record(
        &owner,
        &RecordType::TaxDocument,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    e.ledger().set_timestamp(300);
    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    let invoices = client.get_records_by_type(&owner, &owner, &RecordType::Invoice, &0, &10);
    assert_eq!(invoices.len(), 2);

    let tax_docs = client.get_records_by_type(&owner, &owner, &RecordType::TaxDocument, &0, &10);
    assert_eq!(tax_docs.len(), 1);
}

#[test]
fn test_date_index_filtering() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);

    e.ledger().set_timestamp(100);
    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    e.ledger().set_timestamp(200);
    client.add_financial_record(
        &owner,
        &RecordType::TaxDocument,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    e.ledger().set_timestamp(300);
    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    let range = client.get_records_by_date_range(&owner, &owner, &150, &250, &0, &10);
    assert_eq!(range.len(), 1);
    assert_eq!(range.get(0).unwrap().timestamp, 200);
}

#[test]
fn test_pagination_get_financial_records() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);

    for i in 0u32..5 {
        client.add_financial_record(
            &owner,
            &RecordType::Receipt,
            &encrypted_ref(&e, 2),
            &policy(&e),
        );
        let _ = i;
    }

    let page1 = client.get_financial_records(&owner, &owner, &0, &3);
    assert_eq!(page1.len(), 3);

    let page2 = client.get_financial_records(&owner, &owner, &3, &3);
    assert_eq!(page2.len(), 2);
}

#[test]
fn test_pagination_get_records_by_type() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);

    for _ in 0..4 {
        client.add_financial_record(
            &owner,
            &RecordType::Receipt,
            &encrypted_ref(&e, 2),
            &policy(&e),
        );
    }

    let page1 = client.get_records_by_type(&owner, &owner, &RecordType::Receipt, &0, &2);
    assert_eq!(page1.len(), 2);

    let page2 = client.get_records_by_type(&owner, &owner, &RecordType::Receipt, &2, &2);
    assert_eq!(page2.len(), 2);
}

#[test]
fn test_type_index_unauthorized_returns_typed_error() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let stranger = Address::generate(&e);

    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    let result = client.try_get_records_by_type(&stranger, &owner, &RecordType::Invoice, &0, &10);
    assert_eq!(result, Err(Ok(ContractError::AccessDenied)));
}

#[test]
fn test_date_index_unauthorized_returns_typed_error() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let stranger = Address::generate(&e);

    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 2),
        &policy(&e),
    );

    let result = client.try_get_records_by_date_range(&stranger, &owner, &0, &999, &0, &10);
    assert_eq!(result, Err(Ok(ContractError::AccessDenied)));
}

// ---------------------------------------------------------------------------
// Audit trail: event emission and post-revocation denial
// ---------------------------------------------------------------------------

#[test]
fn test_grant_access_emits_event() {
    use soroban_sdk::testutils::Events;
    use soroban_sdk::{IntoVal, Val, Vec as SdkVec};

    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let auditor = Address::generate(&e);

    client.grant_access(&owner, &auditor);

    let events = e.events().all();
    let expected_topics: SdkVec<Val> =
        (Symbol::new(&e, "grant"), owner.clone(), auditor.clone()).into_val(&e);
    assert!(
        events
            .iter()
            .any(|(_id, topics, _data)| topics == expected_topics),
        "grant event not emitted"
    );
}

#[test]
fn test_revoke_access_emits_event() {
    use soroban_sdk::testutils::Events;
    use soroban_sdk::{IntoVal, Val, Vec as SdkVec};

    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let auditor = Address::generate(&e);

    client.grant_access(&owner, &auditor);
    client.revoke_access(&owner, &auditor);

    let events = e.events().all();
    let expected_topics: SdkVec<Val> =
        (Symbol::new(&e, "revoke"), owner.clone(), auditor.clone()).into_val(&e);
    assert!(
        events
            .iter()
            .any(|(_id, topics, _data)| topics == expected_topics),
        "revoke event not emitted"
    );
}

#[test]
fn test_read_denied_immediately_after_revocation() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register(FinancialRecordContract, ());
    let client = FinancialRecordContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let auditor = Address::generate(&e);

    client.add_financial_record(
        &owner,
        &RecordType::Invoice,
        &encrypted_ref(&e, 3),
        &policy(&e),
    );

    client.grant_access(&owner, &auditor);
    // Confirm access works before revocation.
    assert_eq!(
        client.get_financial_records(&auditor, &owner, &0, &10).len(),
        1
    );

    client.revoke_access(&owner, &auditor);
    // Access must be denied in the same block — no delay.
    let result = client.try_get_financial_records(&auditor, &owner, &0, &10);
    assert_eq!(result, Err(Ok(ContractError::AccessDenied)));
}
