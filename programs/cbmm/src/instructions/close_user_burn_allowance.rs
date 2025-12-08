use crate::errors::CbmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseUserBurnAllowance<'info> {
    /// The user whose burn allowance is being closed
    /// CHECK: Can be any account.
    pub owner: UncheckedAccount<'info>,

    #[account(
        mut,
        close = burn_allowance_open_payer,
        seeds = [
            USER_BURN_ALLOWANCE_SEED,
            owner.key().as_ref(),
            platform_config.key().as_ref(),
            &[user_burn_allowance.burn_tier_index],
            user_burn_allowance.burn_tier_update_timestamp.to_le_bytes().as_ref(),
        ],
        bump = user_burn_allowance.bump
    )]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    pub platform_config: Account<'info, PlatformConfig>,

    /// CHECK: Checked that it's the same as the payer in the user burn allowance account.
    #[account(address = user_burn_allowance.payer @ CbmmError::InvalidBurnAccountPayer)]
    pub burn_allowance_open_payer: AccountInfo<'info>,
}

pub fn close_user_burn_allowance(ctx: Context<CloseUserBurnAllowance>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let is_closable = ctx
        .accounts
        .user_burn_allowance
        .is_closable(ctx.accounts.platform_config.burn_tiers_updated_at, now);
    require!(is_closable, CbmmError::CannotCloseActiveBurnAllowance);

    Ok(())
}
