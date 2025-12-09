use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializePlatformConfigArgs {
    pub creator_fee_bp: u16,
    pub topup_fee_bp: u16,
    pub platform_fee_bp: u16,

    pub burn_limit_bp_x100: u64,
    pub burn_min_burn_bp_x100: u64,
    pub burn_decay_rate_per_sec_bp_x100: u64,
    pub burn_tiers: Vec<BurnTier>,
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
    pub quote_mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_platform_config(
    ctx: Context<InitializePlatformConfig>,
    args: InitializePlatformConfigArgs,
) -> Result<()> {
    ctx.accounts
        .platform_config
        .set_inner(PlatformConfig::try_new(
            ctx.bumps.platform_config,
            ctx.accounts.creator.key(),
            ctx.accounts.creator.key(),
            ctx.accounts.quote_mint.key(),
            args.burn_tiers,
            args.creator_fee_bp,
            args.topup_fee_bp,
            args.platform_fee_bp,
            args.burn_limit_bp_x100,
            args.burn_min_burn_bp_x100,
            args.burn_decay_rate_per_sec_bp_x100,
        )?);
    Ok(())
}
