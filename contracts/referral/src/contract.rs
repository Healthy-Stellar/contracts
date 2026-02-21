use crate::types::{DataKey, Error, Referral, ReferralStatus};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Symbol, Vec};

#[contract]
pub struct ReferralContract;

#[contractimpl]
impl ReferralContract {
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
        referring_provider.require_auth();

        let referral_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ReferralCount)
            .unwrap_or(0)
            + 1;

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
        env.storage()
            .instance()
            .set(&DataKey::ReferralCount, &referral_id);

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

    pub fn update_referral_status(
        env: Env,
        referral_id: u64,
        provider_id: Address,
        status: Symbol,
        status_note: Option<String>,
    ) -> Result<(), Error> {
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
