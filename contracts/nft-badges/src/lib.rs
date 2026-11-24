#![no_std]

//! # NFT Badges Contract
//!
//! Soulbound NFT badge contract for student/provider achievements with non-transferable
//! credential attestation bound to individual addresses.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Badge issuer (credentialing authority) authentication required.
//! Only issuer can mint badges to recipients. Badges are soulbound (non-transferable) preventing
//! credential fraud. Badge revocation restricted to issuer. Recipient privacy maintained via
//! address-only storage.
//!
//! **Audit Controls:** Badge mint events logged with recipient address and achievement type.
//! Revocation events tracked with reason. Badge holder enumeration enabled via contract query.
//! Achievement type enumeration prevents invalid credential types. All minting operations emitted
//! as events.
//!
//! **Data Retention Policy:** Badges retained indefinitely as permanent credential records.
//! Revoked badges marked as revoked without deletion. Badge issuer information retained.
//! Achievement timestamps immutable. Badge history reconstructible from events.
//!
//! **Encryption/Integrity:** Badge recipient address immutable (soulbound property).
//! Achievement type enumeration prevents invalid badges. Issuer identity validated at mint time.
//! Badge storage enforces non-transferability via smart contract logic.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short,
    Address, Env, String, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized    = 1,
    Unauthorized      = 3,
    BadgeNotFound     = 4,
    /// Soulbound tokens may never be transferred.
    Soulbound         = 5,
    AlreadyMinted     = 6,
    AlreadyRevoked    = 7,
    /// A string field is empty or exceeds its maximum length.
    InvalidInput      = 8,
    /// The badge type is not on the admin-managed allow-list.
    InvalidBadgeType  = 9,
    /// No admin rotation is currently pending.
    NoPendingAdmin    = 10,
}

/// Maximum length (in bytes) of a `badge_type` string.
pub const MAX_BADGE_TYPE_LEN: u32 = 32;
/// Maximum length (in bytes) of an `achievement` string.
pub const MAX_ACHIEVEMENT_LEN: u32 = 128;
/// Maximum length (in bytes) of a `metadata_uri` string.
pub const MAX_METADATA_URI_LEN: u32 = 256;

