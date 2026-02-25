#![no_std]
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, String, Symbol, Vec,
};

const REQUIRED_CREDENTIALS: u32 = 5;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    CaseNotFound = 2,
    InvalidStatusTransition = 3,
    DocumentNotFound = 4,
    InvalidCredentialType = 5,
    InvalidRating = 6,
    InvalidInput = 7,
    PrivilegeNotFound = 8,
    AlreadySuspended = 9,
    NotSuspended = 10,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialingStatus {
    Incomplete,
    InReview,
    CommitteeReview,
    Approved,
    Denied,
    DeferredForMoreInfo,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Privilege {
    pub privilege_id: u64,
    pub privilege_category: Symbol,
    pub privilege_name: String,
    pub scope: String,
    pub restrictions: Vec<String>,
    pub supervision_required: bool,
    pub volume_requirements: Option<u32>,
    pub granted_date: u64,
    pub expiration_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompetencyRating {
    pub competency_area: Symbol,
    pub rating: u32, // 1-5 scale
    pub clinical_examples: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialingCase {
    pub case_id: u64,
    pub provider_id: Address,
    pub facility_id: Address,
    pub case_type: Symbol, // initial, reappointment, addition
    pub status: CredentialingStatus,
    pub initiated_date: u64,
    pub target_completion_date: u64,
    pub verifications_complete: u32,
    pub verifications_required: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialDocument {
    pub document_type: Symbol,
    pub document_hash: BytesN<32>,
    pub issuing_authority: String,
    pub issue_date: u64,
    pub expiration_date: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRecord {
    pub credential_type: Symbol,
    pub verifier: Address,
    pub verification_method: Symbol,
    pub verification_result: bool,
    pub verification_date: u64,
    pub verification_notes: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanctionCheckRecord {
    pub checker: Address,
    pub databases_checked: Vec<Symbol>,
    pub sanctions_found: bool,
    pub check_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerReferenceRecord {
    pub reference_provider: Address,
    pub competency_ratings: Vec<CompetencyRating>,
    pub reference_notes_hash: BytesN<32>,
    pub recommended: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisionalPrivilegeRequest {
    pub request_id: u64,
    pub provider_id: Address,
    pub facility_id: Address,
    pub privilege_category: Symbol,
    pub supervising_provider: Address,
    pub justification: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClinicalActivityRecord {
    pub procedure_code: String,
    pub outcome: Symbol,
    pub complications: bool,
    pub activity_date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusedReviewRecord {
    pub review_id: u64,
    pub provider_id: Address,
    pub facility_id: Address,
    pub trigger_reason: Symbol,
    pub review_type: Symbol,
    pub initiated_by: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecredentialingSchedule {
    pub schedule_id: u64,
    pub provider_id: Address,
    pub facility_id: Address,
    pub due_date: u64,
    pub notification_sent: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuspensionRecord {
    pub suspension_authority: Address,
    pub suspension_reason: String,
    pub suspension_date: u64,
    pub is_immediate: bool,
    pub peer_review_required: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReinstatementRecord {
    pub reinstatement_authority: Address,
    pub corrective_actions_completed: Vec<String>,
    pub monitoring_requirements: Vec<String>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    CaseCounter,
    Case(u64),
    ProviderFacilityCase(Address, Address),
    CaseDocuments(u64),
    CaseVerifications(u64),
    CaseSanctions(u64),
    CasePeerReferences(u64),
    PrivilegeCounter,
    ProviderFacilityPrivileges(Address, Address),
    ProvisionalCounter,
    ProvisionalRequest(u64),
    ProviderFacilityProvisional(Address, Address),
    ProviderFacilityActivities(Address, Address),
    FocusedReviewCounter,
    FocusedReview(u64),
    RecredentialingCounter,
    Recredentialing(u64),
    ProviderFacilityRecredentialings(Address, Address),
    ProviderFacilitySuspensions(Address, Address),
    ProviderFacilityReinstatements(Address, Address),
}

#[contract]
pub struct HealthcareCredentialingSystem;

#[contractimpl]
impl HealthcareCredentialingSystem {
    pub fn initiate_credentialing(
        env: Env,
        provider_id: Address,
        facility_id: Address,
        application_date: u64,
        requested_privileges: Vec<Symbol>,
    ) -> Result<u64, Error> {
        provider_id.require_auth();
        if requested_privileges.is_empty() {
            return Err(Error::InvalidInput);
        }

        let current: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CaseCounter)
            .unwrap_or(0);
        let case_id = current + 1;
        env.storage()
            .instance()
            .set(&DataKey::CaseCounter, &case_id);

        let case = CredentialingCase {
            case_id,
            provider_id: provider_id.clone(),
            facility_id: facility_id.clone(),
            case_type: Symbol::new(&env, "initial"),
            status: CredentialingStatus::Incomplete,
            initiated_date: application_date,
            target_completion_date: application_date + 60 * 60 * 24 * 90,
            verifications_complete: 0,
            verifications_required: REQUIRED_CREDENTIALS,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Case(case_id), &case);
        env.storage().persistent().set(
            &DataKey::ProviderFacilityCase(provider_id, facility_id),
            &case_id,
        );

        Ok(case_id)
    }

    pub fn submit_credential_document(
        env: Env,
        case_id: u64,
        document_type: Symbol, // medical_license, dea, board_cert, cv, references
        document_hash: BytesN<32>,
        issuing_authority: String,
        issue_date: u64,
        expiration_date: Option<u64>,
    ) -> Result<(), Error> {
        let mut case = get_case(&env, case_id)?;
        if !is_supported_credential_type(&env, &document_type) {
            return Err(Error::InvalidCredentialType);
        }

        let mut docs: Vec<CredentialDocument> = env
            .storage()
            .persistent()
            .get(&DataKey::CaseDocuments(case_id))
            .unwrap_or(Vec::new(&env));

        docs.push_back(CredentialDocument {
            document_type,
            document_hash,
            issuing_authority,
            issue_date,
            expiration_date,
        });

        case.status = CredentialingStatus::InReview;
        env.storage()
            .persistent()
            .set(&DataKey::CaseDocuments(case_id), &docs);
        env.storage()
            .persistent()
            .set(&DataKey::Case(case_id), &case);
        Ok(())
    }

    pub fn verify_credential(
        env: Env,
        case_id: u64,
        credential_type: Symbol,
        verifier: Address,
        verification_method: Symbol,
        verification_result: bool,
        verification_date: u64,
        verification_notes: String,
    ) -> Result<(), Error> {
        verifier.require_auth();
        let mut case = get_case(&env, case_id)?;

        let docs: Vec<CredentialDocument> = env
            .storage()
            .persistent()
            .get(&DataKey::CaseDocuments(case_id))
            .unwrap_or(Vec::new(&env));
        if !document_exists_for_type(&docs, &credential_type) {
            return Err(Error::DocumentNotFound);
        }

        let mut records: Vec<VerificationRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::CaseVerifications(case_id))
            .unwrap_or(Vec::new(&env));
        records.push_back(VerificationRecord {
            credential_type,
            verifier,
            verification_method,
            verification_result,
            verification_date,
            verification_notes,
        });
        env.storage()
            .persistent()
            .set(&DataKey::CaseVerifications(case_id), &records);

        if verification_result {
            if case.verifications_complete < case.verifications_required {
                case.verifications_complete += 1;
            }
            if case.verifications_complete >= case.verifications_required {
                case.status = CredentialingStatus::CommitteeReview;
            } else {
                case.status = CredentialingStatus::InReview;
            }
        } else {
            case.status = CredentialingStatus::DeferredForMoreInfo;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Case(case_id), &case);
        Ok(())
    }

    pub fn check_sanctions(
        env: Env,
        case_id: u64,
        checker: Address,
        databases_checked: Vec<Symbol>, // NPDB, OIG, SAM, etc.
        sanctions_found: bool,
        check_date: u64,
    ) -> Result<(), Error> {
        checker.require_auth();
        let mut case = get_case(&env, case_id)?;

        let mut checks: Vec<SanctionCheckRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::CaseSanctions(case_id))
            .unwrap_or(Vec::new(&env));
        checks.push_back(SanctionCheckRecord {
            checker,
            databases_checked,
            sanctions_found,
            check_date,
        });
        env.storage()
            .persistent()
            .set(&DataKey::CaseSanctions(case_id), &checks);

        if sanctions_found {
            case.status = CredentialingStatus::Denied;
        }
        env.storage()
            .persistent()
            .set(&DataKey::Case(case_id), &case);
        Ok(())
    }

    pub fn conduct_peer_reference(
        env: Env,
        case_id: u64,
        reference_provider: Address,
        competency_ratings: Vec<CompetencyRating>,
        reference_notes_hash: BytesN<32>,
        recommended: bool,
    ) -> Result<(), Error> {
        reference_provider.require_auth();
        let mut case = get_case(&env, case_id)?;
        validate_competency_ratings(&competency_ratings)?;

        let mut refs: Vec<PeerReferenceRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::CasePeerReferences(case_id))
            .unwrap_or(Vec::new(&env));
        refs.push_back(PeerReferenceRecord {
            reference_provider,
            competency_ratings,
            reference_notes_hash,
            recommended,
        });
        env.storage()
            .persistent()
            .set(&DataKey::CasePeerReferences(case_id), &refs);

        if !recommended {
            case.status = CredentialingStatus::DeferredForMoreInfo;
            env.storage()
                .persistent()
                .set(&DataKey::Case(case_id), &case);
        }
        Ok(())
    }

    pub fn grant_privileges(
        env: Env,
        case_id: u64,
        credentialing_committee: Address,
        approved_privileges: Vec<Symbol>,
        conditions: Option<Vec<String>>,
        effective_date: u64,
        expiration_date: u64,
    ) -> Result<(), Error> {
        credentialing_committee.require_auth();
        let mut case = get_case(&env, case_id)?;
        if case.status != CredentialingStatus::CommitteeReview {
            return Err(Error::InvalidStatusTransition);
        }

        if approved_privileges.is_empty() || expiration_date <= effective_date {
            return Err(Error::InvalidInput);
        }

        let mut privilege_counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PrivilegeCounter)
            .unwrap_or(0);
        let mut current: Vec<Privilege> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderFacilityPrivileges(
                case.provider_id.clone(),
                case.facility_id.clone(),
            ))
            .unwrap_or(Vec::new(&env));

        let restrictions_template = conditions.unwrap_or(Vec::new(&env));
        let mut idx: u32 = 0;
        while idx < approved_privileges.len() {
            let category = approved_privileges.get(idx).ok_or(Error::InvalidInput)?;
            privilege_counter += 1;

            current.push_back(Privilege {
                privilege_id: privilege_counter,
                privilege_category: category,
                privilege_name: String::from_str(&env, "Approved Privilege"),
                scope: String::from_str(&env, "facility_scope"),
                restrictions: restrictions_template.clone(),
                supervision_required: false,
                volume_requirements: None,
                granted_date: effective_date,
                expiration_date,
            });
            idx += 1;
        }

        case.status = CredentialingStatus::Approved;
        env.storage()
            .instance()
            .set(&DataKey::PrivilegeCounter, &privilege_counter);
        env.storage().persistent().set(
            &DataKey::ProviderFacilityPrivileges(
                case.provider_id.clone(),
                case.facility_id.clone(),
            ),
            &current,
        );
        env.storage()
            .persistent()
            .set(&DataKey::Case(case_id), &case);
        Ok(())
    }

    pub fn request_provisional_privileges(
        env: Env,
        provider_id: Address,
        facility_id: Address,
        privilege_category: Symbol,
        supervising_provider: Address,
        justification: String,
    ) -> Result<u64, Error> {
        provider_id.require_auth();
        let current: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ProvisionalCounter)
            .unwrap_or(0);
        let request_id = current + 1;
        env.storage()
            .instance()
            .set(&DataKey::ProvisionalCounter, &request_id);

        env.storage().persistent().set(
            &DataKey::ProvisionalRequest(request_id),
            &ProvisionalPrivilegeRequest {
                request_id,
                provider_id: provider_id.clone(),
                facility_id: facility_id.clone(),
                privilege_category,
                supervising_provider,
                justification,
            },
        );

        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderFacilityProvisional(
                provider_id.clone(),
                facility_id.clone(),
            ))
            .unwrap_or(Vec::new(&env));
        ids.push_back(request_id);
        env.storage().persistent().set(
            &DataKey::ProviderFacilityProvisional(provider_id, facility_id),
            &ids,
        );

        Ok(request_id)
    }

    pub fn track_clinical_activity(
        env: Env,
        provider_id: Address,
        facility_id: Address,
        procedure_code: String,
        outcome: Symbol,
        complications: bool,
        activity_date: u64,
    ) -> Result<(), Error> {
        provider_id.require_auth();
        let mut records: Vec<ClinicalActivityRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderFacilityActivities(
                provider_id.clone(),
                facility_id.clone(),
            ))
            .unwrap_or(Vec::new(&env));

        records.push_back(ClinicalActivityRecord {
            procedure_code,
            outcome,
            complications,
            activity_date,
        });
        env.storage().persistent().set(
            &DataKey::ProviderFacilityActivities(provider_id, facility_id),
            &records,
        );
        Ok(())
    }

    pub fn trigger_focused_review(
        env: Env,
        provider_id: Address,
        facility_id: Address,
        trigger_reason: Symbol,
        review_type: Symbol,
        initiated_by: Address,
    ) -> Result<u64, Error> {
        initiated_by.require_auth();
        let current: u64 = env
            .storage()
            .instance()
            .get(&DataKey::FocusedReviewCounter)
            .unwrap_or(0);
        let review_id = current + 1;
        env.storage()
            .instance()
            .set(&DataKey::FocusedReviewCounter, &review_id);

        env.storage().persistent().set(
            &DataKey::FocusedReview(review_id),
            &FocusedReviewRecord {
                review_id,
                provider_id,
                facility_id,
                trigger_reason,
                review_type,
                initiated_by,
            },
        );
        Ok(review_id)
    }

    pub fn schedule_recredentialing(
        env: Env,
        provider_id: Address,
        facility_id: Address,
        due_date: u64,
        notification_sent: bool,
    ) -> Result<u64, Error> {
        provider_id.require_auth();
        let current: u64 = env
            .storage()
            .instance()
            .get(&DataKey::RecredentialingCounter)
            .unwrap_or(0);
        let schedule_id = current + 1;
        env.storage()
            .instance()
            .set(&DataKey::RecredentialingCounter, &schedule_id);

        env.storage().persistent().set(
            &DataKey::Recredentialing(schedule_id),
            &RecredentialingSchedule {
                schedule_id,
                provider_id: provider_id.clone(),
                facility_id: facility_id.clone(),
                due_date,
                notification_sent,
            },
        );

        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderFacilityRecredentialings(
                provider_id.clone(),
                facility_id.clone(),
            ))
            .unwrap_or(Vec::new(&env));
        ids.push_back(schedule_id);
        env.storage().persistent().set(
            &DataKey::ProviderFacilityRecredentialings(provider_id, facility_id),
            &ids,
        );

        Ok(schedule_id)
    }

    pub fn suspend_privileges(
        env: Env,
        provider_id: Address,
        facility_id: Address,
        suspension_authority: Address,
        suspension_reason: String,
        suspension_date: u64,
        is_immediate: bool,
        peer_review_required: bool,
    ) -> Result<(), Error> {
        suspension_authority.require_auth();
        let mut privileges: Vec<Privilege> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderFacilityPrivileges(
                provider_id.clone(),
                facility_id.clone(),
            ))
            .ok_or(Error::PrivilegeNotFound)?;

        let suspended_marker = String::from_str(&env, "SUSPENDED");
        let immediate_marker = String::from_str(&env, "IMMEDIATE");
        let peer_marker = String::from_str(&env, "PEER_REVIEW_REQ");

        if all_privileges_have_marker(&privileges, &suspended_marker) {
            return Err(Error::AlreadySuspended);
        }

        let mut idx: u32 = 0;
        while idx < privileges.len() {
            let mut p = privileges.get(idx).ok_or(Error::PrivilegeNotFound)?;
            p.restrictions = add_unique_marker(p.restrictions, &suspended_marker);
            if is_immediate {
                p.restrictions = add_unique_marker(p.restrictions, &immediate_marker);
            }
            if peer_review_required {
                p.restrictions = add_unique_marker(p.restrictions, &peer_marker);
            }
            privileges.set(idx, p);
            idx += 1;
        }

        let mut history: Vec<SuspensionRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderFacilitySuspensions(
                provider_id.clone(),
                facility_id.clone(),
            ))
            .unwrap_or(Vec::new(&env));
        history.push_back(SuspensionRecord {
            suspension_authority,
            suspension_reason,
            suspension_date,
            is_immediate,
            peer_review_required,
        });

        env.storage().persistent().set(
            &DataKey::ProviderFacilityPrivileges(provider_id.clone(), facility_id.clone()),
            &privileges,
        );
        env.storage().persistent().set(
            &DataKey::ProviderFacilitySuspensions(provider_id, facility_id),
            &history,
        );
        Ok(())
    }

    pub fn reinstate_privileges(
        env: Env,
        provider_id: Address,
        facility_id: Address,
        reinstatement_authority: Address,
        corrective_actions_completed: Vec<String>,
        monitoring_requirements: Vec<String>,
    ) -> Result<(), Error> {
        reinstatement_authority.require_auth();
        let mut privileges: Vec<Privilege> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderFacilityPrivileges(
                provider_id.clone(),
                facility_id.clone(),
            ))
            .ok_or(Error::PrivilegeNotFound)?;

        let suspended_marker = String::from_str(&env, "SUSPENDED");
        if !any_privilege_has_marker(&privileges, &suspended_marker) {
            return Err(Error::NotSuspended);
        }

        let immediate_marker = String::from_str(&env, "IMMEDIATE");
        let peer_marker = String::from_str(&env, "PEER_REVIEW_REQ");
        let mut idx: u32 = 0;
        while idx < privileges.len() {
            let mut p = privileges.get(idx).ok_or(Error::PrivilegeNotFound)?;
            p.restrictions = remove_marker(&env, p.restrictions, &suspended_marker);
            p.restrictions = remove_marker(&env, p.restrictions, &immediate_marker);
            p.restrictions = remove_marker(&env, p.restrictions, &peer_marker);
            privileges.set(idx, p);
            idx += 1;
        }

        let mut history: Vec<ReinstatementRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::ProviderFacilityReinstatements(
                provider_id.clone(),
                facility_id.clone(),
            ))
            .unwrap_or(Vec::new(&env));
        history.push_back(ReinstatementRecord {
            reinstatement_authority,
            corrective_actions_completed,
            monitoring_requirements,
        });

        env.storage().persistent().set(
            &DataKey::ProviderFacilityPrivileges(provider_id.clone(), facility_id.clone()),
            &privileges,
        );
        env.storage().persistent().set(
            &DataKey::ProviderFacilityReinstatements(provider_id, facility_id),
            &history,
        );
        Ok(())
    }

    pub fn get_credentialing_case(env: Env, case_id: u64) -> Result<CredentialingCase, Error> {
        get_case(&env, case_id)
    }

    pub fn get_provider_privileges(
        env: Env,
        provider_id: Address,
        facility_id: Address,
    ) -> Vec<Privilege> {
        env.storage()
            .persistent()
            .get(&DataKey::ProviderFacilityPrivileges(
                provider_id,
                facility_id,
            ))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_clinical_activities(
        env: Env,
        provider_id: Address,
        facility_id: Address,
    ) -> Vec<ClinicalActivityRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::ProviderFacilityActivities(
                provider_id,
                facility_id,
            ))
            .unwrap_or(Vec::new(&env))
    }
}

