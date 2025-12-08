use crate::errors::CbmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseVirtualTokenAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        close = owner,
        has_one = owner @ CbmmError::InvalidOwner,
        constraint = virtual_token_account.balance == 0 @ CbmmError::NonzeroBalance
    )]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
}

pub fn close_virtual_token_account(_ctx: Context<CloseVirtualTokenAccount>) -> Result<()> {
    Ok(())
}
