use crate::errors::CbmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct InitializeUserBurnAllowanceArgs {
    pub burn_tier_index: u8,
}

#[derive(Accounts)]
#[instruction(args: InitializeUserBurnAllowanceArgs)]
pub struct InitializeUserBurnAllowance<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The user for whom the burn allowance is being initialized
    /// CHECK: This is just a pubkey, not an account
    pub owner: UncheckedAccount<'info>,

    #[account(
        init,
        payer = payer,
        space = 8 + UserBurnAllowance::INIT_SPACE,
        seeds = [
            USER_BURN_ALLOWANCE_SEED,
            owner.key().as_ref(),
            platform_config.key().as_ref(),
            &[args.burn_tier_index],
            platform_config.burn_tiers_updated_at.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    pub platform_config: Account<'info, PlatformConfig>,

    pub system_program: Program<'info, System>,

    /// Optional pool account - only needed if requesting a pool creator burn tier
    #[account(
        seeds = [
            CBMM_POOL_SEED,
            pool.pool_index.to_le_bytes().as_ref(),
            pool.creator.as_ref(),
            platform_config.key().as_ref(),
        ],
        bump = pool.bump,
    )]
    pub pool: Option<Account<'info, CbmmPool>>,
}

pub fn initialize_user_burn_allowance(
    ctx: Context<InitializeUserBurnAllowance>,
    args: InitializeUserBurnAllowanceArgs,
) -> Result<()> {
    require_gt!(
        ctx.accounts.platform_config.burn_tiers.len() as u8,
        args.burn_tier_index,
        CbmmError::InvalidBurnTierIndex
    );

    let burn_tier = &ctx.accounts.platform_config.burn_tiers[args.burn_tier_index as usize];
    match burn_tier.role {
        BurnRole::PoolOwner => {
            require!(
                ctx.accounts.pool.is_some(),
                CbmmError::PoolCreatorBurnTierRequiresPool
            );
            let pool = &ctx.accounts.pool.as_ref().unwrap();
            require_keys_eq!(pool.creator, ctx.accounts.owner.key());
        }
        BurnRole::SpecificPubkey(pubkey) => {
            require_keys_eq!(pubkey, ctx.accounts.owner.key());
        }
        BurnRole::Anyone => {}
    }

    ctx.accounts
        .user_burn_allowance
        .set_inner(UserBurnAllowance::new(
            ctx.bumps.user_burn_allowance,
            ctx.accounts.owner.key(),
            ctx.accounts.platform_config.key(),
            ctx.accounts.payer.key(),
            args.burn_tier_index,
            ctx.accounts.platform_config.burn_tiers_updated_at,
            Clock::get()?.unix_timestamp,
        ));
    Ok(())
}
