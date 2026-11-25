use soroban_sdk::{contracttype, Address, BytesN, String, Symbol, Vec};

/// Lifecycle states for a lab order.
///
/// Transitions:
///   Ordered -> Assigned -> Completed -> Amended
///   Assigned -> QcFailed (a failed QC run is recorded, not reverted)
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LabOrderStatus {
    Ordered = 0,
    Assigned = 1,
    Completed = 2,
    Amended = 3,
    QcFailed = 4,
}

impl LabOrderStatus {
    /// Whether a lab may be (re)assigned while the order is in this state.
    pub fn can_assign(&self) -> bool {
        matches!(self, LabOrderStatus::Ordered | LabOrderStatus::Assigned)
    }

    /// Whether results may be submitted while the order is in this state.
    pub fn can_submit_results(&self) -> bool {
        matches!(self, LabOrderStatus::Assigned | LabOrderStatus::QcFailed)
    }

    /// Whether results may be amended while the order is in this state.
    pub fn can_amend(&self) -> bool {
        matches!(self, LabOrderStatus::Completed | LabOrderStatus::Amended)
    }
}

/// A single recorded amendment to a completed lab order's results.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabAmendment {
    pub previous_hash: BytesN<32>,
    pub new_hash: BytesN<32>,
    pub reason: String,
    pub lab_id: Address,
    pub amended_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabOrder {
    pub order_id: u64,
    pub patient: Address,
    pub ordering_provider: Address,
    pub lab_id: Option<Address>,
    pub status: LabOrderStatus,
    pub results_hash: Option<BytesN<32>>,
    pub quality_control_passed: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub amendments: Vec<LabAmendment>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabEvent {
    pub order_id: u64,
    pub event_type: Symbol,
    pub actor: Address,
    pub timestamp: u64,
}
