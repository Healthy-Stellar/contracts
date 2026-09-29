use crate::types::{DataKey, Error, Referral, ReferralStatus};
use shared::privacy::validate_nonzero_address;
use shared_contracts::safe_increment;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, IntoVal, String, Symbol, Vec, vec};

#[contract]
pub struct ReferralContract;

#[contractimpl]
impl ReferralContract {
    pub fn initialize(env: Env, provider_registry: Address) -> Result<(), Error> {
        validate_nonzero_address(&provider_registry).map_err(|_| Error::InvalidAddress)?;
        env.storage()
            .instance()
            .set(&DataKey::ProviderRegistry, &provider_registry);
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
        reason: String,
        priority: Symbol,
        clinical_summary_hash: BytesN<32>,
        requested_services: Vec<String>,
    ) -> Result<u64, Error> {
        validate_nonzero_address(&referring_provider).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&patient_id).map_err(|_| Error::InvalidAddress)?;
        validate_nonzero_address(&referred_to).map_err(|_| Error::InvalidAddress)?;
        referring_provider.require_auth();

        if !Self::is_provider_registered(&env, &referred_to) {
            return Err(Error::ProviderNotRegistered);
        }

        let referral_id = safe_increment(&env, &DataKey::ReferralCount);

        let referral = Referral {
            referral_id,
            referring_provider,
            receiving_provider: referred_to,
            patient_id,
            specialty,
            reason,
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

        let new_status = if status == Symbol::new(&env, "Pending") {
            ReferralStatus::Pending
        } else if status == Symbol::new(&env, "Accepted") {
            ReferralStatus::Accepted
        } else if status == Symbol::new(&env, "Declined") {
            ReferralStatus::Declined
        } else if status == Symbol::new(&env, "Scheduled") {
            ReferralStatus::Scheduled
        } else if status == Symbol::new(&env, "InProgress") {
            ReferralStatus::InProgress
        } else if status == Symbol::new(&env, "Completed") {
            ReferralStatus::Completed
        } else if status == Symbol::new(&env, "Cancelled") {
            ReferralStatus::Cancelled
        } else {
            return Err(Error::InvalidStatusTransition);
        };

        if referral.receiving_provider != provider_id
            && matches!(
                &new_status,
                ReferralStatus::Accepted | ReferralStatus::Declined | ReferralStatus::Completed
            )
        {
            return Err(Error::NotAuthorized);
        }

        if !Self::is_valid_status_transition(&referral.status, &new_status) {
            return Err(Error::InvalidStatusTransition);
        }

        // Keep the timestamp fields consistent with what the dedicated
        // accept_referral / complete_referral functions record for the
        // corresponding transitions.
        if new_status == ReferralStatus::Accepted {
            referral.accepted_at = Some(env.ledger().timestamp());
        }
        if new_status == ReferralStatus::Completed {
            referral.completed_at = Some(env.ledger().timestamp());
        }

        referral.status = new_status;
        env.storage()
            .persistent()
            .set(&DataKey::Referral(referral_id), &referral);
        env.events().publish(
            (Symbol::new(&env, "referral_status_updated"), referral_id),
            (status, status_note),
        );

        Ok(())
    }

    pub fn complete_referral(
        env: Env,
        referral_id: u64,
        receiving_provider: Address,
        consultation_summary_hash: BytesN<32>,
        recommendations: String,
        followup_required: bool,
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

        // Cannot complete unless it has at least been accepted
        match referral.status {
            ReferralStatus::Pending | ReferralStatus::Declined | ReferralStatus::Cancelled => {
                return Err(Error::InvalidStatusTransition)
            }
            _ => {}
        }

        referral.status = ReferralStatus::Completed;
        referral.completed_at = Some(env.ledger().timestamp());

        env.storage()
            .persistent()
            .set(&DataKey::Referral(referral_id), &referral);
        env.events().publish(
            (Symbol::new(&env, "referral_completed"), referral_id),
            (
                consultation_summary_hash,
                recommendations,
                followup_required,
            ),
        );

        Ok(())
    }

    pub fn share_care_summary(
        env: Env,
        referral_id: u64,
        from_provider: Address,
        summary_type: Symbol,
        summary_hash: BytesN<32>,
    ) -> Result<(), Error> {
        validate_nonzero_address(&from_provider).map_err(|_| Error::InvalidAddress)?;
        from_provider.require_auth();

        let referral: Referral = env
            .storage()
            .persistent()
            .get(&DataKey::Referral(referral_id))
            .ok_or(Error::ReferralNotFound)?;

        if referral.receiving_provider != from_provider
            && referral.referring_provider != from_provider
        {
            return Err(Error::NotAuthorized);
        }

        env.events().publish(
            (Symbol::new(&env, "care_summary_shared"), referral_id),
            (from_provider, summary_type, summary_hash),
        );

        Ok(())
    }

    pub fn request_care_summary(
        env: Env,
        referral_id: u64,
        requesting_provider: Address,
        information_needed: Vec<String>,
    ) -> Result<(), Error> {
        validate_nonzero_address(&requesting_provider).map_err(|_| Error::InvalidAddress)?;
        requesting_provider.require_auth();

        let referral: Referral = env
            .storage()
            .persistent()
            .get(&DataKey::Referral(referral_id))
            .ok_or(Error::ReferralNotFound)?;

        if referral.receiving_provider != requesting_provider
            && referral.referring_provider != requesting_provider
        {
            return Err(Error::NotAuthorized);
        }

        env.events().publish(
            (Symbol::new(&env, "care_summary_requested"), referral_id),
            (requesting_provider, information_needed),
        );

        Ok(())
    }
}
