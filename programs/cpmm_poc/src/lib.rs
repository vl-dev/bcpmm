#![allow(unexpected_cfgs)]
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

    pub fn initialize_central_state(
        ctx: Context<InitializeCentralState>,
        args: InitializeCentralStateArgs,
    ) -> Result<()> {
        instructions::initialize_central_state(ctx, args)
    }

    pub fn initialize_user_burn_allowance(
        ctx: Context<InitializeUserBurnAllowance>,
        pool_owner: bool,
    ) -> Result<()> {
        instructions::initialize_user_burn_allowance(ctx, pool_owner)
    }

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

    pub fn burn_virtual_token(ctx: Context<BurnVirtualToken>, pool_owner: bool) -> Result<()> {
        instructions::burn_virtual_token(ctx, pool_owner)
    }

    pub fn close_virtual_token_account(ctx: Context<CloseVirtualTokenAccount>) -> Result<()> {
        instructions::close_virtual_token_account(ctx)
    }
    pub fn close_user_burn_allowance(
        ctx: Context<CloseUserBurnAllowance>,
        args: CloseUserBurnAllowanceArgs,
    ) -> Result<()> {
        instructions::close_user_burn_allowance(ctx, args)
    }
    pub fn claim_creator_fees(
        ctx: Context<ClaimCreatorFees>,
        args: ClaimCreatorFeesArgs,
    ) -> Result<()> {
        instructions::claim_creator_fees(ctx, args)
    }
    pub fn claim_admin_fees(ctx: Context<ClaimAdminFees>) -> Result<()> {
        instructions::claim_admin_fees(ctx)
    }
}
