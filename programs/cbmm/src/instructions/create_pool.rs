use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};


#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePoolArgs {
    /// a_virtual_reserve is the virtual reserve of the A mint including decimals
    pub a_virtual_reserve: u64,
}
#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub a_mint: InterfaceAccount<'info, Mint>,    
    
    #[account(init,
         payer = payer, 
         space = BcpmmPool::INIT_SPACE + 8,
         seeds = [BCPMM_POOL_SEED, BCPMM_POOL_INDEX_SEED.to_le_bytes().as_ref(), payer.key().as_ref()],
         bump
    )]
    pub pool: Account<'info, BcpmmPool>,        

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = a_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program        
    )]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,    

    #[account(mut)]
    pub central_state: Account<'info, CentralState>,

    #[account(
        init_if_needed, 
        payer = payer, 
        associated_token::mint = a_mint, 
        associated_token::authority = central_state, 
        associated_token::token_program = token_program
    )]
    pub central_state_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn create_pool(ctx: Context<CreatePool>, args: CreatePoolArgs) -> Result<()> {    
    let central_state = &ctx.accounts.central_state;
    ctx.accounts.pool.set_inner(BcpmmPool::try_new(
        ctx.bumps.pool,
        ctx.accounts.payer.key(),
        BCPMM_POOL_INDEX_SEED,
        ctx.accounts.a_mint.key(),
        args.a_virtual_reserve,
        central_state.creator_fee_basis_points,
        central_state.buyback_fee_basis_points,
        central_state.platform_fee_basis_points,
    )?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestRunner;
    use solana_sdk::signature::{Keypair, Signer};

    // Helper function to set up test environment with CentralState
    fn setup_with_central_state(runner: &mut TestRunner, payer: &Keypair) {
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        runner.create_central_state_mock(
            payer,
            10,    // max_user_daily_burn_count
            5,     // max_creator_daily_burn_count
            1000,  // user_burn_bp_x100
            500,   // creator_burn_bp_x100
            43200, // burn_reset_time (noon)
            100,   // creator_fee_basis_points (1%)
            200,   // buyback_fee_basis_points (2%)
            300,   // platform_fee_basis_points (3%)
        );
    }

    #[test]
    fn test_create_pool_basic() {
        // 1. ARRANGE - Set up test environment
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        setup_with_central_state(&mut runner, &payer);

        // Create a mint for the pool
        let a_mint = runner.create_mint(&payer, 9); // 9 decimals

        // 2. ACT - Create the pool using the actual instruction
        let a_virtual_reserve = 1_000_000_000; // 1 token with 9 decimals
        let pool_pda = runner
            .create_pool(&payer, a_mint, a_virtual_reserve)
            .expect(&format!(
                "Should successfully create pool for mint {} with virtual_reserve {}",
                a_mint, a_virtual_reserve
            ));

        // 3. ASSERT - Verify the pool was created correctly
        let account = runner
            .svm
            .get_account(&pool_pda)
            .expect("Pool account should exist");
        
        // Verify account ownership
        assert_eq!(
            account.owner, runner.program_id,
            "Pool should be owned by the program"
        );
        
        // Verify account size
        assert_eq!(
            account.data.len(),
            crate::state::BcpmmPool::INIT_SPACE + 8,
            "Pool account size should match INIT_SPACE + 8 (discriminator)"
        );
        
        let pool_data = crate::state::BcpmmPool::try_deserialize(&mut account.data.as_slice())
            .expect("Should deserialize pool data");

        // Verify pool fields
        assert_eq!(
            pool_data.creator,
            anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes()),
            "Pool creator should match payer"
        );
        assert_eq!(pool_data.pool_index, crate::state::BCPMM_POOL_INDEX_SEED);
        assert_eq!(
            pool_data.a_mint,
            anchor_lang::prelude::Pubkey::from(a_mint.to_bytes())
        );
        assert_eq!(pool_data.a_virtual_reserve, a_virtual_reserve);
        assert_eq!(pool_data.a_reserve, 0, "Initial a_reserve should be 0");
        assert_eq!(pool_data.b_reserve, crate::state::DEFAULT_B_MINT_RESERVE, "b_reserve should be default value");
        assert_eq!(pool_data.b_mint_decimals, 6, "Virtual token should have 6 decimals");
        assert_eq!(pool_data.creator_fees_balance, 0);
        assert_eq!(pool_data.buyback_fees_balance, 0);
        assert_eq!(pool_data.a_outstanding_topup, 0);
        assert_eq!(pool_data.burns_today, 0);
        assert_eq!(pool_data.last_burn_timestamp, 0);
        assert_eq!(pool_data.creator_fee_basis_points, 100, "Creator fee should be 1%");
        assert_eq!(pool_data.buyback_fee_basis_points, 200, "Buyback fee should be 2%");
        assert_eq!(pool_data.platform_fee_basis_points, 300, "Platform fee should be 3%");

        println!("✅ Pool created successfully using actual instruction!");
    }

    #[test]
    fn test_create_pool_fails_without_central_state() {
        // 1. ARRANGE - No CentralState initialized
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        let a_mint = runner.create_mint(&payer, 9);
        
        // 2. ACT - Try to create pool WITHOUT initializing central state
        let result = runner.create_pool(&payer, a_mint, 1_000_000_000);
        
        // 3. ASSERT - Should fail
        assert!(result.is_err(), "Should fail when CentralState doesn't exist");
        
        println!("✅ Correctly requires CentralState to exist!");
    }

    #[test]
    fn test_create_pool_with_minimum_virtual_reserve() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        setup_with_central_state(&mut runner, &payer);
        
        let a_mint = runner.create_mint(&payer, 9);
        
        // 2. ACT - Test with virtual reserve = 1 (minimum valid value)
        let pool_pda = runner.create_pool(&payer, a_mint, 1)
            .expect("Should create pool with virtual_reserve = 1");
        
        // 3. ASSERT
        let account = runner.svm.get_account(&pool_pda).unwrap();
        let pool_data = crate::state::BcpmmPool::try_deserialize(&mut account.data.as_slice()).unwrap();
        
        assert_eq!(pool_data.a_virtual_reserve, 1);
        
        println!("✅ Minimum virtual reserve (1) works correctly!");
    }

    #[test]
    fn test_create_pool_fails_with_zero_virtual_reserve() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        setup_with_central_state(&mut runner, &payer);
        
        let a_mint = runner.create_mint(&payer, 9);
        
        // 2. ACT - Should fail with a_virtual_reserve = 0
        let result = runner.create_pool(&payer, a_mint, 0);
        
        // 3. ASSERT
        assert!(result.is_err(), "Should fail with zero virtual reserve");
        let error_msg = result.unwrap_err().message;
        assert!(
            error_msg.contains("InvalidVirtualReserve") || error_msg.contains("6000"),
            "Expected InvalidVirtualReserve error, got: {}",
            error_msg
        );
        
        println!("✅ Correctly rejects zero virtual reserve!");
    }

    #[test]
    fn test_create_pool_with_various_mint_decimals() {
        // Test that pools can be created with mints of different decimal places
        // Need different creators because pool_index is hardcoded
        let mut runner = TestRunner::new();
        
        // Initialize CentralState once
        let first_payer = Keypair::new();
        setup_with_central_state(&mut runner, &first_payer);
        
        for decimals in [0, 6, 9, 18] {
            let payer = Keypair::new();  // Different payer for each pool
            runner.airdrop(&payer.pubkey(), 10_000_000_000);
            
            let a_mint = runner.create_mint(&payer, decimals);
            let pool_pda = runner.create_pool(&payer, a_mint, 1_000_000_000)
                .expect(&format!("Should create pool with {} decimals", decimals));
            
            let account = runner.svm.get_account(&pool_pda).unwrap();
            let pool_data = crate::state::BcpmmPool::try_deserialize(&mut account.data.as_slice()).unwrap();
            
            assert_eq!(
                pool_data.a_mint,
                anchor_lang::prelude::Pubkey::from(a_mint.to_bytes()),
                "Mint with {} decimals should be stored correctly",
                decimals
            );
            
            // B mint decimals should always be 6 (virtual token)
            assert_eq!(pool_data.b_mint_decimals, 6);
            
            println!("✅ Pool with {} decimal mint created successfully!", decimals);
        }
    }

    #[test]
    fn test_create_pool_with_large_virtual_reserve() {
        // Test with a very large virtual reserve value
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        setup_with_central_state(&mut runner, &payer);
        
        let a_mint = runner.create_mint(&payer, 9);
        let large_reserve = u64::MAX;
        
        let pool_pda = runner.create_pool(&payer, a_mint, large_reserve)
            .expect("Should create pool with maximum u64 virtual reserve");
        
        let account = runner.svm.get_account(&pool_pda).unwrap();
        let pool_data = crate::state::BcpmmPool::try_deserialize(&mut account.data.as_slice()).unwrap();
        
        assert_eq!(pool_data.a_virtual_reserve, u64::MAX);
        
        println!("✅ Large virtual reserve handled correctly!");
    }

    #[test]
    fn test_create_pool_fees_copied_from_central_state() {
        // Verify that fee basis points are correctly copied from CentralState
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        // Create mock with specific fees
        runner.create_central_state_mock(
            &payer,
            10, 5, 1000, 500, 43200,
            250,  // 2.5% creator fee
            500,  // 5% buyback fee
            750,  // 7.5% platform fee
        );
        
        let a_mint = runner.create_mint(&payer, 9);
        let pool_pda = runner.create_pool(&payer, a_mint, 1_000_000_000)
            .expect("Should create pool");
        
        let account = runner.svm.get_account(&pool_pda).unwrap();
        let pool_data = crate::state::BcpmmPool::try_deserialize(&mut account.data.as_slice()).unwrap();
        
        // Verify fees match CentralState values
        assert_eq!(pool_data.creator_fee_basis_points, 250);
        assert_eq!(pool_data.buyback_fee_basis_points, 500);
        assert_eq!(pool_data.platform_fee_basis_points, 750);
        
        println!("✅ Pool fees correctly copied from CentralState!");
    }

    #[test]
    fn test_create_multiple_pools_same_creator() {
        // Currently pool_index is hardcoded to 0, so multiple pools would fail
        // This test documents current behavior
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        setup_with_central_state(&mut runner, &payer);
        
        let mint1 = runner.create_mint(&payer, 9);
        let mint2 = runner.create_mint(&payer, 6);
        
        // Create first pool
        let _pool1 = runner.create_pool(&payer, mint1, 1_000_000_000)
            .expect("First pool should succeed");
        
        // Try to create second pool - will fail because pool_index is same
        let result = runner.create_pool(&payer, mint2, 1_000_000_000);
        
        assert!(result.is_err(), "Second pool should fail with same creator (pool_index collision)");
        
        println!("✅ Correctly prevents duplicate pool creation (current behavior with fixed pool_index)!");
    }
}