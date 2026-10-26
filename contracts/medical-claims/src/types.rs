use shared::privacy::PolicyMetadata;
use soroban_sdk::{contracterror, contracttype, Address, BytesN, String, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimReconciledEvent {
    pub claim_id: u64,
    pub payment_amount: i128,
    pub claim_amount: i128,
    pub outstanding_balance: i128,
    pub reconciliation_status: ReconciliationStatus,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    ClaimNotFound = 2,
    InvalidAppealLevel = 3,
    InvalidStateTransition = 4,
    AlreadyInitialized = 5,
    NotInitialized = 6,
    InsurerNotRegistered = 7,
    InvalidAmount = 8,
    AmountOverflow = 9,
    InvalidPolicyMetadata = 10,
    /// #300: Patient has not granted consent to this provider, or consent has
    /// expired / been revoked.  The access-control contract is the authority.
    ConsentNotVerified = 11,
    PaymentNotFound = 12,
    PaymentAlreadyReconciled = 13,
    InvalidReconciliationAmount = 14,
    /// #527: Insurer credential is expired or revoked in insurer-registry.
    InsurerNotActive = 15,
    /// #520: Caller is not a registered authorized dispute reviewer.
    NotAuthorizedReviewer = 16,
    /// #520: No dispute exists for the given dispute_id.
    DisputeNotFound = 17,
    /// #520: Dispute is not in the Open state and cannot be resolved again.
    DisputeAlreadyResolved = 18,
    /// #981: Dispute is not in a state that permits the requested transition.
    InvalidDisputeTransition = 19,
    /// #981: Claim is not in a status that permits opening a dispute.
    ClaimNotDisputable = 20,
    /// #981: Claim already has an open (Open/Escalated) dispute.
    DisputeAlreadyOpen = 21,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimStatus {
    Submitted,
    Adjudicated,
    Appealed,
    Paid,
    Closed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconciliationStatus {
    Pending,
    PartiallyPaid,
    FullyReconciled,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Open,
    Resolved,
    Escalated,
    Closed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeRecord {
    pub dispute_id: u64,
    pub claim_id: u64,
    pub opened_by: Address,
    pub status: DisputeStatus,
    pub reason_hash: BytesN<32>,
    pub opened_at: u64,
    pub resolved_by: Option<Address>,
    pub resolution_hash: Option<BytesN<32>>,
    pub resolved_at: Option<u64>,
    pub escalation_level: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeOpenedEvent {
    pub dispute_id: u64,
    pub claim_id: u64,
    pub opened_by: Address,
    pub reason_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeResolvedEvent {
    pub dispute_id: u64,
    pub claim_id: u64,
    pub resolved_by: Address,
    pub resolution_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeEscalatedEvent {
    pub dispute_id: u64,
    pub claim_id: u64,
    pub escalated_by: Address,
    pub escalation_level: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceLine {
    pub procedure_code: String,
    pub modifier: Option<String>,
    pub quantity: u32,
    pub charge_amount: i128,
    pub diagnosis_pointers: Vec<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenialInfo {
    pub line_number: u64,
    pub denial_code: String,
    pub denial_reason_hash: BytesN<32>,
    pub is_appealable: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsurerPaymentRecord {
    pub payment_date: u64,
    pub payment_amount: i128,
    pub payment_reference_hash: BytesN<32>,
    pub reconciled: bool,
    pub financial_record_owner: Option<Address>,
    pub financial_record_idx: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatientPaymentRecord {
    pub payment_date: u64,
    pub payment_amount: i128,
    pub reconciled: bool,
    pub financial_record_owner: Option<Address>,
    pub financial_record_idx: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimRecord {
    pub claim_id: u64,
    pub provider_id: Address,
    pub patient_id: Address,
    pub insurer_id: Address, // bound payer identity
    pub policy_id: u64,
    pub service_date: u64,
    pub service_codes: Vec<ServiceLine>,
    pub diagnosis_hashes: Vec<BytesN<32>>,
    pub details_hash: BytesN<32>,
    pub policy: PolicyMetadata,
    pub total_amount: i128,
    pub status: ClaimStatus,
    pub approved_amount: Option<i128>,
    pub patient_responsibility: Option<i128>,
    pub appeal_level: u32,
    pub insurer_paid_amount: i128,
    pub patient_paid_amount: i128,
    pub reconciliation_status: ReconciliationStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Insurer(Address), // insurer_id -> bool
    ClaimCounter,
    Claim(u64),
    DenialInfos(u64),
    ApprovedLines(u64),
    ProviderClaims(Address),
    PatientClaims(Address),
    ClaimPayment(u64),
    PatientPayment(u64),
    /// #300: Address of the deployed access-control contract used to verify
    /// patient → provider consent before a claim may be submitted.
    AccessControlId,
    FinancialRecordsId,
    ReconciliationThreshold, // configurable threshold in seconds for unreconciled claims
    InsurerUnreconciledClaims(Address), // insurer_id -> Vec<u64> of claim_ids
    /// #527: Address of the deployed insurer-registry contract.
    InsurerRegistryId,
    /// #520: Vec<Address> of addresses authorized to resolve disputes.
    AuthorizedReviewers,
    /// #520: Monotonic counter used to allocate dispute ids.
    DisputeCounter,
    /// #520: dispute_id -> DisputeRecord.
    Dispute(u64),
    /// #520: claim_id -> Vec<u64> of dispute_ids opened against that claim.
    ClaimDisputes(u64),
    /// #781: reviewer_address -> bool indicating if reviewer is deactivated.
    DeactivatedReviewers(Address),
}
