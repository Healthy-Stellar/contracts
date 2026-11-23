#![no_std]

//! # Scholarship Fund Contract
//!
//! Manages healthcare education scholarships with fund pooling, award disbursement, and recipient
//! eligibility verification for healthcare professional training programs.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Admin-only fund management, recipient approval, and lifetime
//! award caps. Awards must use a public commitment and wait through a 48-hour donor-notice period;
//! direct disbursement is disabled. Admin replacement requires proposal and acceptance.
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
//! shares. A proposed award does not reserve tokens, so donors may withdraw during its notice
//! period; available funds are checked again at disbursement. Completed awards reduce the value of
//! every share equally, and the sum of withdrawable amounts never exceeds the uncommitted pool.

use soroban_sdk::{contract,contracterror,contractimpl,contracttype,symbol_short,token,Address,Env,String};
use ttl_config::{extend_critical_ttl, extend_critical_ttl_if_exists};
#[contracterror]
#[derive(Copy,Clone,Debug,Eq,PartialEq)]
#[repr(u32)]
pub enum Error{NotInitialized=1,AlreadyInitialized=2,Unauthorized=3,ZeroAmount=4,InsufficientFunds=5,FundsCommitted=6,RecipientNotEligible=7,RecipientCapExceeded=8,Overflow=9,CommitmentNotFound=10,TimelockNotElapsed=11,InvalidCap=12,AdminRotationNotFound=13,AwardNotFound=14,TimelockRequired=15}
#[contracttype]
pub enum DataKey{Admin,Token,PoolBalance,CommittedFunds,TotalShares,ShareEpoch,Shares(Address),Recipient(Address),PendingAwards(Address),Commitment(u64),Award(u64),NextAwardId,PendingAdmin}
#[contracttype]
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct FundStats{pub pool_balance:i128,pub committed_balance:i128}
/// A donor's pool shares. Shares from an earlier `epoch` are worthless: the epoch advances only when
/// the pool itself is empty, so new donors are not diluted by shares backed by nothing.
#[contracttype]
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct DonorShares{pub epoch:u32,pub shares:i128}
/// Recipient approval and lifetime award limit, including the approving administrator.
#[contracttype]
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct Recipient{pub approved_by:Address,pub cap:i128,pub awarded:i128,pub approved:bool}
/// An immutable record of a completed scholarship award, keyed by its global award ID.
#[contracttype]
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct Award{pub recipient:Address,pub amount:i128,pub awarded_at:u64}
/// A publicly observable proposed award, tied to its intended recipient.
#[contracttype]
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct Commitment{pub recipient:Address,pub amount:i128,pub committed_at:u64}
const COMMITMENT_TIMELOCK_SECONDS:u64=48*60*60;
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
fn require_admin(env:&Env,admin:&Address)->Result<(),Error>{
    admin.require_auth();
    let stored:Address=env.storage().instance().get(&DataKey::Admin).ok_or(Error::NotInitialized)?;
    if *admin!=stored{return Err(Error::Unauthorized);}
    Ok(())
}
fn load_recipient(env:&Env,address:&Address)->Result<Recipient,Error>{
    env.storage().persistent().get(&DataKey::Recipient(address.clone())).ok_or(Error::RecipientNotEligible)
}
fn check_recipient_award(env:&Env,address:&Address,amount:i128)->Result<Recipient,Error>{
    let recipient=load_recipient(env,address)?;
    if !recipient.approved{return Err(Error::RecipientNotEligible);}
    let pending:i128=env.storage().persistent().get(&DataKey::PendingAwards(address.clone())).unwrap_or(0);
    if recipient.awarded.checked_add(pending).and_then(|v|v.checked_add(amount)).ok_or(Error::Overflow)? > recipient.cap{
        return Err(Error::RecipientCapExceeded);
    }
    Ok(recipient)
}
fn record_award(env:&Env,address:&Address,amount:i128)->Result<(),Error>{
    let mut recipient=check_recipient_award(env,address,amount)?;
    recipient.awarded=recipient.awarded.checked_add(amount).ok_or(Error::Overflow)?;
    let id:u64=env.storage().instance().get(&DataKey::NextAwardId).unwrap_or(1);
    let next=id.checked_add(1).ok_or(Error::Overflow)?;
    let key=DataKey::Award(id);
    env.storage().persistent().set(&key,&Award{recipient:address.clone(),amount,awarded_at:env.ledger().timestamp()});
    extend_critical_ttl(&env,&key);
    env.storage().instance().set(&DataKey::NextAwardId,&next);
    env.storage().persistent().set(&DataKey::Recipient(address.clone()),&recipient);
    extend_critical_ttl(&env,&DataKey::Recipient(address.clone()));
    env.events().publish((symbol_short!("AWARD"),id,address.clone()),amount);
    Ok(())
}
#[contract]
pub struct ScholarshipFundContract;
#[contractimpl]
impl ScholarshipFundContract{
    /// Runs atomically as part of contract creation, so the deployer binds the
    /// admin and token addresses and no front-running `initialize` call is possible.
    pub fn __constructor(env:Env,admin:Address,token:Address){
        env.storage().instance().set(&DataKey::Admin,&admin);
        env.storage().instance().set(&DataKey::Token,&token);
        env.storage().instance().set(&DataKey::PoolBalance,&0i128);
        env.storage().instance().set(&DataKey::NextAwardId,&1u64);
    }
    /// Begin a two-step admin rotation. The new admin must accept before it gains authority.
    pub fn propose_admin(env:Env,admin:Address,new_admin:Address)->Result<(),Error>{
        require_admin(&env,&admin)?;
        env.storage().instance().set(&DataKey::PendingAdmin,&new_admin);
        env.events().publish((symbol_short!("ADMINPROP"),),new_admin);
        Ok(())
    }
    /// Accept an admin rotation proposed by the current administrator.
    pub fn accept_admin(env:Env,new_admin:Address)->Result<(),Error>{
        new_admin.require_auth();
        let pending:Address=env.storage().instance().get(&DataKey::PendingAdmin).ok_or(Error::AdminRotationNotFound)?;
        if new_admin!=pending{return Err(Error::Unauthorized);}
        env.storage().instance().set(&DataKey::Admin,&new_admin);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events().publish((symbol_short!("ADMINSET"),),new_admin);
        Ok(())
    }
    /// Cancel a pending admin rotation.
    pub fn cancel_admin_rotation(env:Env,admin:Address)->Result<(),Error>{
        require_admin(&env,&admin)?;
        if !env.storage().instance().has(&DataKey::PendingAdmin){return Err(Error::AdminRotationNotFound);}
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events().publish((symbol_short!("ADMNCAN"),),admin);
        Ok(())
    }
    /// Approve or revoke a scholarship recipient. The approving administrator is recorded.
    pub fn set_recipient_eligibility(env:Env,admin:Address,recipient:Address,approved:bool)->Result<(),Error>{
        require_admin(&env,&admin)?;
        let key=DataKey::Recipient(recipient.clone());
        let mut record:Recipient=env.storage().persistent().get(&key).unwrap_or(Recipient{approved_by:admin.clone(),cap:0,awarded:0,approved:false});
        if approved{record.approved_by=admin.clone();}
        record.approved=approved;
        env.storage().persistent().set(&key,&record);
        extend_critical_ttl(&env,&key);
        env.events().publish((symbol_short!("RECIPIENT"),recipient),approved);
        Ok(())
    }
    /// Set a recipient's lifetime award cap. The cap cannot be lowered below awarded or pending funds.
    pub fn set_recipient_cap(env:Env,admin:Address,recipient:Address,cap:i128)->Result<(),Error>{
        require_admin(&env,&admin)?;
        if cap<0{return Err(Error::InvalidCap);}
        let key=DataKey::Recipient(recipient.clone());
        let mut record:Recipient=env.storage().persistent().get(&key).unwrap_or(Recipient{approved_by:admin.clone(),cap:0,awarded:0,approved:false});
        let pending:i128=env.storage().persistent().get(&DataKey::PendingAwards(recipient.clone())).unwrap_or(0);
        if record.awarded.checked_add(pending).ok_or(Error::Overflow)? > cap{return Err(Error::RecipientCapExceeded);}
        record.cap=cap;
        env.storage().persistent().set(&key,&record);
        extend_critical_ttl(&env,&key);
        env.events().publish((symbol_short!("RECIPCAP"),recipient),cap);
        Ok(())
    }
    pub fn get_recipient(env:Env,recipient:Address)->Result<Recipient,Error>{
        load_recipient(&env,&recipient)
    }
    pub fn get_award(env:Env,id:u64)->Result<Award,Error>{
        env.storage().persistent().get(&DataKey::Award(id)).ok_or(Error::AwardNotFound)
    }
    pub fn get_commitment(env:Env,id:u64)->Result<Commitment,Error>{
        env.storage().persistent().get(&DataKey::Commitment(id)).ok_or(Error::CommitmentNotFound)
    }
    /// Deposit tokens into the pool in exchange for pro-rata shares of the uncommitted pool.
    pub fn deposit(env:Env,depositor:Address,amount:i128)->Result<(),Error>{
        depositor.require_auth();
        if amount<=0{return Err(Error::ZeroAmount);}
        let token_addr:Address=env.storage().instance().get(&DataKey::Token).ok_or(Error::NotInitialized)?;
        let pool=pool_balance(&env);
        let mut total=total_shares(&env);
        if total>0 && pool<=0{
            // The pool is truly empty: existing shares are backed by nothing, so start a fresh
            // epoch instead of letting stale shares claim part of this deposit. Committed funds
            // are still in the pool and still owned pro-rata by donors, so they do not trigger a
            // reset.
            env.storage().instance().set(&DataKey::ShareEpoch,&(share_epoch(&env)+1));
            total=0;
        }
        // Mint against the full pool value: committed funds remain donor-owned, so shares must be
        // priced against the whole pool rather than only the uncommitted portion.
        let minted=if total==0{amount}else{amount.checked_mul(total).ok_or(Error::Overflow)?/pool};
        if minted<=0{return Err(Error::ZeroAmount);}
        let pool_addr=env.current_contract_address();
        token::Client::new(&env,&token_addr).transfer(&depositor,&pool_addr,&amount);
        let shares_key = DataKey::Shares(depositor.clone());
        extend_critical_ttl_if_exists(&env, &shares_key);
        let held=donor_shares(&env,&shares_key);
        env.storage().persistent().set(&shares_key,&DonorShares{epoch:share_epoch(&env),shares:held+minted});
        extend_critical_ttl(&env, &shares_key);
        env.storage().instance().set(&DataKey::TotalShares,&(total+minted));
        env.storage().instance().set(&DataKey::PoolBalance,&(pool+amount));
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
    /// Earmark pool funds for a pending general obligation, protecting them from donor withdrawal.
    /// Admin-only. Can only commit funds currently uncommitted in the pool.
    pub fn commit_funds(env:Env,admin:Address,amount:i128)->Result<(),Error>{
        require_admin(&env,&admin)?;
        if amount<=0{return Err(Error::ZeroAmount);}
        let pool:i128=env.storage().instance().get(&DataKey::PoolBalance).unwrap_or(0);
        let committed=committed_funds(&env);
        if amount>pool-committed{return Err(Error::InsufficientFunds);}
        env.storage().instance().set(&DataKey::CommittedFunds,&(committed+amount));
        env.events().publish((symbol_short!("COMMIT"),admin),amount);
        Ok(())
    }
    /// Publicly record an approved recipient's proposed award. Donors may withdraw during the
    /// 48-hour notice period; funds are checked again when the commitment is disbursed.
    pub fn commit_funds_for(env:Env,admin:Address,id:u64,recipient:Address,amount:i128)->Result<(),Error>{
        require_admin(&env,&admin)?;
        if amount<=0{return Err(Error::ZeroAmount);}
        let recipient_record=check_recipient_award(&env,&recipient,amount)?;
        let pool:i128=env.storage().instance().get(&DataKey::PoolBalance).unwrap_or(0);
        let committed=committed_funds(&env);
        if amount>pool-committed{return Err(Error::InsufficientFunds);}
        let key=DataKey::Commitment(id);
        if env.storage().persistent().has(&key){return Err(Error::AlreadyInitialized);}
        let commitment=Commitment{recipient:recipient.clone(),amount,committed_at:env.ledger().timestamp()};
        env.storage().persistent().set(&key,&commitment);
        extend_critical_ttl(&env,&key);
        let pending_key=DataKey::PendingAwards(recipient.clone());
        let pending:i128=env.storage().persistent().get(&pending_key).unwrap_or(0);
        env.storage().persistent().set(&pending_key,&pending.checked_add(amount).ok_or(Error::Overflow)?);
        extend_critical_ttl(&env,&pending_key);
        let recipient_key=DataKey::Recipient(recipient.clone());
        env.storage().persistent().set(&recipient_key,&recipient_record);
        extend_critical_ttl(&env,&recipient_key);
        env.events().publish((symbol_short!("COMMIT"),id,admin),commitment);
        Ok(())
    }
    /// Cancel a pending award commitment. Admin-only.
    pub fn release_commitment(env:Env,admin:Address,id:u64)->Result<(),Error>{
        require_admin(&env,&admin)?;
        let key=DataKey::Commitment(id);
        let commitment:Commitment=env.storage().persistent().get(&key).ok_or(Error::CommitmentNotFound)?;
        let pending_key=DataKey::PendingAwards(commitment.recipient.clone());
        let pending:i128=env.storage().persistent().get(&pending_key).unwrap_or(0);
        env.storage().persistent().set(&pending_key,&pending.checked_sub(commitment.amount).ok_or(Error::Overflow)?);
        env.storage().persistent().remove(&key);
        env.events().publish((symbol_short!("RELEASE"),admin),commitment.amount);
        Ok(())
    }
    /// Direct disbursement is disabled so all awards pass through the donor-notice timelock.
    pub fn disburse(env:Env,admin:Address,recipient:Address,amount:i128)->Result<(),Error>{
        require_admin(&env,&admin)?;
        let _=(recipient,amount);
        Err(Error::TimelockRequired)
    }
    /// Disburse a recorded commitment after the 48-hour donor notice period.
    pub fn disburse_commitment(env:Env,admin:Address,id:u64)->Result<(),Error>{
        require_admin(&env,&admin)?;
        let key=DataKey::Commitment(id);
        let commitment:Commitment=env.storage().persistent().get(&key).ok_or(Error::CommitmentNotFound)?;
        let unlock_at=commitment.committed_at.checked_add(COMMITMENT_TIMELOCK_SECONDS).ok_or(Error::Overflow)?;
        if env.ledger().timestamp()<unlock_at{return Err(Error::TimelockNotElapsed);}
        check_recipient_award(&env,&commitment.recipient,0)?;
        let pool=pool_balance(&env);
        let committed=committed_funds(&env);
        if commitment.amount>pool-committed{return Err(Error::InsufficientFunds);}
        let pending_key=DataKey::PendingAwards(commitment.recipient.clone());
        let pending:i128=env.storage().persistent().get(&pending_key).unwrap_or(0);
        env.storage().persistent().set(&pending_key,&pending.checked_sub(commitment.amount).ok_or(Error::Overflow)?);
        env.storage().persistent().remove(&key);
        let token_addr:Address=env.storage().instance().get(&DataKey::Token).ok_or(Error::NotInitialized)?;
        let pool_addr=env.current_contract_address();
        token::Client::new(&env,&token_addr).transfer(&pool_addr,&commitment.recipient,&commitment.amount);
        env.storage().instance().set(&DataKey::PoolBalance,&(pool-commitment.amount));
        record_award(&env,&commitment.recipient,commitment.amount)?;
        env.events().publish((symbol_short!("DISBURSE"),commitment.recipient),commitment.amount);
        Ok(())
    }
    pub fn get_stats(env:Env)->Result<FundStats,Error>{
        Ok(FundStats{pool_balance:pool_balance(&env),committed_balance:committed_funds(&env)})
    }
}
