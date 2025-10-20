use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use anchor_spl::associated_token::AssociatedToken;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePoolArgs {
    /// b_initial_supply is the initial supply of the B mint including decimals
    pub b_initial_supply: u64,

    /// b_decimals is the decimals of the B mint.
    pub b_decimals: u8,

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
    pub a_mint: InterfaceAccount<'info, Mint>,
    // todo add empty mint check and move account owner to program (or pool) - this needs to be Signer
    /// UNCHECKED: this is a virtual mint so it doesn't really exist
    #[account(mut)]
    pub b_mint: AccountInfo<'info>,
    #[account(init, payer = payer, space = BcpmmPool::INIT_SPACE + 8, seeds = [BCPMM_POOL_SEED, b_mint.key().as_ref()], bump)]
    pub pool: Account<'info, BcpmmPool>,
        // todo: check init if needed
    #[account(
        init,
        payer = payer,
        associated_token::mint = a_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program        
    )]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,    
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {
    // todo account checks
    let pool = &mut ctx.accounts.pool;
    pool.a_mint = ctx.accounts.a_mint.to_account_info().key();
    pool.a_reserve = 0;
    pool.a_virtual_reserve = args.a_virtual_reserve;
    pool.a_remaining_topup = 0;

    pool.b_mint = ctx.accounts.b_mint.to_account_info().key();
    pool.b_mint_decimals = args.b_decimals;
    pool.b_reserve = args.b_initial_supply;

    pool.creator_fee_basis_points = args.creator_fee_basis_points;
    pool.buyback_fee_basis_points = args.buyback_fee_basis_points;
    pool.creator = ctx.accounts.payer.key();
    pool.creator_fees_balance = 0;
    pool.buyback_fees_balance = 0;
    Ok(())
}