use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseVirtualTokenAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        close = owner,
        has_one = owner @ BcpmmError::InvalidOwner,
        constraint = virtual_token_account.balance == 0 @ BcpmmError::NonzeroBalance
    )]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
}

pub fn close_virtual_token_account(ctx: Context<CloseVirtualTokenAccount>) -> Result<()> {
    msg!(
        "Closing virtual token account, collected fees: {}",
        ctx.accounts.virtual_token_account.fees_paid
    );
    Ok(())
}
