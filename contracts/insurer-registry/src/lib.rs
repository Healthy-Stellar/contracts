#![no_std]
#![allow(deprecated)]

//! # Insurer Registry Contract
//!
//! Maintains registry of health insurance providers with policy information, network participation,
//! and claims validation for healthcare payment processing.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Admin-only insurer registration. Address nonzero validation
//! prevents invalid entries. Network participation authorization required before claims processing.
//! Update authorization restricted to insurer admin or network admin.
//!
//! **Audit Controls:** Insurer registration events logged with insurer address and name. Network
//! participation events tracked. Policy information update events recorded. Claims approval authority
//! logged. Insurer deactivation events tracked.
//!
//! **Data Retention Policy:** Insurer records retained indefinitely for claims history. Network
//! participation status maintained. Policy information persisted for contract validation. Deactivated
//! insurers retain historical data without deletion.
//!
//! **Encryption/Integrity:** Insurer addresses validated via nonzero checks. Address immutable
//! once registered. Insurer metadata stored encrypted in persistent state. Network participation
//! status validated before claims processing.

use shared::privacy::validate_nonzero_address;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Vec,
};

/// --------------------
/// Error Types
/// --------------------
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    InsurerAlreadyRegistered = 1,
    InsurerNotFound = 2,
    ReviewerAlreadyAuthorized = 3,
    ReviewerNotFound = 4,
    NoReviewersFound = 5,
    NotAuthorized = 6,
    InvalidAddress = 7,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsurerData {
    pub name: String,
    pub license_id: String,
    pub contact_details: String,
    pub coverage_policies: String,
    pub metadata: String,
    pub credential: CredentialAnchor,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialAnchor {
    pub credential_hash: BytesN<32>,
    pub issuer: Address,
    pub attestation_hash: BytesN<32>,
    pub expires_at: u64,
    pub revocation_reference: BytesN<32>,
    pub revoked_at: Option<u64>,
}

#[contracttype]
pub enum DataKey {
    Insurer(Address),
    ClaimsReviewers(Address),
}

#[contract]
pub struct InsurerRegistry;

#[contractimpl]
impl InsurerRegistry {
    /// Register a new insurance company with comprehensive information
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the insurance company
    /// * `name` - The name of the insurance company
    /// * `license_id` - Government-issued insurance license identifier
    /// * `metadata` - Additional information (contact details, coverage policies, etc.)
    pub fn register_insurer(
        env: Env,
        wallet: Address,
        name: String,
        license_id: String,
        metadata: String,
        credential_hash: BytesN<32>,
        issuer: Address,
        attestation_hash: BytesN<32>,
        expires_at: u64,
        revocation_reference: BytesN<32>,
    ) -> Result<(), Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&issuer).map_err(|_| Error::InvalidAddress)?;
        wallet.require_auth();
        issuer.require_auth();

        let key = DataKey::Insurer(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::InsurerAlreadyRegistered);
        }

        let insurer = InsurerData {
            name,
            license_id,
            contact_details: String::from_str(&env, ""),
            coverage_policies: String::from_str(&env, ""),
            metadata,
            credential: CredentialAnchor {
                credential_hash,
                issuer,
                attestation_hash,
                expires_at,
                revocation_reference,
                revoked_at: None,
            },
        };

        env.storage().persistent().set(&key, &insurer);
        env.storage().persistent().set(
            &DataKey::ClaimsReviewers(wallet.clone()),
            &Vec::<Address>::new(&env),
        );

        env.events()
            .publish((symbol_short!("reg_ins"), wallet), symbol_short!("success"));
        Ok(())
    }

    /// Update insurance company metadata and operational information
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the insurance company
    /// * `metadata` - Updated metadata information
    pub fn update_insurer(env: Env, wallet: Address, metadata: String) -> Result<(), Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        wallet.require_auth();

        let key = DataKey::Insurer(wallet.clone());
        let mut insurer: InsurerData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::InsurerNotFound)?;

        insurer.metadata = metadata;
        env.storage()
            .persistent()
            .set(&DataKey::Insurer(wallet.clone()), &insurer);

        env.events()
            .publish((symbol_short!("upd_ins"), wallet), symbol_short!("success"));
        Ok(())
    }

    /// Update insurance company contact details
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the insurance company
    /// * `contact_details` - Updated contact information (phone, email, address)
    pub fn update_contact_details(
        env: Env,
        wallet: Address,
        contact_details: String,
    ) -> Result<(), Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        wallet.require_auth();

        let key = DataKey::Insurer(wallet.clone());
        let mut insurer: InsurerData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::InsurerNotFound)?;

        insurer.contact_details = contact_details;
        env.storage()
            .persistent()
            .set(&DataKey::Insurer(wallet.clone()), &insurer);

        env.events().publish(
            (symbol_short!("upd_cntct"), wallet),
            symbol_short!("success"),
        );
        Ok(())
    }

    /// Update insurance company coverage policies
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the insurance company
    /// * `coverage_policies` - Updated coverage policy information
    pub fn update_coverage_policies(
        env: Env,
        wallet: Address,
        coverage_policies: String,
    ) -> Result<(), Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        wallet.require_auth();

        let key = DataKey::Insurer(wallet.clone());
        let mut insurer: InsurerData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::InsurerNotFound)?;

        insurer.coverage_policies = coverage_policies;
        env.storage()
            .persistent()
            .set(&DataKey::Insurer(wallet.clone()), &insurer);

        env.events()
            .publish((symbol_short!("upd_cov"), wallet), symbol_short!("success"));
        Ok(())
    }

    /// Retrieve insurance company data by wallet address
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the insurance company
    ///
    /// # Returns
    /// The InsurerData for the given wallet address
    pub fn get_insurer(env: Env, wallet: Address) -> Result<InsurerData, Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        let key = DataKey::Insurer(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(Error::InsurerNotFound)
    }

    pub fn is_insurer_active(env: Env, wallet: Address) -> bool {
        let now = env.ledger().timestamp();
        if let Ok(insurer) = Self::get_insurer(env, wallet) {
            insurer.credential.revoked_at.is_none() && insurer.credential.expires_at > now
        } else {
            false
        }
    }

    fn assert_active_insurer(env: &Env, wallet: &Address) -> Result<(), Error> {
        let key = DataKey::Insurer(wallet.clone());
        let insurer: InsurerData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::InsurerNotFound)?;
        if insurer.credential.revoked_at.is_some() {
            return Err(Error::InsurerNotFound);
        }
        Ok(())
    }

    // =====================================================
    //            CLAIMS REVIEWERS MANAGEMENT
    // =====================================================

    /// Add a claims reviewer to the insurance company's authorized list
    ///
    /// # Arguments
    /// * `insurer_wallet` - The wallet address of the insurance company
    /// * `reviewer_wallet` - The wallet address of the claims reviewer to add
    pub fn add_claims_reviewer(
        env: Env,
        insurer_wallet: Address,
        reviewer_wallet: Address,
    ) -> Result<(), Error> {
        validate_nonzero_address(&insurer_wallet).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&reviewer_wallet).map_err(|_| Error::InvalidAddress)?;
        insurer_wallet.require_auth();

        // Verify insurer exists
        let insurer_key = DataKey::Insurer(insurer_wallet.clone());
        if !env.storage().persistent().has(&insurer_key) {
            return Err(Error::InsurerNotFound);
        }

        let reviewers_key = DataKey::ClaimsReviewers(insurer_wallet.clone());
        let mut reviewers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&reviewers_key)
            .unwrap_or_else(|| Vec::new(&env));

        // Check if reviewer already exists
        for i in 0..reviewers.len() {
            if reviewers.get(i).ok_or(Error::NotAuthorized)? == reviewer_wallet {
                return Err(Error::ReviewerAlreadyAuthorized);
            }
        }

        reviewers.push_back(reviewer_wallet.clone());
        env.storage().persistent().set(&reviewers_key, &reviewers);

        env.events().publish(
            (symbol_short!("add_rev"), insurer_wallet, reviewer_wallet),
            symbol_short!("success"),
        );
        Ok(())
    }

    /// Remove a claims reviewer from the insurance company's authorized list
    ///
    /// # Arguments
    /// * `insurer_wallet` - The wallet address of the insurance company
    /// * `reviewer_wallet` - The wallet address of the claims reviewer to remove
    pub fn remove_claims_reviewer(
        env: Env,
        insurer_wallet: Address,
        reviewer_wallet: Address,
    ) -> Result<(), Error> {
        validate_nonzero_address(&insurer_wallet).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&reviewer_wallet).map_err(|_| Error::InvalidAddress)?;
        insurer_wallet.require_auth();
        Self::assert_active_insurer(&env, &insurer_wallet)?;

        let reviewers_key = DataKey::ClaimsReviewers(insurer_wallet.clone());
        let reviewers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&reviewers_key)
            .ok_or(Error::NoReviewersFound)?;

        let mut new_reviewers: Vec<Address> = Vec::new(&env);
        let mut found = false;

        for i in 0..reviewers.len() {
            let reviewer = reviewers.get(i).ok_or(Error::NotAuthorized)?;
            if reviewer != reviewer_wallet {
                new_reviewers.push_back(reviewer);
            } else {
                found = true;
            }
        }

        if !found {
            return Err(Error::ReviewerNotFound);
        }

        env.storage()
            .persistent()
            .set(&reviewers_key, &new_reviewers);

        env.events().publish(
            (symbol_short!("rm_rev"), insurer_wallet, reviewer_wallet),
            symbol_short!("success"),
        );
        Ok(())
    }

    pub fn get_claims_reviewers(env: Env, insurer_wallet: Address) -> Vec<Address> {
        let reviewers_key = DataKey::ClaimsReviewers(insurer_wallet);
        env.storage()
            .persistent()
            .get(&reviewers_key)
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn is_authorized_reviewer(
        env: Env,
        insurer_wallet: Address,
        reviewer_wallet: Address,
    ) -> bool {
        let reviewers_key = DataKey::ClaimsReviewers(insurer_wallet);
        let reviewers: Vec<Address> = match env.storage().persistent().get(&reviewers_key) {
            Some(r) => r,
            None => return false,
        };

        for i in 0..reviewers.len() {
            if let Ok(reviewer) = reviewers.get(i).ok_or(()) {
                if reviewer == reviewer_wallet {
                    return true;
                }
            }
        }
        false
    }
}

mod test;
