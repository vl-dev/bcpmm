use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeCentralStateArgs {
    pub max_user_daily_burn_count: u16,
    pub max_creator_daily_burn_count: u16,
    pub user_burn_bp_x100: u32,
    pub creator_burn_bp_x100: u32,
    pub burn_reset_time_of_day_seconds: u32, // Seconds from midnight
}

#[derive(Accounts)]
pub struct InitializeCentralState<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(init, payer = admin, space = CentralState::INIT_SPACE + 8, seeds = [CENTRAL_STATE_SEED], bump)]
    pub central_state: Account<'info, CentralState>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_central_state(
    ctx: Context<InitializeCentralState>,
    args: InitializeCentralStateArgs,
) -> Result<()> {
    ctx.accounts.central_state.set_inner(CentralState::new(
        ctx.bumps.central_state,
        ctx.accounts.admin.key(),
        args.max_user_daily_burn_count,
        args.max_creator_daily_burn_count,
        args.user_burn_bp_x100,
        args.creator_burn_bp_x100,
        args.burn_reset_time_of_day_seconds,
    ));
    Ok(())
}