fn get_case(env: &Env, case_id: u64) -> Result<CredentialingCase, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Case(case_id))
        .ok_or(Error::CaseNotFound)
}

fn is_supported_credential_type(env: &Env, credential_type: &Symbol) -> bool {
    *credential_type == Symbol::new(env, "medical_license")
        || *credential_type == Symbol::new(env, "dea")
        || *credential_type == Symbol::new(env, "board_cert")
        || *credential_type == Symbol::new(env, "cv")
        || *credential_type == Symbol::new(env, "references")
}

fn document_exists_for_type(docs: &Vec<CredentialDocument>, credential_type: &Symbol) -> bool {
    let mut idx: u32 = 0;
    while idx < docs.len() {
        if let Some(doc) = docs.get(idx) {
            if doc.document_type == *credential_type {
                return true;
            }
        }
        idx += 1;
    }
    false
}

fn validate_competency_ratings(ratings: &Vec<CompetencyRating>) -> Result<(), Error> {
    if ratings.is_empty() {
        return Err(Error::InvalidRating);
    }
    let mut idx: u32 = 0;
    while idx < ratings.len() {
        let rating = ratings.get(idx).ok_or(Error::InvalidRating)?;
        if rating.rating < 1 || rating.rating > 5 {
            return Err(Error::InvalidRating);
        }
        idx += 1;
    }
    Ok(())
}

