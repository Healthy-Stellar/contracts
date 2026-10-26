use crate::types::{DataKey, Error, Referral, ReferralStatus};
use shared::privacy::validate_nonzero_address;
use shared_contracts::safe_increment;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, IntoVal, String, Symbol, Vec, vec};

#[contract]
pub struct ReferralContract;

#[contractimpl]
impl ReferralContract {
    pub fn initialize(env: Env, admin: Address, provider_registry: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        validate_nonzero_address(&admin).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&provider_registry).map_err(|_| Error::InvalidAddress)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &provider_registry);
        Ok(())
    }

    pub fn set_provider_registry(
        env: Env,
        admin: Address,
        provider_registry: Address,
    ) -> Result<(), Error> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if stored_admin != admin {
            return Err(Error::NotAuthorized);
        }
        admin.require_auth();
        validate_nonzero_address(&provider_registry).map_err(|_| Error::InvalidAddress)?;
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &provider_registry);
        env.events().publish(
            (Symbol::new(&env, "provider_registry_updated"),),
            provider_registry,
        );
        Ok(())
    }

    fn is_provider_registered(env: &Env, provider: &Address) -> bool {
        if let Some(provider_registry) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::ProviderRegistry)
        {
            shared::actor_verification::is_provider_registered(env, &provider_registry, provider)
        } else {
            false
        }
    }

    pub fn create_referral(
        env: Env,
        referring_provider: Address,
        patient_id: Address,
        referred_to: Address,
        specialty: Symbol,
        reason_hash: BytesN<32>,
        priority: Symbol,
        clinical_summary_hash: BytesN<32>,
        requested_services: Vec<String>,
    ) -> Result<u64, Error> {
        validate_nonzero_address(&referring_provider).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&patient_id).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&referred_to).map_err(|_| Error::InvalidAddress)?;
        referring_provider.require_auth();

        if !Self::is_provider_registered(&env, &referring_provider) {
            return Err(Error::ProviderNotRegistered);
        }

        if !Self::is_provider_registered(&env, &referred_to) {
            return Err(Error::ProviderNotRegistered);
        }

        // Patient consent: the patient must co-sign the disclosure of their
        // information to the receiving provider.
        patient_id.require_auth();

        let referral_id = safe_increment(&env, &DataKey::ReferralCount);

        let referral = Referral {
            referral_id,
            referring_provider,
            receiving_provider: referred_to,
            patient_id,
            specialty,
            reason_hash,
            priority,
            status: ReferralStatus::Pending,
            created_at: env.ledger().timestamp(),
            accepted_at: None,
            completed_at: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Referral(referral_id), &referral);

        // Emit events for extended data that is not stored in the state struct to save space
        env.events().publish(
            (Symbol::new(&env, "referral_created"), referral_id),
            (clinical_summary_hash, requested_services),
        );

        Ok(referral_id)
    }

    pub fn accept_referral(
        env: Env,
        referral_id: u64,
        receiving_provider: Address,
        estimated_appointment_date: Option<u64>,
    ) -> Result<(), Error> {
        validate_nonzero_address(&receiving_provider).map_err(|_| Error::InvalidAddress)?;
        receiving_provider.require_auth();

        let mut referral: Referral = env
            .storage()
            .persistent()
            .get(&DataKey::Referral(referral_id))
            .ok_or(Error::ReferralNotFound)?;

        if referral.receiving_provider != receiving_provider {
            return Err(Error::NotAuthorized);
        }

        if referral.status != ReferralStatus::Pending {
            return Err(Error::InvalidStatusTransition);
        }

        referral.status = ReferralStatus::Accepted;
        referral.accepted_at = Some(env.ledger().timestamp());

        env.storage()
            .persistent()
            .set(&DataKey::Referral(referral_id), &referral);
        env.events().publish(
            (Symbol::new(&env, "referral_accepted"), referral_id),
            estimated_appointment_date,
        );

        Ok(())
    }

    pub fn decline_referral(
        env: Env,
        referral_id: u64,
        receiving_provider: Address,
        decline_reason: String,
        suggest_alternative: Option<Address>,
    ) -> Result<(), Error> {
        validate_nonzero_address(&receiving_provider).map_err(|_| Error::InvalidAddress)?;
        if let Some(alt) = &suggest_alternative {
            validate_nonzero_address(alt).map_err(|_| Error::InvalidAddress)?;
        }
        receiving_provider.require_auth();

        let mut referral: Referral = env
            .storage()
            .persistent()
            .get(&DataKey::Referral(referral_id))
            .ok_or(Error::ReferralNotFound)?;

        if referral.receiving_provider != receiving_provider {
            return Err(Error::NotAuthorized);
        }

        if referral.status != ReferralStatus::Pending {
            return Err(Error::InvalidStatusTransition);
        }

        referral.status = ReferralStatus::Declined;

        env.storage()
            .persistent()
            .set(&DataKey::Referral(referral_id), &referral);
        env.events().publish(
            (Symbol::new(&env, "referral_declined"), referral_id),
            (decline_reason, suggest_alternative),
        );

        Ok(())
    }

    /// Returns whether `current -> new_status` is a legal state transition.
    ///
    /// This mirrors (and generalizes to all `ReferralStatus` variants) the
    /// gating already enforced by the dedicated transition functions:
    /// `accept_referral` and `decline_referral` both require `Pending`, and
    /// `complete_referral` rejects `Pending`/`Declined`/`Cancelled`. Same-state
    /// transitions and any backward/terminal-state jump are disallowed.
    fn is_valid_status_transition(current: &ReferralStatus, new_status: &ReferralStatus) -> bool {
        use ReferralStatus::*;
        matches!(
            (current, new_status),
            (Pending, Accepted)
                | (Pending, Declined)
                | (Pending, Cancelled)
                | (Accepted, Scheduled)
                | (Accepted, InProgress)
                | (Accepted, Cancelled)
                | (Accepted, Completed)
                | (Scheduled, InProgress)
                | (Scheduled, Cancelled)
                | (Scheduled, Completed)
                | (InProgress, Completed)
        )
    }

    pub fn update_referral_status(
        env: Env,
        referral_id: u64,
        provider_id: Address,
        status: Symbol,
        status_note: Option<String>,
    ) -> Result<(), Error> {
        validate_nonzero_address(&provider_id).map_err(|_| Error::InvalidAddress)?;
        provider_id.require_auth();

        let mut referral: Referral = env
            .storage()
            .persistent()
            .get(&DataKey::Referral(referral_id))
            .ok_or(Error::ReferralNotFound)?;

        if referral.receiving_provider != provider_id && referral.referring_provider != provider_id
        {
            return Err(Error::NotAuthorized);
        }

        let new_status = if status == Symbol::new(&env, "Pending") 

/* … truncated 1165 chars — edit only what you need near the top … */
