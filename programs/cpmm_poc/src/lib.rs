use anchor_lang::prelude::*;

mod errors;
mod helpers;
mod instructions;
mod state;

#[cfg(test)]
mod test_utils;

use instructions::*;

declare_id!("2rpy7rFzUMqPEbMP8pQGVS1tZfGeLsrsNcnzQcdAk2fz");

#[program]
pub mod cpmm_poc {
    use super::*;

    pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {
        instructions::create_pool(ctx, args)
    }

    pub fn initialize_virtual_token_account(
        ctx: Context<InitializeVirtualTokenAccount>,
    ) -> Result<()> {
        instructions::initialize_virtual_token_account(ctx)
    }

    pub fn buy_virtual_token(
        ctx: Context<BuyVirtualToken>,
        args: BuyVirtualTokenArgs,
    ) -> Result<()> {
        instructions::buy_virtual_token(ctx, args)
    }

    pub fn sell_virtual_token(
        ctx: Context<SellVirtualToken>,
        args: SellVirtualTokenArgs,
    ) -> Result<()> {
        instructions::sell_virtual_token(ctx, args)
    }

    pub fn burn_virtual_token(
        ctx: Context<BurnVirtualToken>,
        args: BurnVirtualTokenArgs,
    ) -> Result<()> {
        instructions::burn_virtual_token(ctx, args)
    }

    pub fn close_virtual_token_account(ctx: Context<CloseVirtualTokenAccount>) -> Result<()> {
        instructions::close_virtual_token_account(ctx)
    }
}
