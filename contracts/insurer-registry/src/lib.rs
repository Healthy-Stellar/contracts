#![no_std]

use shared::privacy::validate_nonzero_address;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
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
    PlanNotFound = 8,
}

/// Structured coverage plan with service-code allowlist used by downstream
/// contracts (e.g. prior-authorization) to verify benefit eligibility.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoveragePlan {
    pub plan_id: u64,
    pub plan_name: String,
    pub service_codes: Vec<String>,
    pub is_active: bool,
    pub effective_from: u64,
    pub effective_until: Option<u64>,
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
    pub revocation_reason: Option<Symbol>,
    pub revoked_by: Option<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoveragePlan {
    pub plan_id: u64,
    pub tier: Symbol,
    pub services: Vec<String>,
    pub copay_bps: u32,
    pub deductible: i128,
}

#[contracttype]
pub enum DataKey {
    Insurer(Address),
    ClaimsReviewers(Address),
    /// insurer_wallet -> Vec<CoveragePlan>
    CoveragePlans(Address),
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
                revocation_reason: None,
                revoked_by: None,
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

        Self::assert_credential_valid(&env, &insurer)?;

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

        Self::assert_credential_valid(&env, &insurer)?;

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

    /// Update insurance company coverage policies (free-form human-readable notes)
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

        Self::assert_credential_valid(&env, &insurer)?;

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

    /// Replace the full coverage-plan catalog for an insurer.
    pub fn set_coverage_plans(
        env: Env,
        insurer_wallet: Address,
        plans: Vec<CoveragePlan>,
    ) -> Result<(), Error> {
        validate_nonzero_address(&insurer_wallet).map_err(|_| Error::InvalidAddress)?;
        insurer_wallet.require_auth();

        let insurer_key = DataKey::Insurer(insurer_wallet.clone());
        if !env.storage().persistent().has(&insurer_key) {
            return Err(Error::InsurerNotFound);
        }

        env.storage()
            .persistent()
            .set(&DataKey::CoveragePlans(insurer_wallet), &plans);
        Ok(())
    }

    /// Return all coverage plans registered for an insurer.
    pub fn get_coverage_plans(env: Env, insurer_wallet: Address) -> Vec<CoveragePlan> {
        env.storage()
            .persistent()
            .get(&DataKey::CoveragePlans(insurer_wallet))
            .unwrap_or_else(|| Vec::new(&env))
    }

