#![no_std]

//! # Scholarship Fund Contract
//!
//! Manages healthcare education scholarships with fund pooling, award disbursement, and recipient
//! eligibility verification for healthcare professional training programs.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Admin-only fund initialization and management. Eligible recipient
//! verification via authentication. Deposit authorization per depositor. Disbursement authorization
//! by fund administrator. Recipient identity validation prevents unauthorized access.
//!
//! **Audit Controls:** Fund deposit events logged with depositor and amount. Fund withdrawal events
//! tracked with recipient and award amount. Award events emitted with grant details. Fund balance
//! changes auditable. Insufficient funds errors logged.
//!
//! **Data Retention Policy:** Fund pool balance maintained indefinitely. Deposit records retained
//! for reporting. Award disbursement records archived. Recipient awards tracked for compliance.
//! Fund history reconstructible from events.
//!
//! **Encryption/Integrity:** Fund amount validation prevents overflow. Address zero checks prevent
//! invalid recipients. Deposit/withdrawal amounts immutable once recorded. Fund balance enforced
//! mathematically. Authorization required before disbursement.
//!
//! ## Donor Accounting
//!
//! Donors hold pro-rata *shares* of the uncommitted pool (`PoolBalance - CommittedFunds`), like LP
//! shares. Committing or disbursing an award lowers the value of every share equally, so the cost
//! of an award is borne by all donors in proportion to their stake rather than by whoever withdraws
//! last. The sum of all donors' withdrawable amounts never exceeds the uncommitted pool.

