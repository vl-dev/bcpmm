use crate::errors::BcpmmError;
use anchor_lang::prelude::*;

pub const CENTRAL_STATE_SEED: &[u8] = b"central_state";
pub const BCPMM_POOL_SEED: &[u8] = b"bcpmm_pool";
pub const VIRTUAL_TOKEN_ACCOUNT_SEED: &[u8] = b"virtual_token_account";

pub const DEFAULT_B_MINT_DECIMALS: u8 = 6;
pub const DEFAULT_B_MINT_RESERVE: u64 = 1_000_000_000 * 10u64.pow(DEFAULT_B_MINT_DECIMALS as u32);

#[account]
#[derive(Default, InitSpace)]
pub struct CentralState {
    pub admin: Pubkey,
    pub b_mint_index: u64,
    pub daily_burn_allowance: u64,
    pub creator_daily_burn_allowance: u64,
    pub user_burn_bp: u16,    // todo change some micro units
    pub creator_burn_bp: u16, // todo change some micro units
    pub burn_reset_time: u64,
}

impl CentralState {
    pub fn new(
        admin: Pubkey,
        daily_burn_allowance: u64,
        creator_daily_burn_allowance: u64,
        user_burn_bp: u16,
        creator_burn_bp: u16,
        burn_reset_time: u64,
    ) -> Self {
        Self {
            admin,
            b_mint_index: 0,
            daily_burn_allowance,
            creator_daily_burn_allowance,
            user_burn_bp,
            creator_burn_bp,
            burn_reset_time,
        }
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
    pub last_burn_timestamp: u64,
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
    pub allowance: u64, // todo change some micro units
}
