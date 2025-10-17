use crate::state::*;
use crate::errors::BcpmmError;
use anchor_lang::error::ErrorCode;
use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{transfer_checked, Mint, TokenAccount, TransferChecked, TokenInterface},
};

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

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BuyVirtualTokenArgs {
    /// a_amount is the amount of Mint A to swap for Mint B. Includes decimals.
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
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,    
}

// todo check the right floor/ceil
// todo check the decimals
pub fn buy_virtual_token(ctx: Context<BuyVirtualToken>, args: BuyVirtualTokenArgs) -> Result<()> {
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;

    let fees = calculate_fees(
        args.a_amount,
        ctx.accounts.pool.creator_fee_basis_points,
        ctx.accounts.pool.buyback_fee_basis_points,
    );

    let swap_amount = args.a_amount - fees.creator_fees_amount - fees.buyback_fees_amount;

    let output_amount = calculate_buy_output_amount(
        swap_amount,
        ctx.accounts.pool.a_reserve,
        ctx.accounts.pool.b_reserve,
        ctx.accounts.pool.a_virtual_reserve,
    );

    if output_amount == 0 {
        return Err(BcpmmError::AmountTooSmall.into());
    }

    virtual_token_account.balance += output_amount;
    virtual_token_account.fees_paid += fees.creator_fees_amount + fees.buyback_fees_amount;
    ctx.accounts.pool.a_reserve += swap_amount;
    ctx.accounts.pool.b_reserve -= output_amount;
    ctx.accounts.pool.creator_fees_balance += fees.creator_fees_amount;    
    let remaining_topup_amount = ctx.accounts.pool.a_remaining_topup;
    if remaining_topup_amount > 0 {        
        let buyback_fees_amount = fees.buyback_fees_amount;
        let real_topup_amount = if remaining_topup_amount > buyback_fees_amount { buyback_fees_amount } else { remaining_topup_amount };
        ctx.accounts.pool.a_remaining_topup = ctx.accounts.pool.a_remaining_topup - real_topup_amount;
        ctx.accounts.pool.a_reserve += real_topup_amount;
    } else {
        ctx.accounts.pool.buyback_fees_balance += fees.buyback_fees_amount;
    }

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
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn sell_virtual_token(ctx: Context<SellVirtualToken>, args: SellVirtualTokenArgs) -> Result<()> {
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;

    require!(
        virtual_token_account.balance >= args.b_amount,
        BcpmmError::InsufficientVirtualTokenBalance
    );

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

    let fees = calculate_fees(
        output_amount,
        ctx.accounts.pool.creator_fee_basis_points,
        ctx.accounts.pool.buyback_fee_basis_points,
    );
    virtual_token_account.fees_paid += fees.creator_fees_amount + fees.buyback_fees_amount;
    ctx.accounts.pool.creator_fees_balance += fees.creator_fees_amount;
    if ctx.accounts.pool.a_remaining_topup > 0 {
        let remaining_topup_amount = ctx.accounts.pool.a_remaining_topup;
        let real_topup_amount = if remaining_topup_amount > fees.buyback_fees_amount { fees.buyback_fees_amount } else { remaining_topup_amount };
        ctx.accounts.pool.a_remaining_topup = ctx.accounts.pool.a_remaining_topup - real_topup_amount;
        ctx.accounts.pool.a_reserve += real_topup_amount;
    } else {
        ctx.accounts.pool.buyback_fees_balance += fees.buyback_fees_amount;
    }

    let output_amount_less_fees = output_amount - fees.creator_fees_amount - fees.buyback_fees_amount;

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
    transfer_checked(cpi_context, output_amount_less_fees, decimals)?;

    Ok(())
}

#[derive(Accounts)]
pub struct CloseVirtualTokenAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        close = owner,
        constraint = virtual_token_account.owner == owner.key() @ BcpmmError::InvalidOwner,
        constraint = virtual_token_account.balance == 0 @ BcpmmError::NonzeroBalance
    )]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,    
}

pub fn close_virtual_token_account(ctx: Context<CloseVirtualTokenAccount>) -> Result<()> {
    msg!("Closing virtual token account, collected fees: {}", ctx.accounts.virtual_token_account.fees_paid);
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BurnVirtualTokenArgs {
    pub b_amount_basis_points: u16, // 1 not small enough, todo change some micro units
}

#[derive(Accounts)]
pub struct BurnVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, BcpmmPool>,
}

pub fn burn_virtual_token(ctx: Context<BurnVirtualToken>, args: BurnVirtualTokenArgs) -> Result<()> {
    let burn_amount = calculate_burn_amount(
        args.b_amount_basis_points,
        ctx.accounts.pool.b_reserve,
    );    
    let new_virtual_reserve = calculate_new_virtual_reserve(
        ctx.accounts.pool.a_virtual_reserve,
        ctx.accounts.pool.b_reserve,
        burn_amount,
    );
    ctx.accounts.pool.a_remaining_topup += ctx.accounts.pool.a_virtual_reserve - new_virtual_reserve;
    ctx.accounts.pool.a_virtual_reserve = new_virtual_reserve;
    ctx.accounts.pool.b_reserve -= burn_amount;
    Ok(())
}
pub struct Fees {
    pub creator_fees_amount: u64,
    pub buyback_fees_amount: u64,
}

fn calculate_fees(
    a_amount: u64,
    creator_fee_basis_points: u16,
    buyback_fee_basis_points: u16,
) -> Fees {
    // Use ceiling division for fees to avoid rounding down: ceil(x / d) = (x + d - 1) / d
    let creator_fees_amount = ((a_amount as u128 * creator_fee_basis_points as u128 + 9999) / 10000) as u64;
    let buyback_fees_amount = ((a_amount as u128 * buyback_fee_basis_points as u128 + 9999) / 10000) as u64;
    Fees {
        creator_fees_amount,
        buyback_fees_amount,
    }
}

/// Calculates the amount of Mint B received when spending Mint A.
fn calculate_buy_output_amount(
    a_amount: u64,
    a_reserve: u64,
    b_reserve: u64,
    a_virtual_reserve: u64    
) -> u64 {
    let numerator = b_reserve as u128 * a_amount as u128;
    let denominator = a_reserve as u128 + a_virtual_reserve as u128 + a_amount as u128;
    (numerator / denominator) as u64
}

/// Calculates the amount of Mint A received when selling Mint B.
fn calculate_sell_output_amount(
    b_amount: u64,
    b_reserve: u64,
    a_reserve: u64,
    a_virtual_reserve: u64,
) -> u64 {
    let numerator = b_amount as u128 * (a_reserve as u128 + a_virtual_reserve as u128);
    let denominator = b_reserve as u128 + b_amount as u128;
    (numerator / denominator) as u64
}

fn calculate_burn_amount(
    b_amount_basis_points: u16,
    b_reserve: u64,
) -> u64 {
    (b_reserve as u128 * b_amount_basis_points as u128 / 10000 as u128) as u64
}

fn calculate_new_virtual_reserve(
    a_virtual_reserve: u64,
    b_reserve: u64,
    b_burn_amount: u64,
) -> u64 {
    (a_virtual_reserve as u128 * (b_reserve as u128 - b_burn_amount as u128) / b_reserve as u128) as u64
}