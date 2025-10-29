use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};


#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePoolArgs {
    /// a_virtual_reserve is the virtual reserve of the A mint including decimals
    pub a_virtual_reserve: u64,
}
#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub a_mint: InterfaceAccount<'info, Mint>,    
    
    #[account(init,
         payer = payer, 
         space = BcpmmPool::INIT_SPACE + 8,
         seeds = [BCPMM_POOL_SEED, BCPMM_POOL_INDEX_SEED.to_le_bytes().as_ref(), payer.key().as_ref()],
         bump
    )]
    pub pool: Account<'info, BcpmmPool>,        

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = a_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program        
    )]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,    

    #[account(mut)]
    pub central_state: Account<'info, CentralState>,

    #[account(
        init_if_needed, 
        payer = payer, 
        associated_token::mint = a_mint, 
        associated_token::authority = central_state, 
        associated_token::token_program = token_program
    )]
    pub central_state_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {    
    let central_state = &ctx.accounts.central_state;
    ctx.accounts.pool.set_inner(BcpmmPool::try_new(
        ctx.bumps.pool,
        ctx.accounts.payer.key(),
        BCPMM_POOL_INDEX_SEED,
        ctx.accounts.a_mint.key(),
        args.a_virtual_reserve,
        central_state.creator_fee_basis_points,
        central_state.buyback_fee_basis_points,
        central_state.platform_fee_basis_points,
    )?);
    Ok(())
}