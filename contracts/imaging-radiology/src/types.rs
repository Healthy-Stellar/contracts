use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

/// Reference to an off-chain encrypted envelope (PHI payload).
/// The ledger only stores the content hash + locator, never plaintext PHI.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedEnvelopeRef {
    /// SHA-256 hash of the encrypted envelope contents.
    pub content_hash: BytesN<32>,
    /// Opaque off-chain locator (e.g. content-addressed storage URI/key).
    pub locator: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StudyType {
    XRay,
    Ct,
    Mri,
    Ultrasound,
    Pet,
    Mammography,
    Other,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Priority {
    Routine,
    Urgent,
    Stat,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderStatus {
    Scheduled,
    ImagesUploaded,
    PreliminaryReported,
    FinalReported,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagingOrder {
    pub order_id: u64,
    pub patient: Address,
    pub ordering_provider: Address,
    /// Radiologist assigned to read/report this order.
    pub assigned_radiologist: Address,
    /// Imaging center recorded at scheduling; only this center may upload images.
    pub scheduled_center: Address,
    /// Coded, non-identifying metadata kept in the clear.
    pub study_type: StudyType,
    pub priority: Priority,
    /// Encrypted clinical indication (PHI) — never stored in plaintext.
    pub clinical_indication: EncryptedEnvelopeRef,
    /// Encrypted body part (PHI) — never stored in plaintext.
    pub body_part: EncryptedEnvelopeRef,
    pub status: OrderStatus,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreliminaryReport {
    pub order_id: u64,
    pub radiologist: Address,
    /// Encrypted impression (PHI) — never stored in plaintext.
    pub impression: EncryptedEnvelopeRef,
    pub submitted_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalReport {
    pub order_id: u64,
    pub radiologist: Address,
    /// Encrypted impression (PHI) — never stored in plaintext.
    pub impression: EncryptedEnvelopeRef,
    pub submitted_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grantee {
    pub grantee: Address,
    pub granted_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentGrant {
    pub order_id: u64,
    pub grantees: Vec<Grantee>,
}