#[contracttype]
pub enum DataKey {
    Admin,
    PendingAdmin,
    NextId,
    Badge(u64),
    OwnerBadges(Address),
    /// Index of the currently active badge for (owner, badge_type).
    ActiveBadge(Address, String),
    /// Admin-managed allow-list of badge types.
    AllowedBadgeType(String),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadgeMetadata {
    pub id:          u64,
    pub recipient:   Address,
    pub badge_type:  String,   // e.g. "completion", "honor_roll", "attendance"
    pub achievement: String,   // human-readable title
    pub issued_at:   u64,
    pub metadata_uri: String,  // IPFS / off-chain URI
    pub revoked:      bool,
    pub revoked_reason: Option<String>,
}

#[contract]
pub struct NftBadgesContract;

#[contractimpl]
impl NftBadgesContract {
    /// Runs atomically as part of contract creation, so the deployer binds the
    /// admin address and no front-running `initialize` call is possible.
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextId, &1u64);
    }

    /// Step 1 of two-step admin rotation: current admin proposes a successor.
    pub fn propose_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);

        env.events().publish(
            (symbol_short!("PROPOSE"), admin),
            new_admin,
        );
        Ok(())
    }

    /// Step 2 of two-step admin rotation: the proposed successor accepts.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        new_admin.require_auth();
        let pending: Address = env.storage().instance()
            .get(&DataKey::PendingAdmin)
            .ok_or(Error::NoPendingAdmin)?;
        if new_admin != pending {
            return Err(Error::Unauthorized);
        }

        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage().instance().remove(&DataKey::PendingAdmin);

        env.events().publish(
            (symbol_short!("ACCEPT"), new_admin.clone()),
            new_admin,
        );
        Ok(())
    }

    /// Add a badge type to the admin-managed allow-list.
    pub fn add_badge_type(env: Env, admin: Address, badge_type: String) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        Self::validate_string(&badge_type, MAX_BADGE_TYPE_LEN)?;

        env.storage().persistent().set(&DataKey::AllowedBadgeType(badge_type.clone()), &true);

        env.events().publish(
            (symbol_short!("TYPEADD"), admin),
            badge_type,
        );
        Ok(())
    }

    /// Remove a badge type from the admin-managed allow-list.
    pub fn remove_badge_type(env: Env, admin: Address, badge_type: String) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        env.storage().persistent().remove(&DataKey::AllowedBadgeType(badge_type.clone()));

        env.events().publish(
            (symbol_short!("TYPEREM"), admin),
            badge_type,
        );
        Ok(())
    }

    /// Returns true if `badge_type` is on the admin-managed allow-list.
    pub fn is_allowed_badge_type(env: Env, badge_type: String) -> bool {
        env.storage().persistent()
            .get::<_, bool>(&DataKey::AllowedBadgeType(badge_type))
            .unwrap_or(false)
    }

    fn validate_string(s: &String, max_len: u32) -> Result<(), Error> {
        let len = s.len();
        if len == 0 || len > max_len {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }

    /// Mint a soulbound badge to `recipient`.
    /// Only the contract admin may call this.
    pub fn mint(
        env:          Env,
        admin:        Address,
        recipient:    Address,
        badge_type:   String,
        achievement:  String,
        metadata_uri: String,
    ) -> Result<u64, Error> {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        Self::validate_string(&badge_type, MAX_BADGE_TYPE_LEN)?;
        Self::validate_string(&achievement, MAX_ACHIEVEMENT_LEN)?;
        Self::validate_string(&metadata_uri, MAX_METADATA_URI_LEN)?;

        let allowed: bool = env.storage().persistent()
            .get::<_, bool>(&DataKey::AllowedBadgeType(badge_type.clone()))
            .unwrap_or(false);
        if !allowed {
            return Err(Error::InvalidBadgeType);
        }

        // O(1) duplicate check via the active-badge index.
        if env.storage().persistent()
            .has(&DataKey::ActiveBadge(recipient.clone(), badge_type.clone()))
        {
            return Err(Error::AlreadyMinted);
        }

        let mut badges: Vec<u64> = env.storage().persistent()
            .get(&DataKey::OwnerBadges(recipient.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let id: u64 = env.storage().instance()
            .get(&DataKey::NextId)
            .unwrap_or(1);

        let badge = BadgeMetadata {
            id,
            recipient:    recipient.clone(),
            badge_type:   badge_type.clone(),
            achievement,
            issued_at:    env.ledger().timestamp(),
            metadata_uri,
            revoked:      false,
            revoked_reason: None,
        };

        env.storage().persistent().set(&DataKey::Badge(id), &badge);

        badges.push_back(id);
        env.storage().persistent().set(&DataKey::OwnerBadges(recipient.clone()), &badges);

        env.storage().persistent().set(
            &DataKey::ActiveBadge(recipient.clone(), badge_type.clone()),
            &id,
        );

        env.storage().instance().set(&DataKey::NextId, &(id + 1));

        env.events().publish(
            (symbol_short!("MINT"), recipient),
            (id, badge_type),
        );
        Ok(id)
    }

    /// Always panics — badges are soulbound and non-transferable.
    pub fn transfer(_env: Env, _from: Address, _to: Address, _id: u64) -> Result<(), Error> {
        Err(Error::Soulbound)
    }

    /// Revoke a previously minted badge. Restricted to the stored admin.
    /// The badge is flagged as revoked (with a reason) rather than deleted.
    pub fn revoke(env: Env, admin: Address, badge_id: u64, reason: String) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        let mut badge: BadgeMetadata = env.storage().persistent()
            .get(&DataKey::Badge(badge_id))
            .ok_or(Error::BadgeNotFound)?;

        if badge.revoked {
            return Err(Error::AlreadyRevoked);
        }

        // Drop the active-badge index entry so a new badge of this type can be minted.
        let active_key = DataKey::ActiveBadge(badge.recipient.clone(), badge.badge_type.clone());
        if let Some(active_id) = env.storage().persistent().get::<_, u64>(&active_key) {
            if active_id == badge_id {
                env.storage().persistent().remove(&active_key);
            }
        }

        badge.revoked = true;
        badge.revoked_reason = Some(reason.clone());
        env.storage().persistent().set(&DataKey::Badge(badge_id), &badge);

        env.events().publish(
            (symbol_short!("REVOKE"), badge.recipient),
            (badge_id, reason),
        );
        Ok(())
    }

    pub fn get_badge(env: Env, id: u64) -> Result<BadgeMetadata, Error> {
        env.storage().persistent()
            .get(&DataKey::Badge(id))
            .ok_or(Error::BadgeNotFound)
    }

    /// Returns true if the badge exists and has not been revoked.
    pub fn is_valid(env: Env, id: u64) -> bool {
        match env.storage().persistent().get::<_, BadgeMetadata>(&DataKey::Badge(id)) {
            Some(badge) => !badge.revoked,
            None => false,
        }
    }

    /// Returns true if `owner` holds a non-revoked badge of `badge_type`.
    pub fn has_active_badge(env: Env, owner: Address, badge_type: String) -> bool {
        env.storage().persistent()
            .has(&DataKey::ActiveBadge(owner, badge_type))
    }

    /// Returns all badge ids ever issued to `owner`, including revoked ones.
    ///
    /// Callers MUST NOT treat a non-empty result as proof of a valid credential;
    /// use `is_valid(id)` or `has_active_badge(owner, badge_type)` instead.
    pub fn badges_of(env: Env, owner: Address) -> Vec<u64> {
        env.storage().persistent()
            .get(&DataKey::OwnerBadges(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage().instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PendingAdmin)
    }
}

#[cfg(test)]
mod test;