    fn assert_active_insurer(env: &Env, wallet: &Address) -> Result<(), Error> {
        let key = DataKey::Insurer(wallet.clone());
        let insurer: InsurerData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::InsurerNotFound)?;
        if insurer.credential.revoked_at.is_some() {
            return Err(Error::NotAuthorized);
        }
        Ok(())
    }

    // =====================================================
    //            STRUCTURED COVERAGE PLANS
    // =====================================================

    /// Add a structured coverage plan for an insurer.
    /// Returns the auto-assigned plan ID (unique and incrementing per insurer).
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the insurance company
    /// * `tier` - Coverage tier label (e.g. bronze, silver, gold)
    /// * `services` - List of covered service codes or names
    /// * `copay_bps` - Copay as basis points (e.g. 2000 = 20%)
    /// * `deductible` - Annual deductible amount in the smallest currency unit
    pub fn add_coverage_plan(
        env: Env,
        wallet: Address,
        tier: Symbol,
        services: Vec<String>,
        copay_bps: u32,
        deductible: i128,
    ) -> Result<u64, Error> {
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        wallet.require_auth();

        let insurer_key = DataKey::Insurer(wallet.clone());
        if !env.storage().persistent().has(&insurer_key) {
            return Err(Error::InsurerNotFound);
        }

        let counter_key = DataKey::CoveragePlanCounter(wallet.clone());
        let next_id: u64 = env
            .storage()
            .persistent()
            .get(&counter_key)
            .unwrap_or(0_u64)
            + 1;

        let plan = CoveragePlan {
            plan_id: next_id,
            tier,
            services,
            copay_bps,
            deductible,
        };

        let plans_key = DataKey::CoveragePlans(wallet.clone());
        let mut plans: Vec<CoveragePlan> = env
            .storage()
            .persistent()
            .get(&plans_key)
            .unwrap_or_else(|| Vec::new(&env));

        plans.push_back(plan);
        env.storage().persistent().set(&plans_key, &plans);
        env.storage().persistent().set(&counter_key, &next_id);

        env.events()
            .publish((symbol_short!("add_plan"), wallet), next_id);
        Ok(next_id)
    }

    /// Retrieve all structured coverage plans for an insurer.
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the insurance company
    pub fn get_coverage_plans(env: Env, wallet: Address) -> Vec<CoveragePlan> {
        let plans_key = DataKey::CoveragePlans(wallet);
        env.storage()
            .persistent()
            .get(&plans_key)
            .unwrap_or_else(|| Vec::new(&env))
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

        // Verify insurer exists and credential is valid
        let insurer_key = DataKey::Insurer(insurer_wallet.clone());
        let insurer: InsurerData = env
            .storage()
            .persistent()
            .get(&insurer_key)
            .ok_or(Error::InsurerNotFound)?;
        Self::assert_credential_valid(&env, &insurer)?;

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

    /// Add multiple claims reviewers in a single call (max 20 per batch).
    /// The entire batch is validated before any changes are committed — a duplicate
    /// in the batch or against the existing list rolls back the whole operation.
    ///
    /// # Arguments
    /// * `insurer_wallet` - The wallet address of the insurance company
    /// * `reviewers` - List of reviewer wallet addresses (max 20)
    pub fn add_claims_reviewers_batch(
        env: Env,
        insurer_wallet: Address,
        reviewers: Vec<Address>,
    ) -> Result<(), Error> {
        validate_nonzero_address(&insurer_wallet).map_err(|_| Error::InvalidAddress)?;
        insurer_wallet.require_auth();

        if reviewers.len() > 20 {
            return Err(Error::BatchSizeExceeded);
        }

        let insurer_key = DataKey::Insurer(insurer_wallet.clone());
        if !env.storage().persistent().has(&insurer_key) {
            return Err(Error::InsurerNotFound);
        }

        let reviewers_key = DataKey::ClaimsReviewers(insurer_wallet.clone());
        let existing: Vec<Address> = env
            .storage()
            .persistent()
            .get(&reviewers_key)
            .unwrap_or_else(|| Vec::new(&env));

        // Validate entire batch before committing: check against existing list and within batch
        let mut validated: Vec<Address> = Vec::new(&env);

        for i in 0..reviewers.len() {
            let reviewer = reviewers.get(i).unwrap();

            for j in 0..existing.len() {
                if existing.get(j).unwrap() == reviewer {
                    return Err(Error::ReviewerAlreadyAuthorized);
                }
            }

            for k in 0..validated.len() {
                if validated.get(k).unwrap() == reviewer {
                    return Err(Error::ReviewerAlreadyAuthorized);
                }
            }

            validated.push_back(reviewer);
        }

        // All checks passed — commit atomically
        let mut updated = existing;
        for i in 0..validated.len() {
            updated.push_back(validated.get(i).unwrap());
        }
        env.storage().persistent().set(&reviewers_key, &updated);

        env.events().publish(
            (symbol_short!("batch_rev"), insurer_wallet),
            validated.len(),
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

        let rm_insurer_key = DataKey::Insurer(insurer_wallet.clone());
        let rm_insurer: InsurerData = env
            .storage()
            .persistent()
            .get(&rm_insurer_key)
            .ok_or(Error::InsurerNotFound)?;
        Self::assert_credential_valid(&env, &rm_insurer)?;

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

    /// Revoke an insurer's credential, recording the reason and revoking authority (#494).
    ///
    /// Only the credential's original issuer may call this; the insurer cannot self-revoke.
    ///
    /// # Arguments
    /// * `admin`  - Must be the issuer stored in the credential.
    /// * `wallet` - The insurer whose credential is being revoked.
    /// * `reason` - A short symbol code describing the revocation reason.
    pub fn revoke_credential(
        env: Env,
        admin: Address,
        wallet: Address,
        reason: Symbol,
    ) -> Result<(), Error> {
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&wallet).map_err(|_| Error::InvalidAddress)?;
        admin.require_auth();

        let key = DataKey::Insurer(wallet.clone());
        let mut insurer: InsurerData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::InsurerNotFound)?;

        // Only the credential issuer is authorised to revoke.
        if admin != insurer.credential.issuer {
            return Err(Error::NotAuthorized);
        }
        // Insurer cannot self-revoke.
        if admin == wallet {
            return Err(Error::NotAuthorized);
        }

        let now = env.ledger().timestamp();
        insurer.credential.revoked_at = Some(now);
        insurer.credential.revocation_reason = Some(reason.clone());
        insurer.credential.revoked_by = Some(admin.clone());
        env.storage().persistent().set(&key, &insurer);

        env.events().publish(
            (symbol_short!("cred_rev"), wallet, admin),
            reason,
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
