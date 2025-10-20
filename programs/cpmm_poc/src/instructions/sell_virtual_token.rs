use crate::errors::BcpmmError;
use crate::helpers::{calculate_fees, calculate_sell_output_amount};
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

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

pub fn sell_virtual_token(
    ctx: Context<SellVirtualToken>,
    args: SellVirtualTokenArgs,
) -> Result<()> {
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
    )?;
    virtual_token_account.fees_paid += fees.creator_fees_amount + fees.buyback_fees_amount;
    ctx.accounts.pool.creator_fees_balance += fees.creator_fees_amount;
    if ctx.accounts.pool.a_remaining_topup > 0 {
        let remaining_topup_amount = ctx.accounts.pool.a_remaining_topup;
        let real_topup_amount = if remaining_topup_amount > fees.buyback_fees_amount {
            fees.buyback_fees_amount
        } else {
            remaining_topup_amount
        };
        ctx.accounts.pool.a_remaining_topup =
            ctx.accounts.pool.a_remaining_topup - real_topup_amount;
        ctx.accounts.pool.a_reserve += real_topup_amount;
    } else {
        ctx.accounts.pool.buyback_fees_balance += fees.buyback_fees_amount;
    }

    let output_amount_less_fees =
        output_amount - fees.creator_fees_amount - fees.buyback_fees_amount;

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
