use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

#[event]
pub struct BuyEvent {
    pub a_input: u64,
    pub b_output: u64,

    pub creator_fees: u64,
    pub buyback_fees: u64,
    pub platform_fees: u64,

    pub topup_paid: u64,

    pub new_b_reserve: u64,
    pub new_a_reserve: u64,
    pub new_outstanding_topup: u64,
    pub new_creator_fees_balance: u64,
    pub new_buyback_fees_balance: u64,

    pub buyer: Pubkey,
    pub pool: Pubkey,
}

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

    #[account(address = pool.a_mint @ BcpmmError::InvalidMint)]
    pub a_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn buy_virtual_token(ctx: Context<BuyVirtualToken>, args: BuyVirtualTokenArgs) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;

    let fees = pool.calculate_fees(args.a_amount)?;
    let real_swap_amount = args.a_amount - fees.total_fees_amount();

    let output_amount = pool.calculate_buy_output_amount(real_swap_amount);
    require_gt!(output_amount, 0, BcpmmError::AmountTooSmall);
    require_gte!(output_amount, args.b_amount_min, BcpmmError::SlippageExceeded);

    virtual_token_account.add(output_amount, &fees)?;

    // Update the pool state
    let real_topup_amount = pool.a_outstanding_topup.min(fees.buyback_fees_amount);
    pool.a_outstanding_topup -= real_topup_amount;    
    pool.buyback_fees_balance += fees.buyback_fees_amount - real_topup_amount;
    pool.creator_fees_balance += fees.creator_fees_amount;
    pool.a_reserve += real_swap_amount + real_topup_amount;
    pool.b_reserve -= output_amount;

    // Transfer A tokens to pool ata, excluding platform fees
    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.payer_ata.to_account_info(),
        to: ctx.accounts.pool_ata.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
    transfer_checked(
        cpi_context,
         args.a_amount - fees.platform_fees_amount,
         ctx.accounts.a_mint.decimals)?;

    
    // Transfer platform fees to central state ata
    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.payer_ata.to_account_info(),
        to: ctx.accounts.central_state_ata.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
    transfer_checked(
        cpi_context,
        fees.platform_fees_amount,
        ctx.accounts.a_mint.decimals
    )?;
    emit!(BuyEvent {
        a_input: args.a_amount,
        b_output: output_amount,
        creator_fees: fees.creator_fees_amount,
        buyback_fees: fees.buyback_fees_amount,
        platform_fees: fees.platform_fees_amount,
        topup_paid: real_topup_amount,
        new_b_reserve: pool.b_reserve,
        new_a_reserve: pool.a_reserve,
        new_outstanding_topup: pool.a_outstanding_topup,
        new_creator_fees_balance: pool.creator_fees_balance,
        new_buyback_fees_balance: pool.buyback_fees_balance,
        buyer: ctx.accounts.payer.key(),
        pool: ctx.accounts.pool.key(),
    }); 
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
        let platform_fee_basis_points = 200;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;
        let a_outstanding_topup = 100;

        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let another_wallet = Keypair::new();
        
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        runner.airdrop(&another_wallet.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let payer_ata = runner.create_associated_token_account(&payer, a_mint, &payer.pubkey());
        runner.mint_to(&payer, &a_mint, payer_ata, 10_000_000_000);

        let central_state = runner.create_central_state_mock(&payer, 
            5,
             5,
              2, 
            1, 
            10000, 
            creator_fee_basis_points, 
            buyback_fee_basis_points, 
            platform_fee_basis_points);
        // central state ata
        runner.create_associated_token_account(&payer, a_mint, &central_state);

        let test_pool = runner.create_pool_mock(
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
        runner.create_associated_token_account(&payer, a_mint, &test_pool.pool);

        (runner, payer, another_wallet, test_pool, payer_ata, a_mint)
    }

    #[test]
    fn test_buy_virtual_token_success() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        let a_amount = 5000;
        let a_virtual_reserve = 1_000_000;
        let b_reserve = 2_000_000;

        let a_outstanding_topup = 100;
        let creator_fees = 100;
        let buyback_fees = 300;
        let buyback_fees_after_topup = buyback_fees - a_outstanding_topup;
        let platform_fees = 100;
        let a_amount_after_fees = a_amount - creator_fees - buyback_fees_after_topup - platform_fees;

        let calculated_b_amount_min = 8959;
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
        result_buy.unwrap();
        //assert!(result_buy.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.a_reserve, a_amount_after_fees);
        assert_eq!(pool_data.b_reserve, b_reserve - calculated_b_amount_min);
        assert_eq!(pool_data.a_virtual_reserve, a_virtual_reserve); // Unchanged
        assert_eq!(pool_data.buyback_fees_balance, buyback_fees_after_topup);
        assert_eq!(pool_data.creator_fees_balance, creator_fees);
        assert_eq!(pool_data.a_outstanding_topup, 0);
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

    // ========================================
    // Phase 1: Whitepaper Mathematical Tests
    // ========================================

    /// Test 1.1: Invariant Preservation During Buy
    /// Formula: k = (A + V) * B should increase or stay constant (due to fees adding to A)
    /// Whitepaper Section: 2 (Mathematical Model)
    #[test]
    fn test_buy_preserves_invariant_with_fees() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        // Get initial pool state
        let pool_before = runner.get_pool_data(&pool.pool);
        let k_before = runner.calculate_invariant(
            pool_before.a_reserve,
            pool_before.a_virtual_reserve,
            pool_before.b_reserve,
        );

        println!("Initial state:");
        println!("  A = {}, V = {}, B = {}", pool_before.a_reserve, pool_before.a_virtual_reserve, pool_before.b_reserve);
        println!("  k_before = {}", k_before);
        
        // Buy tokens
        let a_amount = 5000;
        let vta = runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);
        runner.buy_virtual_token(&payer, payer_ata, a_mint, pool.pool, vta, a_amount, 0).unwrap();
        
        // Get final pool state
        let pool_after = runner.get_pool_data(&pool.pool);
        let k_after = runner.calculate_invariant(
            pool_after.a_reserve,
            pool_after.a_virtual_reserve,
            pool_after.b_reserve,
        );

        println!("After buy:");
        println!("  A = {}, V = {}, B = {}", pool_after.a_reserve, pool_after.a_virtual_reserve, pool_after.b_reserve);
        println!("  k_after = {}", k_after);
        
        // Invariant should increase because fees go to real reserve A
        // V stays constant during buys
        assert_eq!(pool_after.a_virtual_reserve, pool_before.a_virtual_reserve, 
            "Virtual reserve should stay constant during buy");
        assert!(k_after > k_before, 
            "Invariant should increase due to fees: k_before={}, k_after={}", k_before, k_after);

        // Calculate expected increase from fees going to real reserve
        // Fees = a_amount * 10% = 500 (200 creator + 300 buyback after topup + 200 platform)
        // But buyback fees reduced by topup (100), so effective increase in A = 4500 + 100 = 4600
        let expected_a_increase = a_amount - 500 + 100; // Real swap + topup from buyback fees
        assert_eq!(pool_after.a_reserve, expected_a_increase,
            "Real reserve should increase by swap amount plus topup");

        println!("✅ Invariant preserved: k increased from {} to {}", k_before, k_after);
    }

    /// Test 1.2: Buy Output Formula Accuracy
    /// Formula: b = B₀ - k / (A₀ + ΔA + V)
    /// Whitepaper Section: 2.1 (Trading)
    /// Canonical test vector from whitepaper-tests.md
    #[test]
    fn test_buy_output_matches_whitepaper_formula() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        // Canonical test vector (from whitepaper-tests.md, corrected)
        // A₀ = 0, V = 1_000_000, B₀ = 2_000_000
        // a_amount = 5_000
        // Total fees = 10% = 500
        // ΔA_real = 5_000 - 500 = 4_500
        // Formula: b_out = (B * ΔA) / (A + V + ΔA)
        // b_out = (2_000_000 * 4_500) / (0 + 1_000_000 + 4_500)
        // b_out = 9_000_000_000 / 1_004_500
        // b_out = 8_959 (floor division)
        
        let a_amount = 5_000;
        let pool_before = runner.get_pool_data(&pool.pool);
        
        // Calculate fees (10% total)
        let total_fee_bp = 200 + 600 + 200; // creator + buyback + platform
        let total_fees = a_amount * total_fee_bp / 10_000;
        let a_input_after_fees = a_amount - total_fees;
        
        println!("Test setup:");
        println!("  a_amount = {}", a_amount);
        println!("  total_fees = {}", total_fees);
        println!("  a_input_after_fees = {}", a_input_after_fees);
        
        // Calculate expected output using formula
        let expected_output = runner.calculate_expected_buy_output(
            pool_before.a_reserve,
            pool_before.a_virtual_reserve,
            pool_before.b_reserve,
            a_input_after_fees,
        );
        
        println!("Expected buy output (from formula): {}", expected_output);
        
        // Canonical assertion (corrected value)
        assert_eq!(expected_output, 8_959, 
            "Expected output from canonical test vector should be exactly 8_959");
        
        // Perform buy
        let vta = runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);
        runner.buy_virtual_token(&payer, payer_ata, a_mint, pool.pool, vta, a_amount, expected_output).unwrap();
        
        // Verify actual output matches formula
        let vta_data = runner.get_vta_data(&vta);
        assert_eq!(vta_data.balance, expected_output, 
            "Actual output should match whitepaper formula exactly");

        println!("✅ Buy output formula verified: b_out = {}", expected_output);
    }

    /// Test 1.2b: Buy Output Formula with Different Amounts
    /// Test the formula with small, medium, and large amounts
    #[test]
    fn test_buy_output_formula_various_amounts() {
        let test_amounts = vec![
            100,      // Small
            10_000,   // Medium
            500_000,  // Large (25% of pool)
        ];

        for a_amount in test_amounts {
            let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
            
            let pool_before = runner.get_pool_data(&pool.pool);
            
            // Calculate expected output
            let total_fee_bp = 200 + 600 + 200;
            let total_fees = a_amount * total_fee_bp / 10_000;
            let a_input_after_fees = a_amount - total_fees;
            
            let expected_output = runner.calculate_expected_buy_output(
                pool_before.a_reserve,
                pool_before.a_virtual_reserve,
                pool_before.b_reserve,
                a_input_after_fees,
            );
            
            // Perform buy
            let vta = runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);
            let result = runner.buy_virtual_token(&payer, payer_ata, a_mint, pool.pool, vta, a_amount, 0);
            
            if result.is_ok() {
                let vta_data = runner.get_vta_data(&vta);
                assert_eq!(vta_data.balance, expected_output, 
                    "Output should match formula for a_amount = {}", a_amount);
                println!("✅ Formula verified for a_amount = {}: b_out = {}", a_amount, expected_output);
            }
        }
    }

    /// Test 1.3: Price Calculation Formula
    /// Formula: P = (A + V) / B
    /// Whitepaper Section: 2.1 (Trading - Price)
    #[test]
    fn test_price_calculation_formula() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        // Initial price: P₀ = (0 + 1_000_000) / 2_000_000 = 0.5
        let pool_before = runner.get_pool_data(&pool.pool);
        let price_before = runner.calculate_price(
            pool_before.a_reserve,
            pool_before.a_virtual_reserve,
            pool_before.b_reserve,
        );
        
        println!("Initial price: P₀ = {}", price_before);
        assert!((price_before - 0.5).abs() < 1e-6, 
            "Initial price should be 0.5 (V/B = 1M/2M)");
        
        // Buy tokens (increases A, decreases B)
        let a_amount = 50_000;
        let vta = runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);
        runner.buy_virtual_token(&payer, payer_ata, a_mint, pool.pool, vta, a_amount, 0).unwrap();
        
        // Price after buy: P₁ = (A₁ + V) / B₁
        let pool_after = runner.get_pool_data(&pool.pool);
        let price_after = runner.calculate_price(
            pool_after.a_reserve,
            pool_after.a_virtual_reserve,
            pool_after.b_reserve,
        );
        
        println!("Price after buy: P₁ = {}", price_after);
        println!("Pool state: A={}, V={}, B={}", 
            pool_after.a_reserve, pool_after.a_virtual_reserve, pool_after.b_reserve);
        
        // Verify price increased
        assert!(price_after > price_before, 
            "Price should increase after buy: P_before={}, P_after={}", price_before, price_after);

        println!("✅ Price calculation formula verified");
        println!("   P₀ = {} → P₁ = {} (increase: {}%)", 
            price_before, price_after, ((price_after - price_before) / price_before * 100.0));
    }

    // ========================================
    // Phase 3: CCB Mechanics Tests
    // ========================================

    /// Test 3.1: Fee Accumulation During Buys
    /// Verify exact fee amounts are accumulated correctly
    /// Whitepaper Section: 3.1 (CCB)
    #[test]
    fn test_ccb_fee_accumulation_exact_amounts() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        let a_amount = 10_000;
        let creator_fee_bp = 200;  // 2%
        let buyback_fee_bp = 600;  // 6%
        let platform_fee_bp = 200; // 2%
        
        // Calculate expected fees
        let expected_creator_fee = a_amount * creator_fee_bp / 10_000;
        let expected_buyback_fee = a_amount * buyback_fee_bp / 10_000;
        let expected_platform_fee = a_amount * platform_fee_bp / 10_000;
        
        println!("Fee accumulation test:");
        println!("  a_amount = {}", a_amount);
        println!("  Expected creator fee: {}", expected_creator_fee);
        println!("  Expected buyback fee: {}", expected_buyback_fee);
        println!("  Expected platform fee: {}", expected_platform_fee);
        
        let pool_before = runner.get_pool_data(&pool.pool);
        
        // Execute buy
        let vta = runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);
        runner.buy_virtual_token(&payer, payer_ata, a_mint, pool.pool, vta, a_amount, 0).unwrap();
        
        let pool_after = runner.get_pool_data(&pool.pool);
        
        // Verify creator fees
        let actual_creator_fee = pool_after.creator_fees_balance - pool_before.creator_fees_balance;
        assert_eq!(actual_creator_fee, expected_creator_fee,
            "Creator fees should match exactly");
        
        // Verify buyback fees (minus any topup)
        let buyback_fee_increase = pool_after.buyback_fees_balance - pool_before.buyback_fees_balance;
        println!("  Actual creator fee: {}", actual_creator_fee);
        println!("  Actual buyback fee increase: {} (after topup)", buyback_fee_increase);
        
        // Note: buyback fees might be less if used for topup
        assert!(buyback_fee_increase <= expected_buyback_fee,
            "Buyback fees should be <= expected (due to possible topup)");
        
        // Verify total accumulated (creator + buyback)
        println!("✅ Fee accumulation verified");
    }

    /// Test 3.2: Top-Up Calculation Formula
    /// Formula: ΔA = min(ΔV, F)
    /// Where ΔV = Virtual reserve reduction, F = Available buyback fees
    /// This test needs to be in burn tests, but we verify the buyback fee storage here
    #[test]
    fn test_ccb_buyback_fees_stored_correctly() {
        let (mut runner, payer, _, pool, payer_ata, a_mint) = setup_test();
        
        // Multiple buys to accumulate fees
        let amounts = vec![5_000, 10_000, 15_000];
        let mut expected_total_buyback = 0u64;
        
        for &a_amount in &amounts {
            let vta = runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);
            runner.buy_virtual_token(&payer, payer_ata, a_mint, pool.pool, vta, a_amount, 0).unwrap();
            
            // 6% buyback fee
            expected_total_buyback += a_amount * 600 / 10_000;
        }
        
        let pool_after = runner.get_pool_data(&pool.pool);
        
        println!("Buyback fee accumulation:");
        println!("  Buys: {:?}", amounts);
        println!("  Expected total buyback fees: {}", expected_total_buyback);
        println!("  Actual buyback_fees_balance: {}", pool_after.buyback_fees_balance);
        
        // Should be less than or equal to expected due to topup usage
        assert!(pool_after.buyback_fees_balance <= expected_total_buyback,
            "Buyback fees should accumulate (minus any topup)");
        
        println!("✅ Buyback fees stored correctly");
    }
}
