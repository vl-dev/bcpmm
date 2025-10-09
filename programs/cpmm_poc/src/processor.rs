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

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = CentralState::INIT_SPACE + 8,
        seeds = [b"central_state"],
        bump
    )]
    pub central_state: Account<'info, CentralState>,

    pub acs_mint: InterfaceAccount<'info, Mint>,

    // todo: check init if needed
    #[account(
        init,
        payer = payer,
        associated_token::mint = acs_mint,
        associated_token::authority = central_state,
        associated_token::token_program = token_program        
    )]
    pub central_state_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
}

pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    let central_state = &mut ctx.accounts.central_state;
    central_state.acs_mint = ctx.accounts.acs_mint.key();
    central_state.mint_counter = 0;
    central_state.acs_mint_decimals = ctx.accounts.acs_mint.decimals;
    Ok(())
}

#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub central_state: Account<'info, CentralState>,
    #[account(init, payer = payer, space = CpmmPool::INIT_SPACE + 8, seeds = [b"cpmm_pool", central_state.mint_counter.to_le_bytes().as_ref()], bump)]
    pub cpmm_pool: Account<'info, CpmmPool>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn create_pool(
    ctx: Context<CreatePool>,
    initial_supply: u64, // todo decimals
    virtual_base_reserve: u64,
) -> Result<()> {
    let cpmm_pool = &mut ctx.accounts.cpmm_pool;
    cpmm_pool.virtual_acs_reserve = virtual_base_reserve;
    cpmm_pool.micro_acs_reserve = 0;
    cpmm_pool.ct_reserve = initial_supply * 10u64.pow(CT_MINT_DECIMALS as u32);
    cpmm_pool.mint_index = ctx.accounts.central_state.mint_counter;

    let central_state = &mut ctx.accounts.central_state;
    central_state.mint_counter += 1;
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeCtAccount<'info> {
    // todo: check init if needed
    #[account(init, payer = payer, space = CtAccount::INIT_SPACE + 8, seeds = [b"token_account", cpmm_pool.key().as_ref(), payer.key().as_ref()], bump)]
    pub token_account: Account<'info, CtAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub cpmm_pool: Account<'info, CpmmPool>,
}

pub fn initialize_ct_account(ctx: Context<InitializeCtAccount>) -> Result<()> {
    let token_account = &mut ctx.accounts.token_account;
    token_account.balance = 0;
    token_account.pool = ctx.accounts.cpmm_pool.key();
    Ok(())
}

#[derive(Accounts)]
pub struct BuyToken<'info> {
    #[account(mut)]
    pub ct_account: Account<'info, CtAccount>,
    #[account(mut)]
    pub central_state_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub cpmm_pool: Account<'info, CpmmPool>,
    pub acs_mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub acs_ata: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// todo pool should have its own ata - performance reasons
// todo check the rigthh floor/ceil
// todo check the micro ACS and decimal stuff
pub fn buy_token(ctx: Context<BuyToken>, amount_micro_acs: u64) -> Result<()> {
    let ct_account = &mut ctx.accounts.ct_account;    
    let output_amount = calculate_buy_output_amount(
        amount_micro_acs,
        ctx.accounts.cpmm_pool.micro_acs_reserve,
        ctx.accounts.cpmm_pool.ct_reserve,
        ctx.accounts.cpmm_pool.virtual_acs_reserve,
    );
    ct_account.balance += output_amount;
    ctx.accounts.cpmm_pool.micro_acs_reserve += amount_micro_acs;
    ctx.accounts.cpmm_pool.ct_reserve -= output_amount;    

    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.acs_mint.to_account_info(),
        from: ctx.accounts.acs_ata.to_account_info(),
        to: ctx.accounts.central_state_ata.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
    transfer_checked(
        cpi_context,
        amount_micro_acs,
        ctx.accounts.acs_mint.decimals,
    )?;
    Ok(())
}

// todo safe math, u128
fn calculate_buy_output_amount(amount_micro_acs: u64, micro_acs_reserve: u64, ct_reserve: u64, virtual_micro_acs_reserve: u64) -> u64 {
    let virtual_x = micro_acs_reserve + virtual_micro_acs_reserve;
    msg!("virtual_x: {}", virtual_x);
    let y = ct_reserve;
    msg!("y: {}", y);
    let k = (virtual_x) * y;
    msg!("k: {}", k);
    let delta_x = amount_micro_acs;
    msg!("delta_x: {}", delta_x);
    let delta_y = y - k / (virtual_x + delta_x);
    msg!("delta_y: {}", delta_y);
    return delta_y;
}

#[derive(Accounts)]
pub struct SellToken<'info> {
    #[account(mut)]
    pub ct_account: Account<'info, CtAccount>,
    #[account(mut)]
    pub central_state_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(
        seeds = [b"central_state"],
        bump,
    )]
    pub central_state: Account<'info, CentralState>,
    pub cpmm_pool: Account<'info, CpmmPool>,
    pub acs_mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub acs_ata: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn sell_token(ctx: Context<SellToken>, amount_ct: u64) -> Result<()> {
    let ct_account = &mut ctx.accounts.ct_account;    
    require!(ct_account.balance >= amount_ct, ErrorCode::InvalidNumericConversion); // todo real error
    let output_amount = calculate_sell_output_amount(
        amount_ct,
        ctx.accounts.cpmm_pool.ct_reserve,
        ctx.accounts.cpmm_pool.micro_acs_reserve,
        ctx.accounts.cpmm_pool.virtual_acs_reserve,
    );
    msg!("current amount of ct: {}", ct_account.balance);
    ct_account.balance -= amount_ct;
    ctx.accounts.cpmm_pool.micro_acs_reserve -= output_amount;
    ctx.accounts.cpmm_pool.ct_reserve += amount_ct;    

    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.acs_mint.to_account_info(),
        from: ctx.accounts.central_state_ata.to_account_info(),
        to: ctx.accounts.acs_ata.to_account_info(),
        authority: ctx.accounts.central_state.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let bump_seed = ctx.bumps.central_state;
    let signer_seeds: &[&[&[u8]]] = &[&[b"central_state".as_ref(), &[bump_seed]]];
    let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
    transfer_checked(
        cpi_context,
        output_amount,
        ctx.accounts.acs_mint.decimals,
    )?;
    Ok(())
}

// todo maybe 
fn calculate_sell_output_amount(amount_ct: u64, ct_reserve: u64, micro_acs_reserve: u64, virtual_acs_reserve: u64) -> u64 {
    let virtual_x = micro_acs_reserve + virtual_acs_reserve;
    msg!("virtual_x: {}", virtual_x);
    let y = ct_reserve;
    msg!("y: {}", y);
    let k = (virtual_x) * y;
    msg!("k: {}", k);
    let delta_y = amount_ct;
    msg!("delta_y: {}", delta_y);
    let delta_x =  virtual_x -k / (y + delta_y);
    msg!("delta_x: {}", delta_x);
    return delta_x;
}