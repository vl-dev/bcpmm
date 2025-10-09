use anchor_lang::prelude::*;

mod processor;
mod state;

use processor::*;

declare_id!("2rpy7rFzUMqPEbMP8pQGVS1tZfGeLsrsNcnzQcdAk2fz");

#[program]
pub mod cpmm_poc {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        processor::initialize(ctx)
    }

    pub fn create_pool(
        ctx: Context<CreatePool>,
        initial_supply: u64,
        virtual_base_reserve: u64,
    ) -> Result<()> {
        processor::create_pool(ctx, initial_supply, virtual_base_reserve)
    }

    pub fn initialize_ct_account(ctx: Context<InitializeCtAccount>) -> Result<()> {
        processor::initialize_ct_account(ctx)
    }

    pub fn buy_token(ctx: Context<BuyToken>, amount_micro_acs: u64) -> Result<()> {
        processor::buy_token(ctx, amount_micro_acs)
    }

    pub fn sell_token(ctx: Context<SellToken>, amount_ct: u64) -> Result<()> {
        processor::sell_token(ctx, amount_ct)
    }
}
