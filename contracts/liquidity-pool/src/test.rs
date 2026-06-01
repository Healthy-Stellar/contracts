#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, LiquidityPoolContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquidityPoolContract, ());
    let client = LiquidityPoolContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn add_liquidity_initial_deposit() {
    let (_, client, _) = setup();
    let provider = Address::generate(&client.env);
    let shares = client.add_liquidity(&provider, &1_000_000, &1_000_000);
    assert!(shares > 0);
    let stats = client.get_stats();
    assert_eq!(stats.reserve_a, 1_000_000);
    assert_eq!(stats.reserve_b, 1_000_000);
}

#[test]
fn remove_liquidity_returns_correct_amounts() {
    let (_, client, _) = setup();
    let provider = Address::generate(&client.env);
    let shares = client.add_liquidity(&provider, &2_000_000, &2_000_000);
    let (out_a, out_b) = client.remove_liquidity(&provider, &shares);
    assert_eq!(out_a, 2_000_000);
    assert_eq!(out_b, 2_000_000);
}

#[test]
fn swap_produces_output_and_updates_reserves() {
    let (_, client, _) = setup();
    let provider = Address::generate(&client.env);
    let trader = Address::generate(&client.env);
    client.add_liquidity(&provider, &1_000_000, &1_000_000);
    let out = client.swap(&trader, &10_000, &1);
    assert!(out > 0);
    let stats = client.get_stats();
    assert_eq!(stats.reserve_a, 1_010_000);
    assert!(stats.reserve_b < 1_000_000);
}

#[test]
#[should_panic]
fn swap_slippage_protection_rejects_bad_trade() {
    let (_, client, _) = setup();
    let provider = Address::generate(&client.env);
    let trader = Address::generate(&client.env);
    client.add_liquidity(&provider, &1_000_000, &1_000_000);
    client.swap(&trader, &10_000, &999_999); // unreachable min_out
}

#[test]
fn get_shares_tracks_provider_balance() {
    let (_, client, _) = setup();
    let provider = Address::generate(&client.env);
    let shares = client.add_liquidity(&provider, &500_000, &500_000);
    assert_eq!(client.get_shares(&provider), shares);
}
