use anchor_lang::prelude::*;

pub const BCPMM_POOL_SEED: &[u8] = b"bcpmm_pool";
pub const VIRTUAL_TOKEN_ACCOUNT_SEED: &[u8] = b"virtual_token_account";

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
