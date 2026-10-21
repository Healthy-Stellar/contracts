#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, token, Address, Env};

fn setup(env: &Env) -> (LiquidityPoolContractClient, Address, Address, Address) {
    let admin = Address::generate(env);
    let token_a = env.register_stellar_asset_contract(admin.clone());
    let token_b = env.register_stellar_asset_contract(admin.clone());
    let contract_id = env.register_contract(None, LiquidityPoolContract);
    let client = LiquidityPoolContractClient::new(env, &contract_id);
    client.initialize(&admin, &token_a, &token_b);
    (client, admin, token_a, token_b)
}

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _token_a, _token_b) = setup(&env);
    let stats = client.get_stats();
    assert_eq!(stats.reserve_a, 0);
    assert_eq!(stats.reserve_b, 0);
    assert_eq!(stats.total_shares, 0);
}

#[test]
fn test_add_liquidity() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_a, token_b) = setup(&env);
    let provider = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&provider, &10_000);
    token::StellarAssetClient::new(&env, &token_b).mint(&provider, &10_000);
    
    let deadline = env.ledger().timestamp() + 1000;
    // sqrt(10_000 * 10_000) = 10_000; minus MINIMUM_LIQUIDITY (1000) = 9_000 shares to provider
    let shares = client.add_liquidity(&provider, &10_000, &10_000, &9_000, &deadline);
    assert_eq!(shares, 9_000);
    
    // Pool total shares is 10_000 (1000 permanently locked in pool)
    let stats = client.get_stats();
    assert_eq!(stats.total_shares, 10_000);
    assert_eq!(client.shares_of(&provider), 9_000);
}

#[test]
fn test_add_liquidity_slippage_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_a, token_b) = setup(&env);
    let provider = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&provider, &10_000);
    token::StellarAssetClient::new(&env, &token_b).mint(&provider, &10_000);
    
    let deadline = env.ledger().timestamp() + 1000;
    // User expects at least 9500 shares, but only receives 9000
    let res = client.try_add_liquidity(&provider, &10_000, &10_000, &9_500, &deadline);
    assert_eq!(res, Err(Ok(Error::SlippageExceeded)));
}

#[test]
fn test_add_liquidity_deadline_exceeded() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(2000);
    let (client, _admin, token_a, token_b) = setup(&env);
    let provider = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&provider, &10_000);
    token::StellarAssetClient::new(&env, &token_b).mint(&provider, &10_000);
    
    let expired_deadline = 1500;
    let res = client.try_add_liquidity(&provider, &10_000, &10_000, &1, &expired_deadline);
    assert_eq!(res, Err(Ok(Error::DeadlineExceeded)));
}

#[test]
fn test_first_deposit_minimum_liquidity_locked() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_a, token_b) = setup(&env);
    let provider = Address::generate(&env);
    // Deposit small amount <= MINIMUM_LIQUIDITY (e.g. sqrt(1000 * 1000) = 1000 <= 1000)
    token::StellarAssetClient::new(&env, &token_a).mint(&provider, &1000);
    token::StellarAssetClient::new(&env, &token_b).mint(&provider, &1000);
    let deadline = env.ledger().timestamp() + 1000;
    
    let res = client.try_add_liquidity(&provider, &1000, &1000, &1, &deadline);
    assert_eq!(res, Err(Ok(Error::InsufficientLiquidity)));
}

#[test]
fn test_swap() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_a, token_b) = setup(&env);
    let provider = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&provider, &100_000);
    token::StellarAssetClient::new(&env, &token_b).mint(&provider, &100_000);
    let deadline = env.ledger().timestamp() + 1000;
    client.add_liquidity(&provider, &100_000, &100_000, &1, &deadline);

    let trader = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&trader, &10_000);
    let out = client.swap(&trader, &10_000, &1, &true, &deadline);
    assert!(out > 0);
}

#[test]
fn test_swap_slippage_protection() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_a, token_b) = setup(&env);
    let provider = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&provider, &100_000);
    token::StellarAssetClient::new(&env, &token_b).mint(&provider, &100_000);
    let deadline = env.ledger().timestamp() + 1000;
    client.add_liquidity(&provider, &100_000, &100_000, &1, &deadline);

    let trader = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&trader, &10_000);
    let result = client.try_swap(&trader, &10_000, &999_999, &true, &deadline);
    assert_eq!(result, Err(Ok(Error::SlippageExceeded)));
}

#[test]
fn test_swap_deadline_exceeded() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(2000);
    let (client, _admin, token_a, token_b) = setup(&env);
    let provider = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&provider, &100_000);
    token::StellarAssetClient::new(&env, &token_b).mint(&provider, &100_000);
    client.add_liquidity(&provider, &100_000, &100_000, &1, &3000);

    let trader = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&trader, &10_000);
    let result = client.try_swap(&trader, &10_000, &1, &true, &1500);
    assert_eq!(result, Err(Ok(Error::DeadlineExceeded)));
}

#[test]
fn test_remove_liquidity_with_slippage_and_deadline() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, token_a, token_b) = setup(&env);
    let provider = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_a).mint(&provider, &50_000);
    token::StellarAssetClient::new(&env, &token_b).mint(&provider, &50_000);
    let deadline = env.ledger().timestamp() + 1000;
    let shares = client.add_liquidity(&provider, &50_000, &50_000, &1, &deadline);

    // Normal remove
    let (out_a, out_b) = client.remove_liquidity(&provider, &1000, &1, &1, &deadline);
    assert!(out_a > 0 && out_b > 0);

    // Slippage rejected
    let res_slip = client.try_remove_liquidity(&provider, &1000, &999_999, &1, &deadline);
    assert_eq!(res_slip, Err(Ok(Error::SlippageExceeded)));

    // Deadline exceeded: advance ledger timestamp past deadline
    env.ledger().set_timestamp(2000);
    let res_dead = client.try_remove_liquidity(&provider, &1000, &1, &1, &1500);
    assert_eq!(res_dead, Err(Ok(Error::DeadlineExceeded)));
}
