use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    Mint, TokenAccount, TokenInterface,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SellVirtualTokenArgs {
    pub b_amount: u64,
}

#[derive(Accounts)]
pub struct SellVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = payer,
        associated_token::token_program = token_program        
    )]
    pub payer_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [VIRTUAL_TOKEN_ACCOUNT_SEED, pool.key().as_ref(), payer.key().as_ref()], bump = virtual_token_account.bump)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,

    #[account(mut, seeds = [BCPMM_POOL_SEED, pool.pool_index.to_le_bytes().as_ref(), pool.creator.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, BcpmmPool>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program        
    )]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = central_state,
        associated_token::token_program = token_program        
    )]
    pub central_state_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [CENTRAL_STATE_SEED], bump)]
    pub central_state: Account<'info, CentralState>,

    pub a_mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}
pub fn sell_virtual_token(
    ctx: Context<SellVirtualToken>,
    args: SellVirtualTokenArgs,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;
    require_gte!(virtual_token_account.balance, args.b_amount, BcpmmError::InsufficientVirtualTokenBalance);

    let output_amount = pool.calculate_sell_output_amount(args.b_amount);
    require_gte!(pool.a_reserve, output_amount, BcpmmError::Underflow);

    let fees = pool.calculate_fees(output_amount)?;
    virtual_token_account.sub(args.b_amount, &fees)?;

    // Update the pool state        
    let real_topup_amount = pool.a_outstanding_topup.min(fees.buyback_fees_amount);
    pool.a_outstanding_topup -= real_topup_amount;    
    pool.buyback_fees_balance += fees.buyback_fees_amount - real_topup_amount;
    pool.creator_fees_balance += fees.creator_fees_amount;
    pool.a_reserve -= output_amount - real_topup_amount;
    pool.b_reserve += args.b_amount;    

    let pool_account_info = pool.to_account_info();
    pool.transfer_out(
        output_amount - fees.total_fees_amount(),
        &pool_account_info,
        &ctx.accounts.a_mint,
        &ctx.accounts.pool_ata,
        &ctx.accounts.payer_ata,
        &ctx.accounts.token_program
    )?;

    pool.transfer_out(
        fees.platform_fees_amount,
        &pool_account_info,
        &ctx.accounts.a_mint,
        &ctx.accounts.pool_ata,
        &ctx.accounts.central_state_ata,
        &ctx.accounts.token_program,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::state::BcpmmPool;
    use crate::test_utils::TestRunner;
    use anchor_lang::prelude::*;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::pubkey::Pubkey;

    fn setup_test() -> (TestRunner, Keypair, Keypair, Pubkey, Pubkey, Pubkey) {
        // Parameters
        let a_reserve = 5000;
        let a_virtual_reserve = 1_000_000;
        let b_reserve = 2_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let platform_fee_basis_points = 200;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;
        let a_outstanding_topup = 150;

        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let another_wallet = Keypair::new();
        
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        runner.airdrop(&another_wallet.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let payer_ata = runner.create_associated_token_account(&payer, a_mint, &payer.pubkey());
        runner.mint_to(&payer, &a_mint, payer_ata, 10_000_000_000);
        let central_state = runner.create_central_state_mock(&payer, 5, 5, 2, 1, 10000, creator_fee_basis_points, buyback_fee_basis_points, platform_fee_basis_points);

        // central state ata
        runner.create_associated_token_account(&payer, a_mint, &central_state);

        let created_pool = runner.create_pool_mock(
            &payer,
            a_mint,
            a_reserve,
            a_virtual_reserve,
            b_reserve,
            b_mint_decimals,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
            creator_fees_balance,
            buyback_fees_balance,
            a_outstanding_topup,
        );

        // pool ata
        runner.create_associated_token_account(&payer, a_mint, &created_pool.pool);
        runner.mint_tokens(&payer, created_pool.pool, a_mint, a_reserve);
        (runner, payer, another_wallet, created_pool.pool, payer_ata, a_mint)
    }

    #[test]
    fn test_sell_virtual_token_success() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        let b_amount = 1000;
        let b_sell_amount = 500;
        let a_reserve = 5000;
        let a_virtual_reserve = 1_000_000;
        let b_reserve = 2_000_000;
        let buyback_fee_basis_points = 600;
        let a_outstanding_topup = 150;

        // Create virtual token account with some balance to sell
        let virtual_token_account = runner.create_virtual_token_account_mock(
            payer.pubkey(),
            pool,
            b_amount, // Start with balance to sell
            0,
        );

        // Test successful sell
        let result_sell = runner.sell_virtual_token(
            &payer,
            payer_ata,
            a_mint,
            pool,
            virtual_token_account,
            b_sell_amount,
        );
        assert!(result_sell.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();

        let expected_output_amount = 251;

        // account for topup repayment from buyback fees on sell (ceil division like in helpers)
        let buyback_fees = ((expected_output_amount as u128 * buyback_fee_basis_points as u128 + 9999) / 10_000) as u64;
        let real_topup = buyback_fees.min(a_outstanding_topup);
        let expected_a_reserve_after = a_reserve - expected_output_amount + real_topup;
        assert_eq!(pool_data.a_reserve, expected_a_reserve_after);
        assert_eq!(pool_data.b_reserve, b_reserve + b_sell_amount);
        assert_eq!(pool_data.a_virtual_reserve, a_virtual_reserve);
        assert_eq!(pool_data.a_outstanding_topup, a_outstanding_topup - buyback_fees);
    }

    #[test]
    fn test_sell_virtual_token_insufficient_balance() {
        let (mut runner, _, another_wallet, pool, payer_ata, a_mint) = setup_test();
        
        let b_amount = 1000;

        // Create virtual token account with insufficient balance
        let virtual_token_account_insufficient = runner.create_virtual_token_account_mock(
            another_wallet.pubkey(),
            pool,
            b_amount - 1, // Insufficient balance
            0,
        );

        let result_sell_insufficient = runner.sell_virtual_token(
            &another_wallet,
            payer_ata,
            a_mint,
            pool,
            virtual_token_account_insufficient,
            b_amount,
        );
        assert!(result_sell_insufficient.is_err());
    }

    #[test]
    fn test_sell_virtual_token_wrong_owner() {
        let (mut runner, payer, another_wallet, pool, payer_ata, a_mint) = setup_test();
        
        let b_amount = 1000;

        // Create virtual token account with wrong owner
        let virtual_token_account_wrong_owner = runner.create_virtual_token_account_mock(
            another_wallet.pubkey(),
            pool,
            b_amount,
            0,
        );

        let result_sell_wrong_owner = runner.sell_virtual_token(
            &payer, // payer is different from virtual token account owner
            payer_ata,
            a_mint,
            pool,
            virtual_token_account_wrong_owner,
            b_amount,
        );
        assert!(result_sell_wrong_owner.is_err());
    }

    #[test]
    fn test_sell_virtual_token_above_balance() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        let b_amount = 1000;

        // Create virtual token account with some balance to sell
        let virtual_token_account = runner.create_virtual_token_account_mock(
            payer.pubkey(),
            pool,
            b_amount, // Start with balance to sell
            0,
        );

        // Test trying to sell more than user has
        let result_sell_above_balance = runner.sell_virtual_token(
            &payer,
            payer_ata,
            a_mint,
            pool,
            virtual_token_account,
            b_amount + 1,
        );
        assert!(result_sell_above_balance.is_err());
    }
}
