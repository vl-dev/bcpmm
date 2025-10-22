use anchor_lang::prelude::*;

pub const CENTRAL_STATE_SEED: &[u8] = b"central_state";
pub const BCPMM_POOL_SEED: &[u8] = b"bcpmm_pool";
pub const VIRTUAL_TOKEN_ACCOUNT_SEED: &[u8] = b"virtual_token_account";
pub const USER_BURN_ALLOWANCE_SEED: &[u8] = b"user_burn_allowance";

#[account]
#[derive(Default, InitSpace)]
pub struct CentralState {
    pub admin: Pubkey,
    pub daily_burn_allowance: u16,
    pub creator_daily_burn_allowance: u16,
    pub user_burn_bp: u16, 
    pub creator_burn_bp: u16,
    pub burn_reset_time_of_day_seconds: u32, // Seconds from midnight
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
            daily_burn_allowance,
            creator_daily_burn_allowance,
            user_burn_bp,
            creator_burn_bp,
            burn_reset_time_of_day_seconds,
        }
    }

    pub fn is_after_burn_reset(&self, current_time: i64) -> bool {
        let seconds_since_midnight = (current_time % 86400) as u32;
        seconds_since_midnight >= self.burn_reset_time_of_day_seconds
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

    /// B mint address
    pub b_mint: Pubkey,
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