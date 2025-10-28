use crate::errors::BcpmmError;
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
        seeds = [USER_BURN_ALLOWANCE_SEED, owner.key().as_ref(), &[args.pool_owner as u8]],
        bump
    )]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    /// CHECK: Checked that it's the same as the payer in the user burn allowance account.
    #[account(address = user_burn_allowance.payer @ BcpmmError::InvalidBurnAccountPayer)]
    pub burn_allowance_open_payer: AccountInfo<'info>,

    #[account(seeds = [CENTRAL_STATE_SEED], bump)]
    pub central_state: Account<'info, CentralState>,
}

pub fn close_user_burn_allowance(
    ctx: Context<CloseUserBurnAllowance>,
    _args: CloseUserBurnAllowanceArgs,
) -> Result<()> {
    // Only allow closing if the burn allowance is inactive: past the reset window and previous burn was before the reset.
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.central_state.is_after_burn_reset(now)?
            && !ctx
                .accounts
                .central_state
                .is_after_burn_reset(ctx.accounts.user_burn_allowance.last_burn_timestamp)?,
        BcpmmError::CannotCloseActiveBurnAllowance
    );

    Ok(())
}
