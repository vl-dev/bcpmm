use crate::helpers::{calculate_burn_amount, calculate_new_virtual_reserve};
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeCentralStateArgs {
    pub daily_burn_allowance: u64,         // todo change some micro units
    pub creator_daily_burn_allowance: u64, // todo change some micro units
    pub user_burn_bp: u16,                 // todo change some micro units
    pub creator_burn_bp: u16,              // todo change some micro units
    pub burn_reset_time: u64,
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
        args.daily_burn_allowance,
        args.creator_daily_burn_allowance,
        args.user_burn_bp,
        args.creator_burn_bp,
        args.burn_reset_time,
    ));
    Ok(())
}
