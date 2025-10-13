use anchor_lang::prelude::*;

pub const BCPMM_POOL_SEED: &[u8] = b"bcpmm_pool";
pub const VIRTUAL_TOKEN_ACCOUNT_SEED: &[u8] = b"virtual_token_account";
// A is the real SPL token
// B is the virtual token
#[account]
#[derive(Default, InitSpace)]
pub struct BcpmmPool {
    pub a_mint: Pubkey,
    pub a_reserve: u64,
    pub a_virtual_reserve: u64,
    pub b_mint: Pubkey,
    pub b_reserve: u64,
}

#[account]
#[derive(Default, InitSpace)]
pub struct VirtualTokenAccount {
    pub pool: Pubkey,
    pub balance: u64,
}
