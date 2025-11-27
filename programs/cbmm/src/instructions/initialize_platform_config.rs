use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};


#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializePlatformConfigArgs {
    pub admin: Pubkey,
    pub burn_allowance: u16,
    pub max_user_daily_burn_count: u16,
    pub max_creator_daily_burn_count: u16,
    pub user_burn_bp_x100: u32,
    pub creator_burn_bp_x100: u32,
    pub burn_reset_time_of_day_seconds: u32, // Seconds from midnight
    pub creator_fee_basis_points: u16,
    pub buyback_fee_basis_points: u16,
    pub platform_fee_basis_points: u16,
}

#[derive(Accounts)]
pub struct InitializePlatformConfig<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        payer = creator,
        space = PlatformConfig::INIT_SPACE + 8,
        seeds = [PLATFORM_CONFIG_SEED, creator.key().as_ref()],
        bump
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(        
        associated_token::mint = a_mint,
        associated_token::authority = platform_config,
        associated_token::token_program = token_program
    )]
    pub platform_config_ata: InterfaceAccount<'info, TokenAccount>,
    pub a_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn initialize_platform_config(
    ctx: Context<InitializePlatformConfig>,
    args: InitializePlatformConfigArgs,
) -> Result<()> {
    ctx.accounts.platform_config.set_inner(PlatformConfig::try_new(
        ctx.bumps.platform_config,
        args.admin,
        ctx.accounts.creator.key(),
        ctx.accounts.a_mint.key(),
        args.burn_allowance,
        args.max_user_daily_burn_count,
        args.max_creator_daily_burn_count,
        args.user_burn_bp_x100,
        args.creator_burn_bp_x100,
        args.burn_reset_time_of_day_seconds,
        args.creator_fee_basis_points,
        args.buyback_fee_basis_points,
        args.platform_fee_basis_points,
    ));
    Ok(())
}
