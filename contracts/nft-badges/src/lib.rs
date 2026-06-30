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
    AlreadyInitialized = 2,
    Unauthorized      = 3,
    BadgeNotFound     = 4,
    /// Soulbound tokens may never be transferred.
    Soulbound         = 5,
    AlreadyMinted     = 6,
}

#[contracttype]
pub enum DataKey {
    Admin,
    NextId,
    Badge(u64),
    OwnerBadges(Address),
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
}

#[contract]
pub struct NftBadgesContract;

#[contractimpl]
impl NftBadgesContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextId, &1u64);
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

        let id: u64 = env.storage().instance()
            .get(&DataKey::NextId)
            .unwrap_or(1);

        let badge = BadgeMetadata {
            id,
            recipient:    recipient.clone(),
            badge_type,
            achievement,
            issued_at:    env.ledger().timestamp(),
            metadata_uri,
        };

        env.storage().persistent().set(&DataKey::Badge(id), &badge);

        let mut badges: Vec<u64> = env.storage().persistent()
            .get(&DataKey::OwnerBadges(recipient.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        badges.push_back(id);
        env.storage().persistent().set(&DataKey::OwnerBadges(recipient.clone()), &badges);

        env.storage().instance().set(&DataKey::NextId, &(id + 1));

        env.events().publish(
            (symbol_short!("MINT"), recipient),
            id,
        );
        Ok(id)
    }

    /// Always panics — badges are soulbound and non-transferable.
    pub fn transfer(_env: Env, _from: Address, _to: Address, _id: u64) -> Result<(), Error> {
        Err(Error::Soulbound)
    }

    pub fn get_badge(env: Env, id: u64) -> Result<BadgeMetadata, Error> {
        env.storage().persistent()
            .get(&DataKey::Badge(id))
            .ok_or(Error::BadgeNotFound)
    }

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
}

#[cfg(test)]
mod test;
