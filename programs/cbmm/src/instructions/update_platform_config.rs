use crate::errors::CbmmError;
use crate::helpers::BurnRateConfig;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdatePlatformConfigArgs {
    pub pool_creator_fee_bp: Option<u16>,
    pub pool_topup_fee_bp: Option<u16>,
    pub platform_fee_bp: Option<u16>,
    pub burn_authority: Option<Option<Pubkey>>,
    pub burn_limit_bp_x100: Option<u64>,
    pub burn_min_bp_x100: Option<u64>,
    pub burn_decay_rate_per_sec_bp_x100: Option<u64>,
    pub burn_tiers: Option<Vec<BurnTier>>,
}

#[derive(Accounts)]
pub struct UpdatePlatformConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PLATFORM_CONFIG_SEED, platform_config.creator.as_ref()],
        has_one = admin @ CbmmError::InvalidPlatformAdmin,
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,
}

pub fn update_platform_config(
    ctx: Context<UpdatePlatformConfig>,
    args: UpdatePlatformConfigArgs,
) -> Result<()> {
    let platform_config = &mut ctx.accounts.platform_config;
    let now = Clock::get()?.unix_timestamp;

    // Check if burn_tiers are being updated, and if so, verify they weren't updated in the last hour
    if args.burn_tiers.is_some() {
        let one_hour_ago = now.saturating_sub(3600);
        require_gt!(
            one_hour_ago,
            platform_config.burn_tiers_updated_at,
            CbmmError::BurnTiersUpdatedTooRecently
        );
    }

    // Update fields that are provided
    if let Some(pool_creator_fee_bp) = args.pool_creator_fee_bp {
        platform_config.pool_creator_fee_bp = pool_creator_fee_bp;
    }
    if let Some(pool_topup_fee_bp) = args.pool_topup_fee_bp {
        platform_config.pool_topup_fee_bp = pool_topup_fee_bp;
    }
    if let Some(platform_fee_bp) = args.platform_fee_bp {
        platform_config.platform_fee_bp = platform_fee_bp;
    }
    if let Some(burn_authority) = args.burn_authority {
        platform_config.burn_authority = burn_authority;
    }
    if let Some(burn_tiers) = args.burn_tiers {
        platform_config.burn_tiers = burn_tiers;
        platform_config.burn_tiers_updated_at = now;
    }

    // Update burn_rate_config if any of its fields are provided
    if args.burn_limit_bp_x100.is_some()
        || args.burn_min_bp_x100.is_some()
        || args.burn_decay_rate_per_sec_bp_x100.is_some()
    {
        let burn_limit_bp_x100 = args
            .burn_limit_bp_x100
            .unwrap_or(platform_config.burn_rate_config.burn_limit_bp_x100);
        let burn_min_bp_x100 = args
            .burn_min_bp_x100
            .unwrap_or(platform_config.burn_rate_config.burn_min_bp_x100);
        let burn_decay_rate_per_sec_bp_x100 = args
            .burn_decay_rate_per_sec_bp_x100
            .unwrap_or(platform_config.burn_rate_config.decay_rate_per_sec_bp_x100);

        platform_config.burn_rate_config = BurnRateConfig::new(
            burn_limit_bp_x100,
            burn_min_bp_x100,
            burn_decay_rate_per_sec_bp_x100,
        );
    }

    // Validate the entire config
    platform_config.validate_fees_and_burn_config()?;

    // Validate burn_tiers length
    require!(
        platform_config.burn_tiers.len() <= 5,
        CbmmError::InvalidBurnTiers
    );

    Ok(())
}
