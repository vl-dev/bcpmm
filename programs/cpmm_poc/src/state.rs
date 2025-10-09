use anchor_lang::prelude::*;

#[account]
#[derive(Default, InitSpace)]
pub struct CentralState {
    pub mint_counter: u64,
    pub acs_mint: Pubkey,
    pub acs_mint_decimals: u8,
}

#[account]
#[derive(Default, InitSpace)]
pub struct CpmmPool {
    pub mint_index: u64,
    pub micro_acs_reserve: u64,
    pub ct_reserve: u64,
    pub virtual_acs_reserve: u64,
}

#[account]
#[derive(Default, InitSpace)]
pub struct CtAccount {
    pub pool: Pubkey,
    pub balance: u64,
}
