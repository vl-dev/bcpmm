use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializeVirtualTokenAccount<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: No check needed, owner can be any account
    pub owner: AccountInfo<'info>,
    #[account(init, payer = payer, space = VirtualTokenAccount::INIT_SPACE + 8, seeds = [VIRTUAL_TOKEN_ACCOUNT_SEED, pool.key().as_ref(), payer.key().as_ref()], bump)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
    pub pool: Account<'info, BcpmmPool>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_virtual_token_account(ctx: Context<InitializeVirtualTokenAccount>) -> Result<()> {
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;
    virtual_token_account.pool = ctx.accounts.pool.key();
    virtual_token_account.owner = ctx.accounts.owner.key();
    virtual_token_account.balance = 0;
    virtual_token_account.fees_paid = 0;

    Ok(())
}
