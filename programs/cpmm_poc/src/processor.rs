use crate::state::*;
use anchor_lang::error::ErrorCode;
use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_interface::{transfer_checked, Mint, TokenAccount, TransferChecked},
};

const CT_MINT_DECIMALS: u8 = 6;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePoolArgs {
    pub b_initial_supply: u64,
    pub a_virtual_reserve: u64,
}
#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub a_mint: InterfaceAccount<'info, Mint>,
    // todo add empty mint check and move mint authority to program (or pool)
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
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {
    // todo account checks
    let pool = &mut ctx.accounts.pool;
    pool.a_mint = ctx.accounts.a_mint.to_account_info().key();
    pool.a_reserve = 0;
    pool.b_mint = ctx.accounts.b_mint.to_account_info().key();
    pool.b_reserve = args.b_initial_supply;
    pool.a_virtual_reserve = args.a_virtual_reserve;

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeVirtualTokenAccount<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(init, payer = payer, space = VirtualTokenAccount::INIT_SPACE + 8, seeds = [VIRTUAL_TOKEN_ACCOUNT_SEED, pool.key().as_ref(), payer.key().as_ref()], bump)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
    pub pool: Account<'info, BcpmmPool>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_virtual_token_account(ctx: Context<InitializeVirtualTokenAccount>) -> Result<()> {
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;
    virtual_token_account.balance = 0;
    virtual_token_account.pool = ctx.accounts.pool.key();
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BuyVirtualTokenArgs {
    pub a_amount: u64,
}
#[derive(Accounts)]
pub struct BuyVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub payer_ata: InterfaceAccount<'info, TokenAccount>,
    // todo check owner (or maybe not? can buy for other user)
    #[account(mut)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,        
    #[account(mut, seeds = [BCPMM_POOL_SEED, b_mint.key().as_ref()], bump)]
    pub pool: Account<'info, BcpmmPool>,
    // todo check owner
    #[account(mut)]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,
    pub a_mint: InterfaceAccount<'info, Mint>,    
    /// UNCHECKED: this is a virtual mint so it doesn't really exist
    pub b_mint: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,    
}

// todo check the right floor/ceil
// todo check the decimals
pub fn buy_virtual_token(ctx: Context<BuyVirtualToken>, args: BuyVirtualTokenArgs) -> Result<()> {
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;
    let output_amount = calculate_buy_output_amount(
        args.a_amount,
        ctx.accounts.pool.a_reserve,
        ctx.accounts.pool.b_reserve,
        ctx.accounts.pool.a_virtual_reserve,
    );
    virtual_token_account.balance += output_amount;
    ctx.accounts.pool.a_reserve += args.a_amount;
    ctx.accounts.pool.b_reserve -= output_amount;

    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.payer_ata.to_account_info(),
        to: ctx.accounts.pool_ata.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
    transfer_checked(cpi_context, args.a_amount, ctx.accounts.a_mint.decimals)?;
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SellVirtualTokenArgs {
    pub b_amount: u64,
}

#[derive(Accounts)]
pub struct SellVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub payer_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
    #[account(mut, seeds = [BCPMM_POOL_SEED, b_mint.key().as_ref()], bump)]
    pub pool: Account<'info, BcpmmPool>,
    #[account(mut)]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,
    pub a_mint: InterfaceAccount<'info, Mint>,    
    /// UNCHECKED: this is a virtual mint so it doesn't really exist
    pub b_mint: AccountInfo<'info>,
    pub system_program: Program<'info, System>,    
    pub token_program: Program<'info, Token>,
}

pub fn sell_virtual_token(ctx: Context<SellVirtualToken>, args: SellVirtualTokenArgs) -> Result<()> {
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;
    require!(
        virtual_token_account.balance >= args.b_amount,
        ErrorCode::InvalidNumericConversion
    ); // todo real error
    let output_amount = calculate_sell_output_amount(
        args.b_amount,
        ctx.accounts.pool.b_reserve,
        ctx.accounts.pool.a_reserve,
        ctx.accounts.pool.a_virtual_reserve,
    );
    require!(
        ctx.accounts.pool.a_reserve >= output_amount,
        ErrorCode::InvalidNumericConversion
    ); // prevent underflow on a_reserve
    virtual_token_account.balance -= args.b_amount;
    ctx.accounts.pool.a_reserve -= output_amount;
    ctx.accounts.pool.b_reserve += args.b_amount;

    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.pool_ata.to_account_info(),
        to: ctx.accounts.payer_ata.to_account_info(),
        authority: ctx.accounts.pool.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let bump_seed = ctx.bumps.pool;
    let b_mint_key = ctx.accounts.b_mint.to_account_info().key();
    let signer_seeds: &[&[&[u8]]] = &[&[BCPMM_POOL_SEED, b_mint_key.as_ref(), &[bump_seed]]];
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts).with_signer(signer_seeds);
    let decimals = ctx.accounts.a_mint.decimals;
    transfer_checked(cpi_context, output_amount, decimals)?;
    Ok(())
}

fn calculate_buy_output_amount(
    a_amount: u64,
    a_reserve: u64,
    b_reserve: u64,
    a_virtual_reserve: u64,
) -> u64 {
    let numerator = b_reserve as u128 * a_amount as u128;
    let denominator = a_reserve as u128 + a_virtual_reserve as u128 + a_amount as u128;
    (numerator / denominator) as u64
}

fn calculate_sell_output_amount(
    b_amount: u64,
    b_reserve: u64,
    a_reserve: u64,
    a_virtual_reserve: u64,
) -> u64 {
    let numerator = (a_virtual_reserve as u128 - a_reserve as u128) * b_amount as u128;
    let denominator = b_reserve as u128 + b_amount as u128;
    (numerator / denominator) as u64
}