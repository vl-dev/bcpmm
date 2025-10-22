use crate::helpers::{calculate_burn_amount, calculate_new_virtual_reserve};
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BurnVirtualTokenArgs {
    pub b_amount_basis_points: u16, // 1 not small enough, todo change some micro units
}

#[derive(Accounts)]
pub struct BurnVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut, seeds = [BCPMM_POOL_SEED, pool.b_mint_index.to_le_bytes().as_ref()], bump = pool.bump)]
    pub pool: Account<'info, BcpmmPool>,
}

pub fn burn_virtual_token(
    ctx: Context<BurnVirtualToken>,
    args: BurnVirtualTokenArgs,
) -> Result<()> {
    let burn_amount =
        calculate_burn_amount(args.b_amount_basis_points, ctx.accounts.pool.b_reserve);
    let new_virtual_reserve = calculate_new_virtual_reserve(
        ctx.accounts.pool.a_virtual_reserve,
        ctx.accounts.pool.b_reserve,
        burn_amount,
    );
    ctx.accounts.pool.a_remaining_topup +=
        ctx.accounts.pool.a_virtual_reserve - new_virtual_reserve;
    ctx.accounts.pool.a_virtual_reserve = new_virtual_reserve;
    ctx.accounts.pool.b_reserve -= burn_amount;
    Ok(())
}
