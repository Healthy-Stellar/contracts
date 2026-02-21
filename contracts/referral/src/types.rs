use soroban_sdk::{contracterror, contracttype, Address, String, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    ReferralNotFound = 2,
    InvalidStatusTransition = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferralStatus {
    Pending,
    Accepted,
    Declined,
    Scheduled,
    InProgress,
    Completed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Referral {
    pub referral_id: u64,
    pub referring_provider: Address,
    pub receiving_provider: Address,
    pub patient_id: Address,
    pub specialty: Symbol,
    pub reason: String,
    pub priority: Symbol,
    pub status: ReferralStatus,
    pub created_at: u64,
    pub accepted_at: Option<u64>,
    pub completed_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Referral(u64),
    ReferralCount,
}
