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
    pub creator_fee_basis_points: u16,
    pub topup_fee_basis_points: u16,
    pub platform_fee_basis_points: u16,
    pub burn_limit_bp_x100: u64,
    pub burn_min_burn_bp_x100: u64,
    pub burn_decay_rate_per_sec_bp_x100: u64,
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
        associated_token::mint = quote_mint,
        associated_token::authority = platform_config,
        associated_token::token_program = token_program
    )]
    pub platform_config_ata: InterfaceAccount<'info, TokenAccount>,
    pub quote_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn initialize_platform_config(
    ctx: Context<InitializePlatformConfig>,
    args: InitializePlatformConfigArgs,
) -> Result<()> {
    // todo check that the args combination is valid
    let burn_tiers = vec![
        BurnTier {
            burn_bp_x100: args.user_burn_bp_x100,
            role: BurnRole::Anyone,
            max_daily_burns: args.max_user_daily_burn_count,
        },
        BurnTier {
            burn_bp_x100: args.creator_burn_bp_x100,
            role: BurnRole::PoolCreator,
            max_daily_burns: args.max_creator_daily_burn_count,
        },
    ];
    ctx.accounts.platform_config.set_inner(PlatformConfig::try_new(
        ctx.bumps.platform_config,
        args.admin,
        ctx.accounts.creator.key(),
        ctx.accounts.quote_mint.key(),
        burn_tiers,
        args.creator_fee_basis_points,
        args.topup_fee_basis_points,
        args.platform_fee_basis_points,
        args.burn_limit_bp_x100,
        args.burn_min_burn_bp_x100,
        args.burn_decay_rate_per_sec_bp_x100,
    )?);
    Ok(())
}
