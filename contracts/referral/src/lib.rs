#![no_std]
#![allow(clippy::too_many_arguments)]

//! # Referral Contract
//!
//! Manages provider-to-provider referrals with referral routing, specialist acceptance,
//! clinical summary exchange, and referral outcome tracking.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Referring provider authentication for referral creation.
//! Referred provider authentication for acceptance. Patient consent for referral transmission.
//! Clinical summary access restricted to parties involved. Referral status changes authorized
//! by appropriate parties.
//!
//! **Audit Controls:** Referral creation events logged with referring and referred providers.
//! Clinical summary transmission events tracked. Referral acceptance events recorded. Rejection
//! events logged with reason. Referral completion events. Outcome tracking events. Urgent referral
//! flags logged.
//!
//! **Data Retention Policy:** Referrals retained indefinitely for care continuity. Clinical
//! summaries archived with referral. Acceptance/rejection decisions maintained. Outcome
//! information retained. Referral timeline reconstructible from events.
//!
//! **Encryption/Integrity:** Clinical summary data encrypted via secure storage. Referring
//! provider identity validated. Referred provider credentials verified. Patient linkage encrypted.
//! Referral status enumeration prevents invalid states.

pub mod contract;
pub mod test;
pub mod types;
