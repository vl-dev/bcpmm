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

    /// The minimum amount of Mint B to receive. If below this, the transaction will fail.
    pub b_amount_min: u64,
}

#[derive(Accounts)]
pub struct BuyVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = payer,
        associated_token::token_program = token_program        
    )]
    pub payer_ata: InterfaceAccount<'info, TokenAccount>,
    // todo check owner (or maybe not? can buy for other user)
    #[account(mut, seeds = [VIRTUAL_TOKEN_ACCOUNT_SEED, pool.key().as_ref(), payer.key().as_ref()], bump = virtual_token_account.bump)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
    #[account(mut, seeds = [BCPMM_POOL_SEED, pool.b_mint_index.to_le_bytes().as_ref()], bump = pool.bump)]
    pub pool: Account<'info, BcpmmPool>,
    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program        
    )]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,
    pub a_mint: InterfaceAccount<'info, Mint>,
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

    if output_amount < args.b_amount_min {
        msg!(
            "Expected output amount: {}, minimum required: {}",
            output_amount,
            args.b_amount_min
        );
        return Err(BcpmmError::SlippageExceeded.into());
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
    use crate::state::BcpmmPool;
    use crate::test_utils::{TestRunner, TestPool};
    use anchor_lang::prelude::*;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::pubkey::Pubkey;

    fn setup_test() -> (TestRunner, Keypair, Keypair, TestPool, Pubkey, Pubkey) {
        // Parameters
        let a_reserve = 0;
        let a_virtual_reserve = 1_000_000;
        let b_reserve = 2_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;

        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let another_wallet = Keypair::new();
        
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        runner.airdrop(&another_wallet.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let payer_ata = runner.create_associated_token_account(&payer, a_mint, &payer.pubkey());
        runner.mint_to(&payer, &a_mint, payer_ata, 10_000_000_000);

        runner.create_central_state_mock(&payer, 5, 5, 2, 1, 10000);

        let test_pool = runner.create_pool_mock(
            &payer,
            a_mint,
            a_reserve,
            a_virtual_reserve,
            b_reserve,
            b_mint_decimals,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            creator_fees_balance,
            buyback_fees_balance,
        );

        (runner, payer, another_wallet, test_pool, payer_ata, a_mint)
    }

    #[test]
    fn test_buy_virtual_token_success() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        let a_amount = 5000;
        let a_virtual_reserve = 1_000_000;
        let b_reserve = 2_000_000;

        let creator_fees = 100;
        let buyback_fees = 300;
        let a_amount_after_fees = a_amount - creator_fees - buyback_fees;

        let calculated_b_amount_min = 9157;
        let virtual_token_account =
            runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);

        let result_buy = runner.buy_virtual_token(
            &payer,
            payer_ata,
            a_mint,
            pool.pool,
            virtual_token_account,
            a_amount,
            calculated_b_amount_min,
        );
        assert!(result_buy.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.a_reserve, a_amount_after_fees);
        assert_eq!(pool_data.b_reserve, b_reserve - calculated_b_amount_min);
        assert_eq!(pool_data.a_virtual_reserve, a_virtual_reserve); // Unchanged
        assert_eq!(pool_data.buyback_fees_balance, buyback_fees);
        assert_eq!(pool_data.creator_fees_balance, creator_fees);
    }

    #[test]
    fn test_buy_virtual_token_slippage_exceeded() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        let a_amount = 5000;
        let calculated_b_amount_min = 9157;

        let virtual_token_account =
            runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);

        let result_buy_min_too_high = runner.buy_virtual_token(
            &payer,
            payer_ata,
            a_mint,
            pool.pool,
            virtual_token_account,
            a_amount,
            calculated_b_amount_min + 1, // Set minimum too high
        );
        assert!(result_buy_min_too_high.is_err());
    }

    #[test]
    fn test_buy_virtual_token_wrong_virtual_account_owner() {
        let (mut runner, payer, another_wallet, pool, payer_ata, a_mint) = setup_test();
        
        let a_amount = 5000;
        let calculated_b_amount_min = 9157;

        let virtual_token_account_another_wallet =
            runner.create_virtual_token_account_mock(another_wallet.pubkey(), pool.pool, 0, 0);

        let result_buy_another_virtual_account = runner.buy_virtual_token(
            &payer,
            payer_ata,
            a_mint,
            pool.pool,
            virtual_token_account_another_wallet,
            a_amount,
            calculated_b_amount_min,
        );
        assert!(result_buy_another_virtual_account.is_err());
    }
}
