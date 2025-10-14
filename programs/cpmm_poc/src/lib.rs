use anchor_lang::prelude::*;

mod errors;
mod processor;
mod state;

use processor::*;

declare_id!("2rpy7rFzUMqPEbMP8pQGVS1tZfGeLsrsNcnzQcdAk2fz");

#[program]
pub mod cpmm_poc {
    use super::*;

    pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {
        processor::create_pool(ctx, args)
    }

    pub fn initialize_virtual_token_account(
        ctx: Context<InitializeVirtualTokenAccount>,
    ) -> Result<()> {
        processor::initialize_virtual_token_account(ctx)
    }

    pub fn buy_virtual_token(
        ctx: Context<BuyVirtualToken>,
        args: BuyVirtualTokenArgs,
    ) -> Result<()> {
        processor::buy_virtual_token(ctx, args)
    }

    pub fn sell_virtual_token(
        ctx: Context<SellVirtualToken>,
        args: SellVirtualTokenArgs,
    ) -> Result<()> {
        processor::sell_virtual_token(ctx, args)
    }

    pub fn burn_virtual_token(
        ctx: Context<BurnVirtualToken>,
        args: BurnVirtualTokenArgs,
    ) -> Result<()> {
        processor::burn_virtual_token(ctx, args)
    }

    pub fn close_virtual_token_account(ctx: Context<CloseVirtualTokenAccount>) -> Result<()> {
        processor::close_virtual_token_account(ctx)
    }
}
