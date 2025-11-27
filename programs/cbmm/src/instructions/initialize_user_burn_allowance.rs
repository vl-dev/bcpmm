use crate::state::*;
use anchor_lang::prelude::*;


#[derive(Accounts)]
#[instruction(pool_owner: bool)]
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
        seeds = [USER_BURN_ALLOWANCE_SEED, owner.key().as_ref(), platform_config.key().as_ref(), &[pool_owner as u8]],
        bump
    )]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    pub platform_config: Account<'info, PlatformConfig>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_user_burn_allowance(
    ctx: Context<InitializeUserBurnAllowance>,
    _: bool,
) -> Result<()> {
    ctx.accounts.user_burn_allowance.set_inner(UserBurnAllowance::new(
        ctx.bumps.user_burn_allowance,
        ctx.accounts.owner.key(),
        ctx.accounts.payer.key(),
    ));
    Ok(())
}
