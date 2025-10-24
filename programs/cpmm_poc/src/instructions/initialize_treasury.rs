use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeTreasuryArgs {
    pub treasury_authority: Pubkey,
}

#[derive(Accounts)]
pub struct InitializeTreasury<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    
    #[account(
        mut,
        seeds = [CENTRAL_STATE_SEED],
        bump = central_state.bump,
        constraint = central_state.admin == admin.key() @ BcpmmError::InvalidAdmin
    )]
    pub central_state: Account<'info, CentralState>,
    
    #[account(
        init,
        payer = admin,
        space = Treasury::INIT_SPACE + 8,
        seeds = [TREASURY_SEED],
        bump
    )]
    pub treasury: Account<'info, Treasury>,
    
    pub system_program: Program<'info, System>,
}

pub fn initialize_treasury(
    ctx: Context<InitializeTreasury>,
    args: InitializeTreasuryArgs,
) -> Result<()> {
    ctx.accounts.treasury.set_inner(Treasury::new(
        args.treasury_authority,
        ctx.bumps.treasury,
    ));
    Ok(())
}
