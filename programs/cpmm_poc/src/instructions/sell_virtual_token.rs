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
    #[account(mut, seeds = [VIRTUAL_TOKEN_ACCOUNT_SEED, pool.key().as_ref(), payer.key().as_ref()], bump)]
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

#[cfg(test)]
mod tests {
    use crate::helpers::{calculate_sell_output_amount, calculate_fees};
    use crate::state::BcpmmPool;
    use crate::test_utils::TestRunner;
    use anchor_lang::prelude::*;
    use solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_sell_virtual_token() {
        // Parameters
        let b_amount = 1000;
        let b_sell_amount = 500;
        let a_reserve = 5000;
        let a_virtual_reserve = 1_000_000;
        let b_reserve = 2_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;

        // Initialize the test environment and related accounts
        let payer = Keypair::new();
        let another_wallet = Keypair::new();
        let mut runner = TestRunner::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        runner.airdrop(&another_wallet.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let payer_ata = runner.create_associated_token_account(&payer, a_mint);
        runner.mint_to(&payer, &a_mint, payer_ata, 10_000_000_000);

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
        
        // Create virtual token account with some balance to sell
        let virtual_token_account = runner.create_virtual_token_account_mock(
            payer.pubkey(),
            test_pool.pool,
            b_amount, // Start with balance to sell
            0,
        );

        // Test selling with insufficient balance
        let virtual_token_account_insufficient = runner.create_virtual_token_account_mock(
            another_wallet.pubkey(),
            test_pool.pool,
            b_amount - 1, // Insufficient balance
            0,
        );
        let result_sell_insufficient = runner.sell_virtual_token(
            &another_wallet,
            payer_ata,
            a_mint,
            test_pool.pool,
            virtual_token_account_insufficient,
            b_amount,
            test_pool.b_mint,
        );
        assert!(result_sell_insufficient.is_err());

        // Test selling with wrong virtual token account owner
        let virtual_token_account_wrong_owner = runner.create_virtual_token_account_mock(
            another_wallet.pubkey(),
            test_pool.pool,
            b_amount,
            0,
        );
        let result_sell_wrong_owner = runner.sell_virtual_token(
            &payer, // payer is different from virtual token account owner
            payer_ata,
            a_mint,
            test_pool.pool,
            virtual_token_account_wrong_owner,
            b_amount,
            test_pool.b_mint,
        );
        assert!(result_sell_wrong_owner.is_err());

        // Test trying to sell more than user has
        let result_sell_above_balance = runner.sell_virtual_token(
            &payer,
            payer_ata,
            a_mint,
            test_pool.pool,
            virtual_token_account,
            b_amount + 1,
            test_pool.b_mint,
        );
        assert!(result_sell_above_balance.is_err());

        // Test successful sell
        let result_sell = runner.sell_virtual_token(
            &payer,
            payer_ata,
            a_mint,
            test_pool.pool,
            virtual_token_account,
            b_sell_amount,
            test_pool.b_mint,
        );
        assert!(result_sell.is_ok());

        // Fetch the test_pool from testrunner lite svm and deserialize the account data
        let pool_account = runner.svm.get_account(&test_pool.pool).unwrap();
        let pool_data: BcpmmPool = BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        
        // Check that the reserves are updated correctly
        let output_amount = calculate_sell_output_amount(
            b_sell_amount,
            b_reserve,
            a_reserve,
            a_virtual_reserve,
        );
        assert_eq!(pool_data.a_reserve, a_reserve - output_amount);
        assert_eq!(pool_data.b_reserve, b_reserve + b_sell_amount);
        assert_eq!(pool_data.a_virtual_reserve, a_virtual_reserve); // Unchanged
    }
}
