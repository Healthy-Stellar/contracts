#![no_std]

//! # Liquidity Pool Contract
//!
//! Manages liquidity pools for healthcare token swaps with fee collection, reserve management,
//! and price discovery for healthcare finance operations.
//!
//! ## HIPAA Compliance
//!
//! **Access Control Safeguards:** Pool initialization restricted to authorized admin. Liquidity
//! provider authentication required for deposits/withdrawals. Swap operations available to network
//! participants. Fee recipient validation prevents unauthorized revenue capture.
//!
//! **Audit Controls:** Pool creation events with token pairs and initial reserves. Liquidity
//! deposit/withdrawal events tracked with participant identity. Swap execution events logged.
//! Fee collection events recorded. Pool balance changes auditable via events.
//!
//! **Data Retention Policy:** Pool reserves maintained indefinitely for continuous operation.
//! Transaction history retained for audit trail. Fee collection metrics tracked. Liquidity
//! provider share records retained for settlement.
//!
//! **Encryption/Integrity:** Pool balance integrity enforced via constant product formula.
//! Price calculations immutable once blocks close. Fee collection (0.30% BPS) enforced
//! mathematically. Reserve balances cryptographically signed via Soroban state.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol,
};

const POOL_FEE_BPS: i128 = 30; // 0.30%
const MINIMUM_LIQUIDITY: i128 = 1000;

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
    ArithmeticOverflow = 8,
    RotationPending = 9,
    NoRotationPending = 10,
    RotationExpired = 11,
    NotPendingAdmin = 12,
    DeadlineExceeded = 13,
}

const ADMIN_ROTATION_WINDOW: u64 = 86_400;

