#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

mod errors;
mod helpers;
mod instructions;
mod state;

#[cfg(test)]
mod test_utils;

use instructions::*;

declare_id!("CBMMzs3HKfTMudbXifeNcw3NcHQhZX7izDBKoGDLRdjj");

#[program]
pub mod cbmm {
    use super::*;

    pub fn initialize_platform_config(
        ctx: Context<InitializePlatformConfig>,
        args: InitializePlatformConfigArgs,
    ) -> Result<()> {
        instructions::initialize_platform_config(ctx, args)
    }

    pub fn initialize_user_burn_allowance(
        ctx: Context<InitializeUserBurnAllowance>,
        args: InitializeUserBurnAllowanceArgs,
    ) -> Result<()> {
        instructions::initialize_user_burn_allowance(ctx, args)
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

    pub fn burn_virtual_token(ctx: Context<BurnVirtualToken>) -> Result<()> {
        instructions::burn_virtual_token(ctx)
    }

    pub fn close_virtual_token_account(ctx: Context<CloseVirtualTokenAccount>) -> Result<()> {
        instructions::close_virtual_token_account(ctx)
    }
    pub fn close_user_burn_allowance(ctx: Context<CloseUserBurnAllowance>) -> Result<()> {
        instructions::close_user_burn_allowance(ctx)
    }
    pub fn claim_creator_fees(ctx: Context<ClaimCreatorFees>) -> Result<()> {
        instructions::claim_creator_fees(ctx)
    }
    pub fn claim_platform_fees(ctx: Context<ClaimPlatformFees>) -> Result<()> {
        instructions::claim_platform_fees(ctx)
    }
}
