use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeCentralStateArgs {
    pub admin: Pubkey,
    pub max_user_daily_burn_count: u16,
    pub max_creator_daily_burn_count: u16,
    pub user_burn_bp_x100: u32,
    pub creator_burn_bp_x100: u32,
    pub burn_reset_time_of_day_seconds: u32, // Seconds from midnight
    pub creator_fee_basis_points: u16,
    pub buyback_fee_basis_points: u16,
    pub platform_fee_basis_points: u16,
}

#[derive(Accounts)]
pub struct InitializeCentralState<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = CentralState::INIT_SPACE + 8, seeds = [CENTRAL_STATE_SEED], bump)]
    pub central_state: Account<'info, CentralState>,
    pub system_program: Program<'info, System>,
    #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()))]
    pub program_data: Account<'info, ProgramData>,
}

pub fn initialize_central_state(
    ctx: Context<InitializeCentralState>,
    args: InitializeCentralStateArgs,
) -> Result<()> {
    ctx.accounts.central_state.set_inner(CentralState::new(
        ctx.bumps.central_state,
        args.admin,
        args.max_user_daily_burn_count,
        args.max_creator_daily_burn_count,
        args.user_burn_bp_x100,
        args.creator_burn_bp_x100,
        args.burn_reset_time_of_day_seconds,
        args.creator_fee_basis_points,
        args.buyback_fee_basis_points,
        args.platform_fee_basis_points,
    ));
    Ok(())
}
