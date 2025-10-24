use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateTreasuryAuthorityArgs {
    pub new_treasury_authority: Pubkey,
}

#[derive(Accounts)]
pub struct UpdateTreasuryAuthority<'info> {
    #[account(mut)]
    pub current_authority: Signer<'info>,
    
    #[account(
        mut,
        seeds = [TREASURY_SEED, a_mint.key().as_ref()],
        bump = treasury.bump,
        constraint = treasury.authority == current_authority.key() @ BcpmmError::InvalidAdmin
    )]
    pub treasury: Account<'info, Treasury>,

    pub a_mint: InterfaceAccount<'info, Mint>,
}

pub fn update_treasury_authority(
    ctx: Context<UpdateTreasuryAuthority>,
    args: UpdateTreasuryAuthorityArgs,
) -> Result<()> {
    ctx.accounts.treasury.authority = args.new_treasury_authority;
    Ok(())
}
