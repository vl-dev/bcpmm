use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount, Token};
use anchor_spl::associated_token::AssociatedToken;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePoolArgs {
    /// a_virtual_reserve is the virtual reserve of the A mint including decimals
    pub a_virtual_reserve: u64,

    /// creator_fee_basis_points is the fee basis points for the creator.
    pub creator_fee_basis_points: u16,

    /// buyback_fee_basis_points is the fee basis points for the buyback.
    pub buyback_fee_basis_points: u16,
}
#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub a_mint: Account<'info, Mint>,    
    #[account(init, payer = payer, space = BcpmmPool::INIT_SPACE + 8, seeds = [BCPMM_POOL_SEED, central_state.b_mint_index.to_le_bytes().as_ref()], bump)]
    pub pool: Account<'info, BcpmmPool>,        

    #[account(mut, seeds = [TREASURY_SEED, a_mint.key().as_ref()], bump = treasury.bump)]
    pub treasury: Account<'info, Treasury>,

    #[account(mut)]
    pub central_state: Account<'info, CentralState>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {    
    ctx.accounts.pool.set_inner(BcpmmPool::try_new(
        ctx.bumps.pool,
        ctx.accounts.payer.key(),
        ctx.accounts.a_mint.key(),
        args.a_virtual_reserve,
        ctx.accounts.central_state.b_mint_index,
        args.creator_fee_basis_points,
        args.buyback_fee_basis_points,
    )?);

    ctx.accounts.central_state.b_mint_index += 1;
    Ok(())
}