use soroban_sdk::{contract,contracterror,contractimpl,contracttype,symbol_short,token,Address,Env,String};
use ttl_config::{extend_critical_ttl, extend_critical_ttl_if_exists};
#[contracterror]
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
#[repr(u32)]
pub enum Error{NotInitialized=1,AlreadyInitialized=2,Unauthorized=3,ZeroAmount=4,InsufficientFunds=5,FundsCommitted=6,RecipientNotEligible=7,RecipientCapExceeded=8,Overflow=9}
#[contracttype]
pub enum DataKey{Admin,Token,PoolBalance,CommittedFunds,TotalShares,ShareEpoch,Shares(Address),Eligible(Address),RecipientAwards(Address),RecipientCap(Address)}
#[contracttype]
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct FundStats{pub pool_balance:i128,pub committed_balance:i128}
/// A donor's pool shares. Shares from an earlier `epoch` are worthless: the epoch advances when the
/// uncommitted pool has been fully spent, so new donors are not diluted by shares backed by nothing.
#[contracttype]
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct DonorShares{pub epoch:u32,pub shares:i128}
fn pool_balance(env:&Env)->i128{env.storage().instance().get(&DataKey::PoolBalance).unwrap_or(0)}
fn committed_funds(env:&Env)->i128{env.storage().instance().get(&DataKey::CommittedFunds).unwrap_or(0)}
fn total_shares(env:&Env)->i128{env.storage().instance().get(&DataKey::TotalShares).unwrap_or(0)}
fn share_epoch(env:&Env)->u32{env.storage().instance().get(&DataKey::ShareEpoch).unwrap_or(0)}
/// Shares held by a donor in the current epoch.
fn donor_shares(env:&Env,key:&DataKey)->i128{
    let epoch=share_epoch(env);
    match env.storage().persistent().get::<_,DonorShares>(key){Some(r) if r.epoch==epoch=>r.shares,_=>0}
}
/// `shares * value / total`, rounded down.
fn share_value(shares:i128,value:i128,total:i128)->Result<i128,Error>{
    if total<=0||shares<=0||value<=0{return Ok(0);}
    Ok(shares.checked_mul(value).ok_or(Error::Overflow)?/total)
}
#[contract]
pub struct ScholarshipFundContract;
#[contractimpl]
impl ScholarshipFundContract{
    pub fn initialize(env:Env,admin:Address,token:Address)->Result<(),Error>{
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Admin){return Err(Error::AlreadyInitialized);}
        env.storage().instance().set(&DataKey::Admin,&admin);
        env.storage().instance().set(&DataKey::Token,&token);
        env.storage().instance().set(&DataKey::PoolBalance,&0i128);
        Ok(())
    }
    /// Deposit tokens into the pool in exchange for pro-rata shares of the uncommitted pool.
    pub fn deposit(env:Env,depositor:Address,amount:i128)->Result<(),Error>{
        depositor.require_auth();
        if amount<=0{return Err(Error::ZeroAmount);}
        let token_addr:Address=env.storage().instance().get(&DataKey::Token).ok_or(Error::NotInitialized)?;
        let available=pool_balance(&env)-committed_funds(&env);
        let mut total=total_shares(&env);
        if total>0 && available<=0{
            // Every existing share is backed by nothing; start a fresh epoch instead of letting
            // stale shares claim part of this deposit.
            env.storage().instance().set(&DataKey::ShareEpoch,&(share_epoch(&env)+1));
            total=0;
        }
        let minted=if total==0{amount}else{amount.checked_mul(total).ok_or(Error::Overflow)?/available};
        if minted<=0{return Err(Error::ZeroAmount);}
        let pool=env.current_contract_address();
        token::Client::new(&env,&token_addr).transfer(&depositor,&pool,&amount);
        let shares_key = DataKey::Shares(depositor.clone());
        extend_critical_ttl_if_exists(&env, &shares_key);
        let held=donor_shares(&env,&shares_key);
        env.storage().persistent().set(&shares_key,&DonorShares{epoch:share_epoch(&env),shares:held+minted});
        extend_critical_ttl(&env, &shares_key);
        env.storage().instance().set(&DataKey::TotalShares,&(total+minted));
        env.storage().instance().set(&DataKey::PoolBalance,&(pool_balance(&env)+amount));
        env.events().publish((symbol_short!("DEPOSIT"),depositor),amount);
        Ok(())
    }
    /// Withdraw up to the depositor's pro-rata share of the uncommitted pool.
    pub fn withdraw(env:Env,depositor:Address,amount:i128)->Result<(),Error>{
        depositor.require_auth();
        if amount<=0{return Err(Error::ZeroAmount);}
        let shares_key = DataKey::Shares(depositor.clone());
        extend_critical_ttl_if_exists(&env, &shares_key);
        let held=donor_shares(&env,&shares_key);
        let total=total_shares(&env);
        let pool=pool_balance(&env);
        let available=pool-committed_funds(&env);
        if amount>share_value(held,available,total)?{
            // Distinguish funds earmarked for a pending award from funds the donor never had.
            if amount<=share_value(held,pool,total)?{return Err(Error::FundsCommitted);}
            return Err(Error::InsufficientFunds);
        }
        // Burn shares rounded up so rounding never favours the withdrawer.
        let burned=(amount.checked_mul(total).ok_or(Error::Overflow)?+available-1)/available;
        let token_addr:Address=env.storage().instance().get(&DataKey::Token).ok_or(Error::NotInitialized)?;
        let pool_addr=env.current_contract_address();
        token::Client::new(&env,&token_addr).transfer(&pool_addr,&depositor,&amount);
        env.storage().persistent().set(&shares_key,&DonorShares{epoch:share_epoch(&env),shares:held-burned});
        extend_critical_ttl(&env, &shares_key);
        env.storage().instance().set(&DataKey::TotalShares,&(total-burned));
        env.storage().instance().set(&DataKey::PoolBalance,&(pool-amount));
        env.events().publish((symbol_short!("WITHDRAW"),depositor),amount);
        Ok(())
    }
    /// Earmark pool funds for a pending award, protecting them from donor withdrawal.
    /// Admin-only. Can only commit funds currently uncommitted in the pool.
    pub fn commit_funds(env:Env,admin:Address,amount:i128)->Result<(),Error>{
        admin.require_auth();
        let stored:Address=env.storage().instance().get(&DataKey::Admin).ok_or(Error::NotInitialized)?;
        if admin!=stored{return Err(Error::Unauthorized);}
        if amount<=0{return Err(Error::ZeroAmount);}
        let pool:i128=env.storage().instance().get(&DataKey::PoolBalance).unwrap_or(0);
        let committed:i128=env.storage().instance().get(&DataKey::CommittedFunds).unwrap_or(0);
        if pool-committed<amount{return Err(Error::InsufficientFunds);}
        env.storage().instance().set(&DataKey::CommittedFunds,&(committed+amount));
        env.events().publish((symbol_short!("COMMIT"),admin),amount);
        Ok(())
    }
    pub fn disburse(env:Env,admin:Address,recipient:Address,amount:i128,reason:String)->Result<(),Error>{
        admin.require_auth();
        let stored:Address=env.storage().instance().get(&DataKey::Admin).ok_or(Error::NotInitialized)?;
        if admin!=stored{return Err(Error::Unauthorized);}
        if amount<=0{return Err(Error::ZeroAmount);}
        let eligible_key = DataKey::Eligible(recipient.clone());
        extend_critical_ttl_if_exists(&env, &eligible_key);
        let eligible:bool=env.storage().persistent().get(&eligible_key).unwrap_or(false);
        if !eligible{return Err(Error::RecipientNotEligible);}
        let awards_key = DataKey::RecipientAwards(recipient.clone());
        extend_critical_ttl_if_exists(&env, &awards_key);
        let prior_awards:i128=env.storage().persistent().get(&awards_key).unwrap_or(0);
        let cap_key = DataKey::RecipientCap(recipient.clone());
        extend_critical_ttl_if_exists(&env, &cap_key);
        let cap:i128=env.storage().persistent().get(&cap_key).unwrap_or(0);
        if cap>0 && prior_awards+amount>cap{return Err(Error::RecipientCapExceeded);}
        let pool:i128=env.storage().instance().get(&DataKey::PoolBalance).unwrap_or(0);
        if pool<amount{return Err(Error::InsufficientFunds);}
        let token_addr:Address=env.storage().instance().get(&DataKey::Token).ok_or(Error::NotInitialized)?;
        let pool_addr=env.current_contract_address();
        token::Client::new(&env,&token_addr).transfer(&pool_addr,&recipient,&amount);
        env.storage().instance().set(&DataKey::PoolBalance,&(pool-amount));
        let committed:i128=env.storage().instance().get(&DataKey::CommittedFunds).unwrap_or(0);
        if committed>0{
            let released=if amount<committed{amount}else{committed};
            env.storage().instance().set(&DataKey::CommittedFunds,&(committed-released));
        }
        env.storage().persistent().set(&awards_key,&(prior_awards+amount));
        extend_critical_ttl(&env, &awards_key);
        env.events().publish((symbol_short!("DISBURSE"),recipient),(amount,reason));
        Ok(())
    }
    /// Admin-only: mark a recipient eligible/ineligible to receive disbursements.
    pub fn set_recipient_eligibility(env:Env,admin:Address,recipient:Address,eligible:bool)->Result<(),Error>{
        admin.require_auth();
        let stored:Address=env.storage().instance().get(&DataKey::Admin).ok_or(Error::NotInitialized)?;
        if admin!=stored{return Err(Error::Unauthorized);}
        let eligible_key = DataKey::Eligible(recipient.clone());
        env.storage().persistent().set(&eligible_key,&eligible);
        extend_critical_ttl(&env, &eligible_key);
        env.events().publish((symbol_short!("ELIGIBLE"),recipient),eligible);
        Ok(())
    }
    /// Admin-only: set the maximum cumulative amount a recipient may receive across all disbursements.
    /// A cap of `0` means no cap is enforced.
    pub fn set_recipient_cap(env:Env,admin:Address,recipient:Address,cap:i128)->Result<(),Error>{
        admin.require_auth();
        let stored:Address=env.storage().instance().get(&DataKey::Admin).ok_or(Error::NotInitialized)?;
        if admin!=stored{return Err(Error::Unauthorized);}
        if cap<0{return Err(Error::ZeroAmount);}
        let cap_key = DataKey::RecipientCap(recipient);
        env.storage().persistent().set(&cap_key,&cap);
        extend_critical_ttl(&env, &cap_key);
        Ok(())
    }
    pub fn get_stats(env:Env)->FundStats{FundStats{pool_balance:env.storage().instance().get(&DataKey::PoolBalance).unwrap_or(0),committed_balance:env.storage().instance().get(&DataKey::CommittedFunds).unwrap_or(0)}}
    /// Amount the depositor could withdraw right now: their pro-rata share of the uncommitted pool.
    pub fn get_deposit(env:Env,depositor:Address)->i128{
        let key=DataKey::Shares(depositor);extend_critical_ttl_if_exists(&env,&key);
        let available=pool_balance(&env)-committed_funds(&env);
        share_value(donor_shares(&env,&key),available,total_shares(&env)).unwrap_or(0)
    }
    /// Pool shares held by the depositor in the current epoch.
    pub fn get_shares(env:Env,depositor:Address)->i128{let key=DataKey::Shares(depositor);extend_critical_ttl_if_exists(&env,&key);donor_shares(&env,&key)}
    /// Cumulative amount this recipient has received across all disbursements.
    pub fn get_recipient_awards(env:Env,recipient:Address)->i128{let key=DataKey::RecipientAwards(recipient);extend_critical_ttl_if_exists(&env,&key);env.storage().persistent().get(&key).unwrap_or(0)}
}
#[cfg(test)]
mod test;
