#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;

fn setup() -> (Env, ZkEligibilityVerifierClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let zk_contract = Address::generate(&env);
    let contract_id = env.register(ZkEligibilityVerifier, (&admin, &zk_contract));
    let client = ZkEligibilityVerifierClient::new(&env, &contract_id);
    (env, client, admin, zk_contract)
}

#[test]
fn test_constructor_stores_admin_and_zk_contract() {
    let (env, client, admin, zk_contract) = setup();

    env.as_contract(&client.address, || {
        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap();
        let stored_zk: Address = env
            .storage()
            .persistent()
            .get(&DataKey::ZkEligibilityContract)
            .unwrap();

        assert_eq!(stored_admin, admin);
        assert_eq!(stored_zk, zk_contract);
    });
}

#[test]
fn test_set_zk_eligibility_contract_admin_success() {
    let (env, client, admin, _zk_contract) = setup();
    let new_zk_contract = Address::generate(&env);

    client.set_zk_eligibility_contract(&admin, &new_zk_contract);
}

#[test]
fn test_set_zk_eligibility_contract_non_admin_rejected() {
    let (env, client, _admin, _zk_contract) = setup();
    let non_admin = Address::generate(&env);
    let new_zk_contract = Address::generate(&env);

    let result = client.try_set_zk_eligibility_contract(&non_admin, &new_zk_contract);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}
