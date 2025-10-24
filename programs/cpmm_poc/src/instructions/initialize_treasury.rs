use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::Mint;
use anchor_spl::token::{TokenAccount, Token};

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
        seeds = [TREASURY_SEED, a_mint.key().as_ref()],
        bump
    )]
    pub treasury: Account<'info, Treasury>,

    #[account(
        init_if_needed,
        payer = admin,
        associated_token::mint = a_mint,
        associated_token::authority = treasury,
        associated_token::token_program = token_program,
    )]
    pub treasury_ata: Account<'info, TokenAccount>,
    
    pub a_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
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
