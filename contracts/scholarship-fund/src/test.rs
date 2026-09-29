#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _,token::{StellarAssetClient,TokenClient},Address,Env,String};
struct Fund{env:Env,c:ScholarshipFundContractClient<'static>,admin:Address,token:TokenClient<'static>,sac:StellarAssetClient<'static>}
fn setup()->Fund{
    let env=Env::default();env.mock_all_auths();
    let token_admin=Address::generate(&env);
    let token_id=env.register_stellar_asset_contract_v2(token_admin).address();
    let id=env.register(ScholarshipFundContract,());
    let c=ScholarshipFundContractClient::new(&env,&id);
    let admin=Address::generate(&env);c.initialize(&admin,&token_id);
    let token=TokenClient::new(&env,&token_id);let sac=StellarAssetClient::new(&env,&token_id);
    Fund{env,c,admin,token,sac}
}
/// A fresh address funded with `amount` tokens.
fn donor(f:&Fund,amount:i128)->Address{let d=Address::generate(&f.env);f.sac.mint(&d,&amount);d}
#[test]fn deposit_increases_pool(){let f=setup();let d=donor(&f,500_000);f.c.deposit(&d,&500_000);assert_eq!(f.c.get_stats().pool_balance,500_000);assert_eq!(f.token.balance(&f.c.address),500_000);assert_eq!(f.token.balance(&d),0);assert_eq!(f.c.get_deposit(&d),500_000);}
#[test]fn withdraw_reduces_pool(){let f=setup();let d=donor(&f,1_000_000);f.c.deposit(&d,&1_000_000);f.c.withdraw(&d,&400_000);assert_eq!(f.c.get_stats().pool_balance,600_000);assert_eq!(f.token.balance(&f.c.address),600_000);assert_eq!(f.token.balance(&d),400_000);assert_eq!(f.c.get_deposit(&d),600_000);}
#[test]fn disburse_reduces_pool(){let f=setup();let env=&f.env;let d=donor(&f,2_000_000);let student=Address::generate(env);f.c.deposit(&d,&2_000_000);f.c.set_recipient_eligibility(&f.admin,&student,&true);f.c.disburse(&f.admin,&student,&1_000_000,&String::from_str(env,"award"));assert_eq!(f.c.get_stats().pool_balance,1_000_000);assert_eq!(f.token.balance(&f.c.address),1_000_000);assert_eq!(f.token.balance(&student),1_000_000);}
#[test]#[should_panic]fn disburse_empty_pool_panics(){let f=setup();let s=Address::generate(&f.env);f.c.set_recipient_eligibility(&f.admin,&s,&true);f.c.disburse(&f.admin,&s,&1,&String::from_str(&f.env,"x"));}
#[test]#[should_panic]fn non_admin_disburse_panics(){let f=setup();let a=Address::generate(&f.env);let s=Address::generate(&f.env);let d=donor(&f,1_000_000);f.c.deposit(&d,&1_000_000);f.c.set_recipient_eligibility(&f.admin,&s,&true);f.c.disburse(&a,&s,&1,&String::from_str(&f.env,"x"));}
#[test]#[should_panic]fn over_withdraw_panics(){let f=setup();let d=donor(&f,100_000);f.c.deposit(&d,&100_000);f.c.withdraw(&d,&200_000);}
#[test]
fn deposit_without_token_balance_fails_and_leaves_pool_untouched() {
    let f = setup();
    let d = Address::generate(&f.env);
    assert!(f.c.try_deposit(&d, &1_000).is_err());
    assert_eq!(f.c.get_stats().pool_balance, 0);
    assert_eq!(f.c.get_deposit(&d), 0);
}
#[test]
fn committed_funds_survive_a_pending_award() {
    let f = setup();
    let env = &f.env;
    let d = donor(&f, 1_000_000);
    let student = Address::generate(env);

    // Donor deposits, admin plans an award and earmarks the funds for it.
    f.c.deposit(&d, &1_000_000);
    f.c.commit_funds(&f.admin, &1_000_000);

    // Donor can no longer withdraw funds that are earmarked for the pending award.
    let result = f.c.try_withdraw(&d, &1_000_000);
    assert_eq!(result, Err(Ok(Error::FundsCommitted)));

    // The planned disbursement still succeeds because the funds were protected.
    f.c.set_recipient_eligibility(&f.admin, &student, &true);
    f.c.disburse(&f.admin, &student, &1_000_000, &String::from_str(env, "award"));
    assert_eq!(f.c.get_stats().pool_balance, 0);
    assert_eq!(f.c.get_stats().committed_balance, 0);
    assert_eq!(f.token.balance(&student), 1_000_000);
    assert_eq!(f.token.balance(&f.c.address), 0);
}
#[test]
fn uncommitted_funds_remain_withdrawable() {
    let f = setup();
    let d = donor(&f, 1_000_000);

    f.c.deposit(&d, &1_000_000);
    f.c.commit_funds(&f.admin, &400_000);

    // Only the committed portion is protected; the rest can still be withdrawn.
    f.c.withdraw(&d, &600_000);
    assert_eq!(f.c.get_stats().pool_balance, 400_000);
    assert_eq!(f.token.balance(&d), 600_000);
    assert_eq!(f.token.balance(&f.c.address), 400_000);
}
#[test]
fn disburse_to_ineligible_recipient_is_rejected() {
    let f = setup();
    let env = &f.env;
    let d = donor(&f, 1_000_000);
    let student = Address::generate(env);
    f.c.deposit(&d, &1_000_000);
    let result = f.c.try_disburse(&f.admin, &student, &500_000, &String::from_str(env, "award"));
    assert_eq!(result, Err(Ok(Error::RecipientNotEligible)));
    assert_eq!(f.c.get_stats().pool_balance, 1_000_000);
    assert_eq!(f.token.balance(&student), 0);
}
#[test]
fn recipient_cap_limits_cumulative_awards() {
    let f = setup();
    let env = &f.env;
    let d = donor(&f, 3_000_000);
    let student = Address::generate(env);
    f.c.deposit(&d, &3_000_000);
    f.c.set_recipient_eligibility(&f.admin, &student, &true);
    f.c.set_recipient_cap(&f.admin, &student, &1_500_000);

    f.c.disburse(&f.admin, &student, &1_000_000, &String::from_str(env, "award-1"));
    assert_eq!(f.c.get_recipient_awards(&student), 1_000_000);

    // Second award would push cumulative total past the cap.
    let result = f.c.try_disburse(&f.admin, &student, &1_000_000, &String::from_str(env, "award-2"));
    assert_eq!(result, Err(Ok(Error::RecipientCapExceeded)));

    // A smaller award that stays within the cap still succeeds.
    f.c.disburse(&f.admin, &student, &500_000, &String::from_str(env, "award-2"));
    assert_eq!(f.c.get_recipient_awards(&student), 1_500_000);
    assert_eq!(f.token.balance(&student), 1_500_000);
}
#[test]
#[should_panic]
fn non_admin_set_eligibility_panics() {
    let f = setup();
    let attacker = Address::generate(&f.env);
    let student = Address::generate(&f.env);
    f.c.set_recipient_eligibility(&attacker, &student, &true);
}

// ── #922 initialize requires admin auth ──────────────────────────────────────

#[test]
fn initialize_without_admin_auth_fails() {
    // No mock_all_auths: a front-runner cannot initialize with an admin they don't control.
    let env = Env::default();
    let token_id = env.register_stellar_asset_contract_v2(Address::generate(&env)).address();
    let id = env.register(ScholarshipFundContract, ());
    let c = ScholarshipFundContractClient::new(&env, &id);
    let attacker_chosen_admin = Address::generate(&env);

    assert!(c.try_initialize(&attacker_chosen_admin, &token_id).is_err());

    // Nothing was stored: the real admin can still initialize.
    let admin = Address::generate(&env);
    env.mock_all_auths();
    c.initialize(&admin, &token_id);
    assert_eq!(env.auths()[0].0, admin);
}
#[test]
fn initialize_twice_is_rejected() {
    let f = setup();
    let other = Address::generate(&f.env);
    assert_eq!(f.c.try_initialize(&other, &f.token.address), Err(Ok(Error::AlreadyInitialized)));
}

// ── #923 awards are shared pro-rata by all donors ────────────────────────────

/// sum(withdrawable) must never exceed the uncommitted pool.
fn assert_solvent(f: &Fund, donors: &[&Address]) {
    let stats = f.c.get_stats();
    let withdrawable: i128 = donors.iter().map(|d| f.c.get_deposit(d)).sum();
    assert!(withdrawable <= stats.pool_balance - stats.committed_balance);
    assert_eq!(f.token.balance(&f.c.address), stats.pool_balance);
}
#[test]
fn disburse_cost_is_shared_by_all_donors() {
    let f = setup();
    let env = &f.env;
    let a = donor(&f, 100);
    let b = donor(&f, 100);
    let student = Address::generate(env);
    f.c.deposit(&a, &100);
    f.c.deposit(&b, &100);
    f.c.set_recipient_eligibility(&f.admin, &student, &true);

    f.c.disburse(&f.admin, &student, &100, &String::from_str(env, "award"));
    assert_eq!(f.c.get_deposit(&a), 50);
    assert_eq!(f.c.get_deposit(&b), 50);
    assert_solvent(&f, &[&a, &b]);

    // A can no longer withdraw a full refund at B's expense.
    assert_eq!(f.c.try_withdraw(&a, &100), Err(Ok(Error::InsufficientFunds)));
    f.c.withdraw(&a, &50);
    f.c.withdraw(&b, &50);
    assert_eq!(f.token.balance(&a), 50);
    assert_eq!(f.token.balance(&b), 50);
    assert_eq!(f.token.balance(&f.c.address), 0);
    assert_solvent(&f, &[&a, &b]);
}
#[test]
fn commitment_is_shared_by_all_donors() {
    let f = setup();
    let a = donor(&f, 300);
    let b = donor(&f, 100);
    f.c.deposit(&a, &300);
    f.c.deposit(&b, &100);
    f.c.commit_funds(&f.admin, &200);

    // Committed funds come out of each donor's stake in proportion to it.
    assert_eq!(f.c.get_deposit(&a), 150);
    assert_eq!(f.c.get_deposit(&b), 50);
    assert_eq!(f.c.try_withdraw(&a, &300), Err(Ok(Error::FundsCommitted)));
    f.c.withdraw(&a, &150);
    assert_eq!(f.c.get_deposit(&b), 50);
    assert_solvent(&f, &[&a, &b]);
}
#[test]
fn later_donor_is_not_charged_for_earlier_awards() {
    let f = setup();
    let env = &f.env;
    let a = donor(&f, 200);
    let b = donor(&f, 100);
    let student = Address::generate(env);
    f.c.deposit(&a, &200);
    f.c.set_recipient_eligibility(&f.admin, &student, &true);
    f.c.disburse(&f.admin, &student, &100, &String::from_str(env, "award"));

    // B joins after the award: their deposit is worth exactly what they put in.
    f.c.deposit(&b, &100);
    assert_eq!(f.c.get_deposit(&a), 100);
    assert_eq!(f.c.get_deposit(&b), 100);
    assert_solvent(&f, &[&a, &b]);
}
#[test]
fn fully_spent_pool_does_not_dilute_new_donors() {
    let f = setup();
    let env = &f.env;
    let a = donor(&f, 100);
    let b = donor(&f, 100);
    let student = Address::generate(env);
    f.c.deposit(&a, &100);
    f.c.set_recipient_eligibility(&f.admin, &student, &true);
    f.c.disburse(&f.admin, &student, &100, &String::from_str(env, "award"));
    assert_eq!(f.c.get_deposit(&a), 0);

    // A's now-worthless shares must not claim part of B's deposit.
    f.c.deposit(&b, &100);
    assert_eq!(f.c.get_shares(&a), 0);
    assert_eq!(f.c.get_deposit(&a), 0);
    assert_eq!(f.c.get_deposit(&b), 100);
    assert_eq!(f.c.try_withdraw(&a, &1), Err(Ok(Error::InsufficientFunds)));
    f.c.withdraw(&b, &100);
    assert_eq!(f.token.balance(&b), 100);
    assert_solvent(&f, &[&a, &b]);
}
#[test]
fn rounding_never_lets_withdrawals_exceed_the_pool() {
    let f = setup();
    let env = &f.env;
    let a = donor(&f, 10);
    let b = donor(&f, 10);
    let c = donor(&f, 10);
    let student = Address::generate(env);
    f.c.deposit(&a, &10);
    f.c.deposit(&b, &10);
    f.c.deposit(&c, &10);
    f.c.set_recipient_eligibility(&f.admin, &student, &true);
    f.c.disburse(&f.admin, &student, &1, &String::from_str(env, "award"));
    assert_solvent(&f, &[&a, &b, &c]);

    for d in [&a, &b, &c] {
        let owed = f.c.get_deposit(d);
        f.c.withdraw(d, &owed);
        assert_solvent(&f, &[&a, &b, &c]);
    }
    assert!(f.token.balance(&f.c.address) >= 0);
}