fn marker_exists(values: &Vec<String>, marker: &String) -> bool {
    let mut idx: u32 = 0;
    while idx < values.len() {
        if let Some(v) = values.get(idx) {
            if v == *marker {
                return true;
            }
        }
        idx += 1;
    }
    false
}

fn add_unique_marker(mut values: Vec<String>, marker: &String) -> Vec<String> {
    if !marker_exists(&values, marker) {
        values.push_back(marker.clone());
    }
    values
}

fn remove_marker(env: &Env, values: Vec<String>, marker: &String) -> Vec<String> {
    let mut out = Vec::new(env);
    let mut idx: u32 = 0;
    while idx < values.len() {
        if let Some(v) = values.get(idx) {
            if v != *marker {
                out.push_back(v);
            }
        }
        idx += 1;
    }
    out
}

fn any_privilege_has_marker(privileges: &Vec<Privilege>, marker: &String) -> bool {
    let mut idx: u32 = 0;
    while idx < privileges.len() {
        if let Some(p) = privileges.get(idx) {
            if marker_exists(&p.restrictions, marker) {
                return true;
            }
        }
        idx += 1;
    }
    false
}

fn all_privileges_have_marker(privileges: &Vec<Privilege>, marker: &String) -> bool {
    if privileges.is_empty() {
        return false;
    }
    let mut idx: u32 = 0;
    while idx < privileges.len() {
        if let Some(p) = privileges.get(idx) {
            if !marker_exists(&p.restrictions, marker) {
                return false;
            }
        }
        idx += 1;
    }
    true
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address};

    fn create_case(
        env: &Env,
        client: &HealthcareCredentialingSystemClient,
        provider: &Address,
        facility: &Address,
    ) -> u64 {
        let mut requested = Vec::new(env);
        requested.push_back(Symbol::new(env, "surgery"));
        client.initiate_credentialing(provider, facility, &1_700_000_000, &requested)
    }

    fn submit_required_docs(env: &Env, client: &HealthcareCredentialingSystemClient, case_id: u64) {
        let types = ["medical_license", "dea", "board_cert", "cv", "references"];

        let mut idx: usize = 0;
        while idx < types.len() {
            client.submit_credential_document(
                &case_id,
                &Symbol::new(env, types[idx]),
                &BytesN::from_array(env, &[idx as u8; 32]),
                &String::from_str(env, "Issuer"),
                &1_700_000_000,
                &Some(1_900_000_000),
            );
            idx += 1;
        }
    }

    #[test]
    fn full_credentialing_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(HealthcareCredentialingSystem, ());
        let client = HealthcareCredentialingSystemClient::new(&env, &contract_id);

        let provider = Address::generate(&env);
        let facility = Address::generate(&env);
        let verifier = Address::generate(&env);
        let checker = Address::generate(&env);
        let ref_provider = Address::generate(&env);
        let committee = Address::generate(&env);

        let case_id = create_case(&env, &client, &provider, &facility);
        submit_required_docs(&env, &client, case_id);

        let verify_types = ["medical_license", "dea", "board_cert", "cv", "references"];
        let mut idx: usize = 0;
        while idx < verify_types.len() {
            client.verify_credential(
                &case_id,
                &Symbol::new(&env, verify_types[idx]),
                &verifier,
                &Symbol::new(&env, "primary_source"),
                &true,
                &1_700_010_000,
                &String::from_str(&env, "Verified"),
            );
            idx += 1;
        }

        let mut dbs = Vec::new(&env);
        dbs.push_back(Symbol::new(&env, "NPDB"));
        dbs.push_back(Symbol::new(&env, "OIG"));
        dbs.push_back(Symbol::new(&env, "SAM"));
        client.check_sanctions(&case_id, &checker, &dbs, &false, &1_700_020_000);

        let mut ratings = Vec::new(&env);
        ratings.push_back(CompetencyRating {
            competency_area: Symbol::new(&env, "clinical_judgment"),
            rating: 5,
            clinical_examples: true,
        });
        client.conduct_peer_reference(
            &case_id,
            &ref_provider,
            &ratings,
            &BytesN::from_array(&env, &[9; 32]),
            &true,
        );

        let mut approved = Vec::new(&env);
        approved.push_back(Symbol::new(&env, "surgery"));
        approved.push_back(Symbol::new(&env, "icu"));

        let mut conditions = Vec::new(&env);
        conditions.push_back(String::from_str(&env, "proctoring_required"));
        client.grant_privileges(
            &case_id,
            &committee,
            &approved,
            &Some(conditions),
            &1_700_030_000,
            &1_900_000_000,
        );

        let case = client.get_credentialing_case(&case_id);
        assert_eq!(case.status, CredentialingStatus::Approved);

        let privileges = client.get_provider_privileges(&provider, &facility);
        assert_eq!(privileges.len(), 2);
    }

    #[test]
    fn verification_requires_submitted_document() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(HealthcareCredentialingSystem, ());
        let client = HealthcareCredentialingSystemClient::new(&env, &contract_id);

        let provider = Address::generate(&env);
        let facility = Address::generate(&env);
        let verifier = Address::generate(&env);
        let case_id = create_case(&env, &client, &provider, &facility);

        let res = client.try_verify_credential(
            &case_id,
            &Symbol::new(&env, "medical_license"),
            &verifier,
            &Symbol::new(&env, "primary_source"),
            &true,
            &1_700_010_000,
            &String::from_str(&env, "Verified"),
        );
        assert_eq!(res, Err(Ok(Error::DocumentNotFound)));
    }

    #[test]
    fn invalid_peer_rating_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(HealthcareCredentialingSystem, ());
        let client = HealthcareCredentialingSystemClient::new(&env, &contract_id);

        let provider = Address::generate(&env);
        let facility = Address::generate(&env);
        let ref_provider = Address::generate(&env);
        let case_id = create_case(&env, &client, &provider, &facility);

        let mut ratings = Vec::new(&env);
        ratings.push_back(CompetencyRating {
            competency_area: Symbol::new(&env, "communication"),
            rating: 6,
            clinical_examples: false,
        });
        let res = client.try_conduct_peer_reference(
            &case_id,
            &ref_provider,
            &ratings,
            &BytesN::from_array(&env, &[0; 32]),
            &true,
        );
        assert_eq!(res, Err(Ok(Error::InvalidRating)));
    }

    #[test]
    fn sanctions_can_deny_case() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(HealthcareCredentialingSystem, ());
        let client = HealthcareCredentialingSystemClient::new(&env, &contract_id);

        let provider = Address::generate(&env);
        let facility = Address::generate(&env);
        let checker = Address::generate(&env);
        let case_id = create_case(&env, &client, &provider, &facility);

        let mut dbs = Vec::new(&env);
        dbs.push_back(Symbol::new(&env, "NPDB"));
        client.check_sanctions(&case_id, &checker, &dbs, &true, &1_700_020_000);

        let case = client.get_credentialing_case(&case_id);
        assert_eq!(case.status, CredentialingStatus::Denied);
    }

    #[test]
    fn provisional_review_schedule_and_activity_flow() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(HealthcareCredentialingSystem, ());
        let client = HealthcareCredentialingSystemClient::new(&env, &contract_id);

        let provider = Address::generate(&env);
        let facility = Address::generate(&env);
        let supervisor = Address::generate(&env);
        let reviewer = Address::generate(&env);

        let req_id = client.request_provisional_privileges(
            &provider,
            &facility,
            &Symbol::new(&env, "icu"),
            &supervisor,
            &String::from_str(&env, "new hire pending references"),
        );
        assert_eq!(req_id, 1);

        client.track_clinical_activity(
            &provider,
            &facility,
            &String::from_str(&env, "99291"),
            &Symbol::new(&env, "successful"),
            &false,
            &1_700_050_000,
        );

        let review_id = client.trigger_focused_review(
            &provider,
            &facility,
            &Symbol::new(&env, "complication_rate"),
            &Symbol::new(&env, "fppe"),
            &reviewer,
        );
        assert_eq!(review_id, 1);

        let schedule_id =
            client.schedule_recredentialing(&provider, &facility, &1_800_000_000, &true);
        assert_eq!(schedule_id, 1);

        let activities = client.get_clinical_activities(&provider, &facility);
        assert_eq!(activities.len(), 1);
    }

    #[test]
    fn suspend_and_reinstate_privileges() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(HealthcareCredentialingSystem, ());
        let client = HealthcareCredentialingSystemClient::new(&env, &contract_id);

        let provider = Address::generate(&env);
        let facility = Address::generate(&env);
        let verifier = Address::generate(&env);
        let checker = Address::generate(&env);
        let ref_provider = Address::generate(&env);
        let committee = Address::generate(&env);
        let authority = Address::generate(&env);

        let case_id = create_case(&env, &client, &provider, &facility);
        submit_required_docs(&env, &client, case_id);

        let verify_types = ["medical_license", "dea", "board_cert", "cv", "references"];
        let mut idx: usize = 0;
        while idx < verify_types.len() {
            client.verify_credential(
                &case_id,
                &Symbol::new(&env, verify_types[idx]),
                &verifier,
                &Symbol::new(&env, "primary_source"),
                &true,
                &1_700_010_000,
                &String::from_str(&env, "Verified"),
            );
            idx += 1;
        }

        let mut dbs = Vec::new(&env);
        dbs.push_back(Symbol::new(&env, "NPDB"));
        client.check_sanctions(&case_id, &checker, &dbs, &false, &1_700_020_000);

        let mut ratings = Vec::new(&env);
        ratings.push_back(CompetencyRating {
            competency_area: Symbol::new(&env, "clinical_judgment"),
            rating: 4,
            clinical_examples: true,
        });
        client.conduct_peer_reference(
            &case_id,
            &ref_provider,
            &ratings,
            &BytesN::from_array(&env, &[7; 32]),
            &true,
        );

        let mut approved = Vec::new(&env);
        approved.push_back(Symbol::new(&env, "icu"));
        client.grant_privileges(
            &case_id,
            &committee,
            &approved,
            &None,
            &1_700_030_000,
            &1_900_000_000,
        );

        client.suspend_privileges(
            &provider,
            &facility,
            &authority,
            &String::from_str(&env, "Quality concern"),
            &1_700_040_000,
            &true,
            &true,
        );

        let after_suspend = client.get_provider_privileges(&provider, &facility);
        assert!(marker_exists(
            &after_suspend.get(0).unwrap().restrictions,
            &String::from_str(&env, "SUSPENDED")
        ));

        let mut actions = Vec::new(&env);
        actions.push_back(String::from_str(&env, "proctoring complete"));
        let mut monitoring = Vec::new(&env);
        monitoring.push_back(String::from_str(&env, "90-day review"));
        client.reinstate_privileges(&provider, &facility, &authority, &actions, &monitoring);

        let after_reinstate = client.get_provider_privileges(&provider, &facility);
        assert!(!marker_exists(
            &after_reinstate.get(0).unwrap().restrictions,
            &String::from_str(&env, "SUSPENDED")
        ));
    }
}