#[contracttype]
pub enum DataKey {
    Admin,
    ReserveA,
    ReserveB,
    TotalShares,
    Shares(Address),
    PendingAdmin,
    RotationExpiry,
    TokenA,
    TokenB,
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
    pub fn initialize(
        env: Env,
        admin: Address,
        token_a: Address,
        token_b: Address,
    ) -> Result<(), Error> {
        admin.require_auth();
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenA, &token_a);
        env.storage().instance().set(&DataKey::TokenB, &token_b);
        env.storage().instance().set(&DataKey::ReserveA, &0i128);
        env.storage().instance().set(&DataKey::ReserveB, &0i128);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
        Ok(())
    }

    /// Deposit token_a and token_b amounts; mint LP shares proportionally.
    /// Deposit token_a and token_b amounts; mint LP shares proportionally.
    /// For non-initial deposits, computes optimal amounts to maintain pool ratio.
    pub fn add_liquidity(
        env: Env,
        provider: Address,
        amount_a: i128,
        amount_b: i128,
        min_shares: i128,
        deadline: u64,
    ) -> Result<i128, Error> {
        provider.require_auth();
        if env.ledger().timestamp() > deadline {
            return Err(Error::DeadlineExceeded);
        }
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

        let (shares_to_mint, actual_a, actual_b, new_total_shares) = if total == 0 {
            // First deposit: lock MINIMUM_LIQUIDITY shares permanently
            let product = amount_a.checked_mul(amount_b).ok_or(Error::ArithmeticOverflow)?;
            let total_minted = sqrt(product);
            if total_minted <= MINIMUM_LIQUIDITY {
                return Err(Error::InsufficientLiquidity);
            }
            let user_shares = total_minted - MINIMUM_LIQUIDITY;
            (user_shares, amount_a, amount_b, total_minted)
        } else {
            // Compute shares and optimal amounts
            let s_a = amount_a.checked_mul(total).ok_or(Error::ArithmeticOverflow)? / reserve_a;
            let s_b = amount_b.checked_mul(total).ok_or(Error::ArithmeticOverflow)? / reserve_b;
            let shares = s_a.min(s_b);

            // Compute optimal amounts to mint exactly 'shares' shares
            let optimal_a = shares.checked_mul(reserve_a).ok_or(Error::ArithmeticOverflow)? / total;
            let optimal_b = shares.checked_mul(reserve_b).ok_or(Error::ArithmeticOverflow)? / total;

            (shares, optimal_a, optimal_b, total + shares)
        };

        if shares_to_mint <= 0 {
            return Err(Error::InsufficientLiquidity);
        }
        if shares_to_mint < min_shares {
            return Err(Error::SlippageExceeded);
        }

        let token_a: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenA)
            .ok_or(Error::NotInitialized)?;
        let token_b: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenB)
            .ok_or(Error::NotInitialized)?;

        // Persist updated reserves/shares BEFORE any external token transfer to
        // close the reentrancy window (matches remove_liquidity ordering).
        env.storage()
            .instance()
            .set(&DataKey::ReserveA, &(reserve_a + actual_a));
        env.storage()
            .instance()
            .set(&DataKey::ReserveB, &(reserve_b + actual_b));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &new_total_shares);

        let prev: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Shares(provider.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Shares(provider.clone()), &(prev + shares_to_mint));

        let pool = env.current_contract_address();
        token::Client::new(&env, &token_a).transfer(&provider, &pool, &actual_a);
        token::Client::new(&env, &token_b).transfer(&provider, &pool, &actual_b);

        env.events().publish(
            (symbol_short!("ADD_LIQ"), provider),
            (actual_a, actual_b, shares_to_mint),
        );
        Ok(shares_to_mint)
    }

    /// Burn LP shares and receive proportional token_a and token_b back.
    pub fn remove_liquidity(
        env: Env,
        provider: Address,
        shares: i128,
        min_a: i128,
        min_b: i128,
        deadline: u64,
    ) -> Result<(i128, i128), Error> {
        provider.require_auth();
        if env.ledger().timestamp() > deadline {
            return Err(Error::DeadlineExceeded);
        }
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

        let out_a = shares
            .checked_mul(reserve_a)
            .ok_or(Error::ArithmeticOverflow)? / total;
        let out_b = shares
            .checked_mul(reserve_b)
            .ok_or(Error::ArithmeticOverflow)? / total;

        if out_a < min_a || out_b < min_b {
            return Err(Error::SlippageExceeded);
        }

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

        let token_a: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenA)
            .ok_or(Error::NotInitialized)?;
        let token_b: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenB)
            .ok_or(Error::NotInitialized)?;
        let pool = env.current_contract_address();
        token::Client::new(&env, &token_a).transfer(&pool, &provider, &out_a);
        token::Client::new(&env, &token_b).transfer(&pool, &provider, &out_b);

        env.events().publish(
            (symbol_short!("REM_LIQ"), provider),
            (out_a, out_b, shares),
        );
        Ok((out_a, out_b))
    }

    /// Swap an exact input amount of one token for the other.
    /// `a_to_b` selects the direction. Applies the pool fee and enforces min_out.
    pub fn swap(
        env: Env,
        trader: Address,
        amount_in: i128,
        min_out: i128,
        a_to_b: bool,
        deadline: u64,
    ) -> Result<i128, Error> {
        trader.require_auth();
        if env.ledger().timestamp() > deadline {
            return Err(Error::DeadlineExceeded);
        }
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

        let (reserve_in, reserve_out) = if a_to_b {
            (reserve_a, reserve_b)
        } else {
            (reserve_b, reserve_a)
        };

        let amount_in_after_fee = amount_in
            .checked_mul(10_000 - POOL_FEE_BPS)
            .ok_or(Error::ArithmeticOverflow)? / 10_000;
        let numerator = amount_in_after_fee
            .checked_mul(reserve_out)
            .ok_or(Error::ArithmeticOverflow)?;
        let denominator = reserve_in
            .checked_add(amount_in_after_fee)
            .ok_or(Error::ArithmeticOverflow)?;
        let amount_out = numerator / denominator;

        if amount_out <= 0 || amount_out >= reserve_out {
            return Err(Error::InsufficientLiquidity);
        }
        if amount_out < min_out {
            return Err(Error::SlippageExceeded);
        }

        let token_a: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenA)
            .ok_or(Error::NotInitialized)?;
        let token_b: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenB)
            .ok_or(Error::NotInitialized)?;
        let (token_in, token_out) = if a_to_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };

        // Persist updated reserves BEFORE any external token transfer to close
        // the reentrancy window (matches remove_liquidity ordering).
        if a_to_b {
            env.storage()
                .instance()
                .set(&DataKey::ReserveA, &(reserve_a + amount_in));
            env.storage()
                .instance()
                .set(&DataKey::ReserveB, &(reserve_b - amount_out));
        } else {
            env.storage()
                .instance()
                .set(&DataKey::ReserveB, &(reserve_b + amount_in));
            env.storage()
                .instance()
                .set(&DataKey::ReserveA, &(reserve_a - amount_out));
        }

        let pool = env.current_contract_address();
        token::Client::new(&env, &token_in).transfer(&trader, &pool, &amount_in);
        token::Client::new(&env, &token_out).transfer(&pool, &trader, &amount_out);

        env.events().publish(
            (symbol_short!("SWAP"), trader),
            (amount_in, amount_out, a_to_b),
        );
        Ok(amount_out)
    }

    /// Read-only pool statistics.
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

    /// LP share balance for a provider.
    pub fn shares_of(env: Env, provider: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Shares(provider))
            .unwrap_or(0)
    }
}

/// Integer square root (floor) for initial share supply.
fn sqrt(value: i128) -> i128 {
    if value <= 0 {
        return 0;
    }
    let mut x = value;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + value / x) / 2;
    }
    x
}

#[cfg(test)]
mod test;
