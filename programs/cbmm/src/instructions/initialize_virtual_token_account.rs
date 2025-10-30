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
    ctx.accounts
        .virtual_token_account
        .set_inner(VirtualTokenAccount::try_new(
            ctx.bumps.virtual_token_account,
            ctx.accounts.pool.key(),
            ctx.accounts.owner.key(),
        ));
    Ok(())
}
