use crate::errors::CbmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CloseUserBurnAllowanceArgs {
    pub pool_owner: bool,
}

#[derive(Accounts)]
#[instruction(args: CloseUserBurnAllowanceArgs)]
pub struct CloseUserBurnAllowance<'info> {
    /// The user whose burn allowance is being closed
    /// CHECK: Can be any account.
    pub owner: UncheckedAccount<'info>,

    #[account(
        mut,
        close = burn_allowance_open_payer,
        seeds = [USER_BURN_ALLOWANCE_SEED, owner.key().as_ref(), platform_config.key().as_ref(), &[args.pool_owner as u8]],
        bump
    )]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    pub platform_config: Account<'info, PlatformConfig>,

    /// CHECK: Checked that it's the same as the payer in the user burn allowance account.
    #[account(address = user_burn_allowance.payer @ CbmmError::InvalidBurnAccountPayer)]
    pub burn_allowance_open_payer: AccountInfo<'info>,
}

pub fn close_user_burn_allowance(
    ctx: Context<CloseUserBurnAllowance>,
    _args: CloseUserBurnAllowanceArgs,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let time_since_last_burn = now - ctx.accounts.user_burn_allowance.last_burn_timestamp;
    require!(
        ctx.accounts.user_burn_allowance.burns_today == 0 || time_since_last_burn >= 86400,
        CbmmError::CannotCloseActiveBurnAllowance
    );

    Ok(())
}
