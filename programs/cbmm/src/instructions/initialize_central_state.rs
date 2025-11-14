use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeCentralStateArgs {
    pub admin: Pubkey,
    pub max_user_daily_burn_count: u16,
    pub max_creator_daily_burn_count: u16,
    pub user_burn_bp_x100: u32,
    pub creator_burn_bp_x100: u32,
    pub burn_reset_time_of_day_seconds: u32, // Seconds from midnight
    pub creator_fee_basis_points: u16,
    pub buyback_fee_basis_points: u16,
    pub platform_fee_basis_points: u16,
}

#[derive(Accounts)]
pub struct InitializeCentralState<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = CentralState::INIT_SPACE + 8, seeds = [CENTRAL_STATE_SEED], bump)]
    pub central_state: Account<'info, CentralState>,
    pub system_program: Program<'info, System>,
    #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()))]
    pub program_data: Account<'info, ProgramData>,
    //// CHECK: Validation skipped in tests. In production, should verify upgrade_authority_address.
    // pub program_data: AccountInfo<'info>,
}

pub fn initialize_central_state(
    ctx: Context<InitializeCentralState>,
    args: InitializeCentralStateArgs,
) -> Result<()> {
    ctx.accounts.central_state.set_inner(CentralState::new(
        ctx.bumps.central_state,
        args.admin,
        args.max_user_daily_burn_count,
        args.max_creator_daily_burn_count,
        args.user_burn_bp_x100,
        args.creator_burn_bp_x100,
        args.burn_reset_time_of_day_seconds,
        args.creator_fee_basis_points,
        args.buyback_fee_basis_points,
        args.platform_fee_basis_points,
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestRunner;
    use solana_sdk::signature::{Keypair, Signer};
    
    #[test]
    fn test_initialize_central_state_basic() {
        // 1. ARRANGE - Set up test environment
        let mut runner = TestRunner::new();
        let authority = Keypair::new();
        let admin = Keypair::new(); // Admin can be different from authority
        runner.airdrop(&authority.pubkey(), 10_000_000_000);
        
        // 2. ACT - Call the actual instruction using the helper from test_runner
        let central_state_pda = runner.initialize_central_state(
            &authority,
            admin.pubkey(),
            10,    // max_user_daily_burn_count
            5,     // max_creator_daily_burn_count
            1000,  // user_burn_bp_x100
            500,   // creator_burn_bp_x100
            43200, // burn_reset_time (noon)
            100,   // creator_fee_basis_points
            200,   // buyback_fee_basis_points
            300,   // platform_fee_basis_points
        ).expect("Should successfully initialize central state");
        
        // 3. ASSERT - Verify results
        let account = runner.svm.get_account(&central_state_pda)
            .expect("CentralState account should exist");
        
        // Verify account ownership
        assert_eq!(
            account.owner, runner.program_id,
            "CentralState should be owned by the program"
        );
        
        // Verify account size
        assert_eq!(
            account.data.len(),
            CentralState::INIT_SPACE + 8,
            "Account size should match INIT_SPACE + 8 (discriminator)"
        );
        
        // Verify that the account data can be deserialized
        let data = crate::state::CentralState::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        // Verify ALL fields match what we sent
        assert_eq!(data.max_user_daily_burn_count, 10);
        assert_eq!(data.max_creator_daily_burn_count, 5);
        assert_eq!(data.user_burn_bp_x100, 1000);
        assert_eq!(data.creator_burn_bp_x100, 500);
        assert_eq!(data.burn_reset_time_of_day_seconds, 43200);
        assert_eq!(data.creator_fee_basis_points, 100, "Creator fee should be 100 bps (1%)");
        assert_eq!(data.buyback_fee_basis_points, 200, "Buyback fee should be 200 bps (2%)");
        assert_eq!(data.platform_fee_basis_points, 300, "Platform fee should be 300 bps (3%)");
        assert_eq!(
            data.admin,
            anchor_lang::prelude::Pubkey::from(admin.pubkey().to_bytes())
        );
        
        println!("✅ Central state initialized successfully using actual instruction!");
    }

    #[test]
    fn test_initialize_central_state_fails_when_already_initialized() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let authority = Keypair::new();
        runner.airdrop(&authority.pubkey(), 10_000_000_000);
        
        // Initialize once
        runner.initialize_central_state(
            &authority, 
            authority.pubkey(), 
            10, 5, 1000, 500, 43200, 100, 200, 300
        ).expect("First initialization should succeed");
        
        // 2. ACT - Try to initialize again
        let result = runner.initialize_central_state(
            &authority,
            authority.pubkey(),
            10, 5, 1000, 500, 43200, 100, 200, 300
        );
        
        // 3. ASSERT - Should fail
        assert!(result.is_err(), "Should not allow re-initialization of CentralState");
        
        println!("✅ Correctly prevented double initialization!");
    }

    #[test]
    fn test_initialize_central_state_with_edge_values() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let authority = Keypair::new();
        runner.airdrop(&authority.pubkey(), 10_000_000_000);
        
        // 2. ACT - Test with maximum and edge values
        let central_state_pda = runner.initialize_central_state(
            &authority,
            authority.pubkey(),
            u16::MAX,  // Max burn count
            u16::MAX,
            u32::MAX,  // Max burn bp
            u32::MAX,
            86399,     // One second before midnight (23:59:59)
            10000,     // 100% fee (edge case)
            0,         // 0% fee (minimum)
            5000,      // 50% fee
        ).expect("Should succeed with edge values");
        
        // 3. ASSERT
        let account = runner.svm.get_account(&central_state_pda)
            .expect("CentralState account should exist");
        let data = crate::state::CentralState::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(data.max_user_daily_burn_count, u16::MAX);
        assert_eq!(data.max_creator_daily_burn_count, u16::MAX);
        assert_eq!(data.user_burn_bp_x100, u32::MAX);
        assert_eq!(data.creator_burn_bp_x100, u32::MAX);
        assert_eq!(data.burn_reset_time_of_day_seconds, 86399);
        assert_eq!(data.creator_fee_basis_points, 10000);
        assert_eq!(data.buyback_fee_basis_points, 0);
        assert_eq!(data.platform_fee_basis_points, 5000);
        
        println!("✅ Edge values handled correctly!");
    }

    #[test]
    fn test_initialize_central_state_with_minimum_values() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let authority = Keypair::new();
        runner.airdrop(&authority.pubkey(), 10_000_000_000);
        
        // 2. ACT - Test with minimum values
        let central_state_pda = runner.initialize_central_state(
            &authority,
            authority.pubkey(),
            0,  // Min burn count
            0,
            0,  // Min burn bp
            0,
            0,  // Midnight
            0,  // 0% fees
            0,
            0,
        ).expect("Should succeed with minimum values");
        
        // 3. ASSERT
        let account = runner.svm.get_account(&central_state_pda).unwrap();
        let data = crate::state::CentralState::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(data.max_user_daily_burn_count, 0);
        assert_eq!(data.burn_reset_time_of_day_seconds, 0);
        assert_eq!(data.creator_fee_basis_points, 0);
        
        println!("✅ Minimum values handled correctly!");
    }

    #[test]
    fn test_initialize_central_state_admin_different_from_authority() {
        // Test that admin can be a different address than authority
        let mut runner = TestRunner::new();
        let authority = Keypair::new();
        let admin = Keypair::new();  // Different from authority
        runner.airdrop(&authority.pubkey(), 10_000_000_000);
        
        let central_state_pda = runner.initialize_central_state(
            &authority,
            admin.pubkey(),  // Admin is different
            10, 5, 1000, 500, 43200, 100, 200, 300
        ).expect("Should allow different admin");
        
        let account = runner.svm.get_account(&central_state_pda).unwrap();
        let data = crate::state::CentralState::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(
            data.admin,
            anchor_lang::prelude::Pubkey::from(admin.pubkey().to_bytes())
        );
        assert_ne!(
            data.admin,
            anchor_lang::prelude::Pubkey::from(authority.pubkey().to_bytes()),
            "Admin should be different from authority"
        );
        
        println!("✅ Admin can be different from authority!");
    }

    #[test]
    fn test_initialize_central_state_unauthorized_fails() {
        // 1. ARRANGE - Set up test environment
        let mut runner = TestRunner::new();
        let upgrade_authority = Keypair::new();
        let unauthorized_caller = Keypair::new();
        let admin = Keypair::new();
        
        // Airdrop to both accounts
        runner.airdrop(&upgrade_authority.pubkey(), 10_000_000_000);
        runner.airdrop(&unauthorized_caller.pubkey(), 10_000_000_000);
        
        // Create a mock ProgramData account with the real upgrade authority
        let program_data_pda = runner.create_program_data_mock(&upgrade_authority.pubkey());
        
        // 2. ACT - Try to call with unauthorized caller
        let result = runner.initialize_central_state_with_program_data(
            &unauthorized_caller,  // ❌ Wrong authority (not the upgrade authority)
            admin.pubkey(),
            10,    // max_user_daily_burn_count
            5,     // max_creator_daily_burn_count
            1000,  // user_burn_bp_x100
            500,   // creator_burn_bp_x100
            43200, // burn_reset_time (noon)
            100,   // creator_fee_basis_points
            200,   // buyback_fee_basis_points
            300,   // platform_fee_basis_points
            program_data_pda,
        );
        
        // 3. ASSERT - Verify the transaction failed
        assert!(
            result.is_err(),
            "Should fail when called by unauthorized account"
        );
        
        let error_message = result.unwrap_err().message;
        assert!(
            error_message.contains("ConstraintRaw") || error_message.contains("2003"),
            "Expected ConstraintRaw error (code 2003), got: {}",
            error_message
        );
        
        println!("✅ Authorization check correctly rejected unauthorized caller!");
    }

    #[test]
    fn test_initialize_central_state_with_correct_authority_succeeds() {
        // 1. ARRANGE - Set up test environment
        let mut runner = TestRunner::new();
        let upgrade_authority = Keypair::new();
        let admin = Keypair::new();
        
        runner.airdrop(&upgrade_authority.pubkey(), 10_000_000_000);
        
        // Create a mock ProgramData account with the upgrade authority
        let program_data_pda = runner.create_program_data_mock(&upgrade_authority.pubkey());
        
        // 2. ACT - Call with the correct upgrade authority
        let central_state_pda = runner.initialize_central_state_with_program_data(
            &upgrade_authority,  // ✅ Correct authority
            admin.pubkey(),
            10,
            5,
            1000,
            500,
            43200,
            100,
            200,
            300,
            program_data_pda,
        ).expect("Should succeed with correct upgrade authority");
        
        // 3. ASSERT - Verify the account was created
        let account = runner.svm.get_account(&central_state_pda)
            .expect("CentralState account should exist");
        let data = crate::state::CentralState::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(data.creator_fee_basis_points, 100);
        
        println!("✅ Authorization check correctly accepted valid upgrade authority!");
    }
}

