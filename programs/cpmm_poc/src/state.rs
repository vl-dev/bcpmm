use crate::errors::BcpmmError;
use anchor_lang::prelude::*;

pub const CENTRAL_STATE_SEED: &[u8] = b"central_state";
pub const BCPMM_POOL_SEED: &[u8] = b"bcpmm_pool";
pub const VIRTUAL_TOKEN_ACCOUNT_SEED: &[u8] = b"virtual_token_account";
pub const USER_BURN_ALLOWANCE_SEED: &[u8] = b"user_burn_allowance";

pub const DEFAULT_B_MINT_DECIMALS: u8 = 6;
pub const DEFAULT_B_MINT_RESERVE: u64 = 1_000_000_000 * 10u64.pow(DEFAULT_B_MINT_DECIMALS as u32);

#[account]
#[derive(Default, InitSpace)]
pub struct CentralState {
    pub admin: Pubkey,
    pub b_mint_index: u64,
    pub daily_burn_allowance: u16,
    pub creator_daily_burn_allowance: u16,
    pub user_burn_bp: u16, 
    pub creator_burn_bp: u16,
    pub burn_reset_time_of_day_seconds: u32, // Seconds from midnight
}

/// Check if given time is after today's burn reset timestamp (for testing with mock time).
pub fn is_after_burn_reset_with_time( time_to_check: i64, current_time: i64, reset_time_of_day_seconds: u32) -> bool {
    let todays_midnight = current_time - current_time.rem_euclid(86400);
    let todays_reset_ts = todays_midnight + reset_time_of_day_seconds as i64;
    time_to_check >= todays_reset_ts
}

impl CentralState {
pub fn new(
    admin: Pubkey,
    daily_burn_allowance: u16,
    creator_daily_burn_allowance: u16,
    user_burn_bp: u16,
    creator_burn_bp: u16,
    burn_reset_time_of_day_seconds: u32,
    ) -> Self {
        Self {
            admin,
            b_mint_index: 0,
            daily_burn_allowance,
            creator_daily_burn_allowance,
            user_burn_bp,
            creator_burn_bp,
            burn_reset_time_of_day_seconds,
        }
    }

    /// Check if given time is after today's burn reset timestamp.
    pub fn is_after_burn_reset(&self, time_to_check: i64) -> Result<bool> {
        let now = Clock::get()?.unix_timestamp;
        Ok(is_after_burn_reset_with_time(time_to_check, now, self.burn_reset_time_of_day_seconds))
    }

}

// A is the real SPL token
// B is the virtual token
#[account]
#[derive(Default, InitSpace)]
pub struct BcpmmPool {
    /// Pool creator address
    pub creator: Pubkey,

    /// A mint address
    pub a_mint: Pubkey,
    /// A reserve including decimals
    pub a_reserve: u64,
    /// A virtual reserve including decimals
    pub a_virtual_reserve: u64,
    // A remaining topup to compensate for the virtual reserve reduction happening on burn
    pub a_remaining_topup: u64,

    /// B mint is virtual and denoted by index
    pub b_mint_index: u64,
    /// B mint decimals
    pub b_mint_decimals: u8,
    /// B reserve including decimals
    pub b_reserve: u64,

    /// Creator fees balance denominated in Mint A including decimals
    pub creator_fees_balance: u64,
    /// Buyback fees balance denominated in Mint A including decimals
    pub buyback_fees_balance: u64,

    /// Creator fee basis points
    pub creator_fee_basis_points: u16,
    /// Buyback fee basis points
    pub buyback_fee_basis_points: u16,

    /// Burn allowance for the pool
    pub burns_today: u16,
    pub last_burn_timestamp: i64,
}

impl BcpmmPool {
    pub fn try_new(
        creator: Pubkey,
        a_mint: Pubkey,
        a_virtual_reserve: u64,
        b_mint_index: u64,
        creator_fee_basis_points: u16,
        buyback_fee_basis_points: u16,
    ) -> Result<Self> {
        require!(a_virtual_reserve > 0, BcpmmError::InvalidVirtualReserve);
        require!(
            buyback_fee_basis_points > 0,
            BcpmmError::InvalidBuybackFeeBasisPoints
        );

        Ok(Self {
            creator,
            a_mint,
            a_reserve: 0,
            a_virtual_reserve,
            a_remaining_topup: 0,
            b_mint_index,
            b_mint_decimals: DEFAULT_B_MINT_DECIMALS,
            b_reserve: DEFAULT_B_MINT_RESERVE,
            creator_fees_balance: 0,
            buyback_fees_balance: 0,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            burns_today: 0,
            last_burn_timestamp: 0,
        })
    }
}

#[account]
#[derive(Default, InitSpace)]
pub struct VirtualTokenAccount {
    /// Pool address
    pub pool: Pubkey,
    /// Owner address
    pub owner: Pubkey,
    /// Balance of Mint B including decimals
    pub balance: u64,
    /// All fees paid when buying and selling tokens to this account. Denominated in Mint A including decimals
    pub fees_paid: u64,
}

#[account]
#[derive(Default, InitSpace)]
pub struct UserBurnAllowance {
    pub user: Pubkey,
    pub payer: Pubkey, // Wallet that receives funds when this account is closed
    pub burns_today: u16,

    pub last_burn_timestamp: i64,
}

impl UserBurnAllowance {
    pub fn new(
        user: Pubkey,
        payer: Pubkey,
    ) -> Self {
        Self { user, payer, burns_today: 0, last_burn_timestamp: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_after_burn_reset_with_time() {

        let midnight = 1761177600;
        let current_time = midnight + 1;
        
        let time_before_reset =  1761177660; // Just after midnight
        assert!(!is_after_burn_reset_with_time(time_before_reset, current_time, 43200));
        
        let yesterday_night = 1761166800;
        assert!(!is_after_burn_reset_with_time(yesterday_night, current_time, 43200));

        
        let time_after_reset_same_day  = 1761224400;
        assert!(is_after_burn_reset_with_time(time_after_reset_same_day, current_time, 43200));

        let next_day  = 1761264000;
        assert!(is_after_burn_reset_with_time(next_day, current_time, 43200));
        
    }

}