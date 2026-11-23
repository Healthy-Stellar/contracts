use soroban_sdk::{Address, Env, Vec, Symbol};

use crate::types::{
    Appeal, AuthorizationRequest, DataKey, ExtensionRequest, PeerToPeerRequest,
    ReviewRecord, Reviewer, SLAConfig, SupportingDocument, UsageRecord,
};

// -----------------------------------------------------------------------
// Counters
// -----------------------------------------------------------------------

pub fn next_auth_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::AuthCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::AuthCounter, &next);
    next
}

pub fn next_appeal_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::AppealCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::AppealCounter, &next);
    next
}

pub fn next_review_id(env: &Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::ReviewCounter)
        .unwrap_or(0);
    let next = id + 1;
    env.storage()
        .persistent()
        .set(&DataKey::ReviewCounter, &next);
    next
}

// -----------------------------------------------------------------------
// AuthorizationRequest
// -----------------------------------------------------------------------

pub fn save_auth_request(env: &Env, req: &AuthorizationRequest) {
    env.storage()
        .persistent()
        .set(&DataKey::AuthRequest(req.auth_request_id), req);
}

pub fn load_auth_request(env: &Env, id: u64) -> Option<AuthorizationRequest> {
    env.storage()
        .persistent()
        .get(&DataKey::AuthRequest(id))
}

pub fn add_provider_auth(env: &Env, provider_id: &Address, auth_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::ProviderAuths(provider_id.clone()))
        .unwrap_or(Vec::new(env));
    ids.push_back(auth_id);
    env.storage()
        .persistent()
        .set(&DataKey::ProviderAuths(provider_id.clone()), &ids);
}

pub fn add_patient_auth(env: &Env, patient_id: &Address, auth_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::PatientAuths(patient_id.clone()))
        .unwrap_or(Vec::new(env));
    ids.push_back(auth_id);
    env.storage()
        .persistent()
        .set(&DataKey::PatientAuths(patient_id.clone()), &ids);
}

// -----------------------------------------------------------------------
// Supporting documents
// -----------------------------------------------------------------------

pub fn save_document(env: &Env, auth_request_id: u64, doc: &SupportingDocument) {
    let mut docs: Vec<SupportingDocument> = env
        .storage()
        .persistent()
        .get(&DataKey::Documents(auth_request_id))
        .unwrap_or(Vec::new(env));
    docs.push_back(doc.clone());
    env.storage()
        .persistent()
        .set(&DataKey::Documents(auth_request_id), &docs);
}

// -----------------------------------------------------------------------
// PeerToPeer
// -----------------------------------------------------------------------

pub fn save_peer_to_peer(env: &Env, req: &PeerToPeerRequest) {
    env.storage()
        .persistent()
        .set(&DataKey::PeerToPeer(req.auth_request_id), req);
}

pub fn load_peer_to_peer(env: &Env, auth_request_id: u64) -> Option<PeerToPeerRequest> {
    env.storage()
        .persistent()
        .get(&DataKey::PeerToPeer(auth_request_id))
}

// -----------------------------------------------------------------------
// Appeals
// -----------------------------------------------------------------------

pub fn save_appeal(env: &Env, appeal: &Appeal) {
    // Index by appeal_id for direct lookup
    env.storage()
        .persistent()
        .set(&DataKey::Appeal(appeal.appeal_id), appeal);

    // Also append to the auth request's appeal list
    let mut appeals: Vec<Appeal> = env
        .storage()
        .persistent()
        .get(&DataKey::Appeals(appeal.auth_request_id))
        .unwrap_or(Vec::new(env));
    appeals.push_back(appeal.clone());
    env.storage()
        .persistent()
        .set(&DataKey::Appeals(appeal.auth_request_id), &appeals);
}

pub fn load_appeals_for_auth(env: &Env, auth_request_id: u64) -> Vec<Appeal> {
    env.storage()
        .persistent()
        .get(&DataKey::Appeals(auth_request_id))
        .unwrap_or(Vec::new(env))
}

// -----------------------------------------------------------------------
// Review records
// -----------------------------------------------------------------------

pub fn save_review_record(env: &Env, review: &ReviewRecord) {
    env.storage()
        .persistent()
        .set(&DataKey::Review(review.review_id), review);

    let mut history: Vec<ReviewRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::ReviewHistory(review.auth_request_id))
        .unwrap_or(Vec::new(env));
    history.push_back(review.clone());
    env.storage()
        .persistent()
        .set(&DataKey::ReviewHistory(review.auth_request_id), &history);
}

pub fn load_review_history(env: &Env, auth_request_id: u64) -> Vec<ReviewRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::ReviewHistory(auth_request_id))
        .unwrap_or(Vec::new(env))
}

// -----------------------------------------------------------------------
// Extension
// -----------------------------------------------------------------------

pub fn save_extension(env: &Env, ext: &ExtensionRequest) {
    env.storage()
        .persistent()
        .set(&DataKey::Extension(ext.auth_request_id), ext);
}

// -----------------------------------------------------------------------
// Usage records
// -----------------------------------------------------------------------

pub fn save_usage_record(env: &Env, record: &UsageRecord) {
    let mut records: Vec<UsageRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::UsageRecords(record.auth_request_id))
        .unwrap_or(Vec::new(env));
    records.push_back(record.clone());
    env.storage()
        .persistent()
        .set(&DataKey::UsageRecords(record.auth_request_id), &records);
}

// -----------------------------------------------------------------------
// Reviewer Registry
// -----------------------------------------------------------------------

pub fn save_reviewer(env: &Env, reviewer: &Reviewer) {
    env.storage()
        .persistent()
        .set(&DataKey::Reviewer(reviewer.reviewer_id.clone()), reviewer);

    // Add to insurer's reviewer list
    let mut reviewers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::InsurerReviewers(reviewer.insurer_id.clone()))
        .unwrap_or(Vec::new(env));
    
    let mut already_listed = false;
    for r in reviewers.iter() {
        if r == reviewer.reviewer_id {
            already_listed = true;
            break;
        }
    }
    if !already_listed {
        reviewers.push_back(reviewer.reviewer_id.clone());
        env.storage()
            .persistent()
            .set(&DataKey::InsurerReviewers(reviewer.insurer_id.clone()), &reviewers);
    }
}

pub fn load_reviewer(env: &Env, reviewer_id: &Address) -> Option<Reviewer> {
    env.storage()
        .persistent()
        .get(&DataKey::Reviewer(reviewer_id.clone()))
}

pub fn load_insurer_reviewers(env: &Env, insurer_id: &Address) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::InsurerReviewers(insurer_id.clone()))
        .unwrap_or(Vec::new(env))
}

/// Remove a reviewer from an insurer's reviewer list.
pub fn remove_insurer_reviewer(env: &Env, insurer_id: &Address, reviewer_id: &Address) {
    let reviewers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::InsurerReviewers(insurer_id.clone()))
        .unwrap_or(Vec::new(env));

    let mut filtered: Vec<Address> = Vec::new(env);
    for r in reviewers.iter() {
        if r != *reviewer_id {
            filtered.push_back(r);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::InsurerReviewers(insurer_id.clone()), &filtered);
}

pub fn update_reviewer_case_count(env: &Env, reviewer_id: &Address, delta: i32) -> Result<(), crate::types::Error> {
    let mut reviewer: Reviewer = load_reviewer(env, reviewer_id)
        .ok_or(crate::types::Error::ReviewerNotFound)?;

    if delta > 0 {
        if reviewer.current_cases + delta as u32 > reviewer.max_cases {
