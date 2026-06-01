#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

const POOL_FEE_BPS: i128 = 30; // 0.30%

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    ZeroAmount = 3,
    InsufficientLiquidity = 4,
    SlippageExceeded = 5,
    InsufficientShares = 6,
    Unauthorized = 7,
}

#[contracttype]
pub enum DataKey {
    Admin,
    ReserveA,
    ReserveB,
    TotalShares,
    Shares(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolStats {
    pub reserve_a: i128,
    pub reserve_b: i128,
    pub total_shares: i128,
}

#[contract]
pub struct LiquidityPoolContract;

#[contractimpl]
impl LiquidityPoolContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::ReserveA, &0i128);
        env.storage().instance().set(&DataKey::ReserveB, &0i128);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
        Ok(())
    }

    /// Deposit token_a and token_b amounts; mint LP shares proportionally.
    pub fn add_liquidity(
        env: Env,
        provider: Address,
        amount_a: i128,
        amount_b: i128,
    ) -> Result<i128, Error> {
        provider.require_auth();
        if amount_a <= 0 || amount_b <= 0 {
            return Err(Error::ZeroAmount);
        }
        let reserve_a: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveA)
            .unwrap_or(0);
        let reserve_b: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveB)
            .unwrap_or(0);
        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0);

        let shares = if total == 0 {
            // First deposit: geometric mean as initial share supply
            sqrt(amount_a * amount_b)
        } else {
            // Pro-rata: min of both ratios to prevent dilution
            let s_a = amount_a * total / reserve_a;
            let s_b = amount_b * total / reserve_b;
            s_a.min(s_b)
        };

        if shares <= 0 {
            return Err(Error::InsufficientLiquidity);
        }

        env.storage()
            .instance()
            .set(&DataKey::ReserveA, &(reserve_a + amount_a));
        env.storage()
            .instance()
            .set(&DataKey::ReserveB, &(reserve_b + amount_b));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total + shares));

        let prev: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Shares(provider.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Shares(provider.clone()), &(prev + shares));

        env.events().publish(
            (symbol_short!("ADD_LIQ"), provider),
            (amount_a, amount_b, shares),
        );
        Ok(shares)
    }

    /// Burn LP shares and receive proportional token_a and token_b back.
    pub fn remove_liquidity(
        env: Env,
        provider: Address,
        shares: i128,
    ) -> Result<(i128, i128), Error> {
        provider.require_auth();
        if shares <= 0 {
            return Err(Error::ZeroAmount);
        }
        let held: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Shares(provider.clone()))
            .unwrap_or(0);
        if held < shares {
            return Err(Error::InsufficientShares);
        }
        let reserve_a: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveA)
            .unwrap_or(0);
        let reserve_b: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveB)
            .unwrap_or(0);
        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0);

        let out_a = shares * reserve_a / total;
        let out_b = shares * reserve_b / total;

        env.storage()
            .instance()
            .set(&DataKey::ReserveA, &(reserve_a - out_a));
        env.storage()
            .instance()
            .set(&DataKey::ReserveB, &(reserve_b - out_b));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total - shares));
        env.storage()
            .persistent()
            .set(&DataKey::Shares(provider.clone()), &(held - shares));

        env.events()
            .publish((symbol_short!("REM_LIQ"), provider), (out_a, out_b, shares));
        Ok((out_a, out_b))
    }

    /// Swap amount_in of token A for token B.
    /// min_out: minimum acceptable output (slippage protection).
    pub fn swap(env: Env, trader: Address, amount_in: i128, min_out: i128) -> Result<i128, Error> {
        trader.require_auth();
        if amount_in <= 0 {
            return Err(Error::ZeroAmount);
        }
        let reserve_a: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveA)
            .unwrap_or(0);
        let reserve_b: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReserveB)
            .unwrap_or(0);
        if reserve_a <= 0 || reserve_b <= 0 {
            return Err(Error::InsufficientLiquidity);
        }

        // Constant-product AMM with fee: (x + dx*(1-fee)) * (y - dy) = x * y
        let amount_in_with_fee = amount_in * (10_000 - POOL_FEE_BPS) / 10_000;
        let amount_out = reserve_b * amount_in_with_fee / (reserve_a + amount_in_with_fee);

        if amount_out < min_out {
            return Err(Error::SlippageExceeded);
        }

        env.storage()
            .instance()
            .set(&DataKey::ReserveA, &(reserve_a + amount_in));
        env.storage()
            .instance()
            .set(&DataKey::ReserveB, &(reserve_b - amount_out));

        env.events()
            .publish((symbol_short!("SWAP"), trader), (amount_in, amount_out));
        Ok(amount_out)
    }

    pub fn get_stats(env: Env) -> PoolStats {
        PoolStats {
            reserve_a: env
                .storage()
                .instance()
                .get(&DataKey::ReserveA)
                .unwrap_or(0),
            reserve_b: env
                .storage()
                .instance()
                .get(&DataKey::ReserveB)
                .unwrap_or(0),
            total_shares: env
                .storage()
                .instance()
                .get(&DataKey::TotalShares)
                .unwrap_or(0),
        }
    }

    pub fn get_shares(env: Env, provider: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Shares(provider))
            .unwrap_or(0)
    }
}

fn sqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod test;
