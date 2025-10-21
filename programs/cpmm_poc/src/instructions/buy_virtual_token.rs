use crate::errors::BcpmmError;
use crate::helpers::{calculate_buy_output_amount, calculate_fees};
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

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

pub fn buy_virtual_token(ctx: Context<BuyVirtualToken>, args: BuyVirtualTokenArgs) -> Result<()> {
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;

    let fees = calculate_fees(
        args.a_amount,
        ctx.accounts.pool.creator_fee_basis_points,
        ctx.accounts.pool.buyback_fee_basis_points,
    )?;

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
        let real_topup_amount = if remaining_topup_amount > buyback_fees_amount {
            buyback_fees_amount
        } else {
            remaining_topup_amount
        };
        ctx.accounts.pool.a_remaining_topup =
            ctx.accounts.pool.a_remaining_topup - real_topup_amount;
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

#[cfg(test)]
mod tests {
    use crate::test_utils::TestRunner;

    #[test]
    fn test_buy_virtual_token() {
        // Initialize the test environment
        let mut runner = TestRunner::new(9);
        let test_pool = runner.create_pool_mock(0, 1000000, 2000000, 6, 200, 600, 0, 0);
        let virtual_token_account = runner.create_virtual_token_account_mock(test_pool.pool, 0, 0);
        let result = runner.buy_virtual_token(
            test_pool.pool,
            virtual_token_account,
            5000,
            test_pool.b_mint,
        );
        assert!(result.is_ok());
    }
}
