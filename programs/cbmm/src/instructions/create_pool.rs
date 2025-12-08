use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use crate::errors::CbmmError;


#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePoolArgs {
    /// quote_virtual_reserve is the virtual reserve of the A mint including decimals
    pub quote_virtual_reserve: u64,
}
#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        constraint = quote_mint.key() == platform_config.quote_mint @ CbmmError::InvalidMint
    )]
    pub quote_mint: InterfaceAccount<'info, Mint>,    
    
    #[account(init,
         payer = payer, 
         space = CbmmPool::INIT_SPACE + 8,
         seeds = [
            CBMM_POOL_SEED,
            CBMM_POOL_INDEX_SEED.to_le_bytes().as_ref(),
            payer.key().as_ref(),
            platform_config.key().as_ref()
          ],
         bump
    )]
    pub pool: Account<'info, CbmmPool>,        

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = quote_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program        
    )]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,    

    #[account(
        mut,
        seeds = [PLATFORM_CONFIG_SEED, platform_config.creator.key().as_ref()],
        bump = platform_config.bump
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {
    require_gte!(args.quote_virtual_reserve, MIN_VIRTUAL_RESERVE, CbmmError::InvalidVirtualReserve);
    let platform_config = &ctx.accounts.platform_config;
    ctx.accounts.pool.set_inner(CbmmPool::try_new(
        ctx.bumps.pool,
        ctx.accounts.payer.key(),
        CBMM_POOL_INDEX_SEED,
        ctx.accounts.platform_config.key(),
        ctx.accounts.quote_mint.key(),
        args.quote_virtual_reserve,
        platform_config.pool_creator_fee_basis_points,
        platform_config.pool_topup_fee_basis_points,
        platform_config.platform_fee_basis_points,
    )?);
    Ok(())
}