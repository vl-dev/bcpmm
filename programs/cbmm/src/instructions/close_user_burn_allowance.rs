use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CloseUserBurnAllowanceArgs {
    pub pool_owner: bool,
}

#[derive(Accounts)]
#[instruction(args: CloseUserBurnAllowanceArgs)]
pub struct CloseUserBurnAllowance<'info> {
    /// The user whose burn allowance is being closed
    /// CHECK: Can be any account.
    pub owner: UncheckedAccount<'info>,

    #[account(
        mut,
        close = burn_allowance_open_payer,
        seeds = [USER_BURN_ALLOWANCE_SEED, owner.key().as_ref(), &[args.pool_owner as u8]],
        bump
    )]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    /// CHECK: Checked that it's the same as the payer in the user burn allowance account.
    #[account(address = user_burn_allowance.payer @ BcpmmError::InvalidBurnAccountPayer)]
    pub burn_allowance_open_payer: AccountInfo<'info>,

    #[account(seeds = [CENTRAL_STATE_SEED], bump)]
    pub central_state: Account<'info, CentralState>,
}

pub fn close_user_burn_allowance(
    ctx: Context<CloseUserBurnAllowance>,
    _args: CloseUserBurnAllowanceArgs,
) -> Result<()> {
    // Only allow closing if the burn allowance is inactive: past the reset window and previous burn was before the reset.
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.central_state.is_after_burn_reset(now)?
            && !ctx
                .accounts
                .central_state
                .is_after_burn_reset(ctx.accounts.user_burn_allowance.last_burn_timestamp)?,
        BcpmmError::CannotCloseActiveBurnAllowance
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_utils::TestRunner;
    use solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_close_user_burn_allowance_inactive() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let user = Keypair::new().pubkey();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        // Create CentralState with reset time at noon (12:00 = 43200 seconds)
        runner.create_central_state_mock(
            &payer,
            10,    // daily_burn_allowance
            10,    // creator_daily_burn_allowance
            1000,  // user_burn_bp_x100
            2000,  // creator_burn_bp_x100
            43200, // burn_reset_time_of_day_seconds (noon)
            100,   // creator_fee_basis_points
            100,   // buyback_fee_basis_points
            100,   // platform_fee_basis_points
        );
        
        // Simulate: Current time is 1 PM, last burn was 11 AM (before reset)
        // This means the burn allowance is "inactive" and can be closed
        let midnight = 1_700_000_000 - (1_700_000_000 % 86400); // Round to midnight
        let last_burn_time = midnight + 11 * 3600; // 11:00 AM (before reset)
        let current_time = midnight + 13 * 3600;   // 1:00 PM (after reset)
        
        // Create burn allowance that was last used before reset
        let uba_pda = runner.create_user_burn_allowance_mock(
            user,
            payer.pubkey(),
            5,  // burns_today
            last_burn_time,
            false, // not pool owner
        );
        
        // Mock the clock to current_time
        runner.svm.set_sysvar(&solana_sdk::sysvar::clock::Clock {
            slot: 0,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: current_time,
        });
        
        let payer_balance_before = runner.svm.get_balance(&payer.pubkey()).unwrap();
        
        // 2. ACT - Close the inactive burn allowance
        runner.close_user_burn_allowance(&payer, user, false)
            .expect("Should close inactive burn allowance");
        
        // 3. ASSERT
        // Verify account is closed
        let account = runner.svm.get_account(&uba_pda);
        assert!(account.is_none(), "UserBurnAllowance should be closed");
        
        // Verify payer received rent refund
        let payer_balance_after = runner.svm.get_balance(&payer.pubkey()).unwrap();
        assert!(
            payer_balance_after > payer_balance_before,
            "Payer should receive rent refund"
        );
        
        println!("✅ Inactive burn allowance closed successfully!");
    }

    #[test]
    fn test_close_user_burn_allowance_fails_when_active() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let user = Keypair::new().pubkey();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        // Create CentralState with reset time at noon
        runner.create_central_state_mock(
            &payer,
            10, 10, 1000, 2000,
            43200, // burn_reset_time_of_day_seconds (noon)
            100, 100, 100,
        );
        
        // Simulate: Current time is 1 PM, last burn was 12:30 PM (AFTER reset)
        // This means the burn allowance is "active" and cannot be closed
        let midnight = 1_700_000_000 - (1_700_000_000 % 86400);
        let last_burn_time = midnight + 12 * 3600 + 1800; // 12:30 PM (after reset)
        let current_time = midnight + 13 * 3600;          // 1:00 PM (after reset)
        
        let uba_pda = runner.create_user_burn_allowance_mock(
            user,
            payer.pubkey(),
            3,
            last_burn_time,
            false,
        );
        
        runner.svm.set_sysvar(&solana_sdk::sysvar::clock::Clock {
            slot: 0,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: current_time,
        });
        
        // 2. ACT - Try to close active burn allowance
        let result = runner.close_user_burn_allowance(&payer, user, false);
        
        // 3. ASSERT - Should fail
        assert!(result.is_err(), "Should fail to close active burn allowance");
        
        let error_msg = result.unwrap_err().message;
        assert!(
            error_msg.contains("CannotCloseActiveBurnAllowance") || error_msg.contains("6006"),
            "Expected CannotCloseActiveBurnAllowance error, got: {}",
            error_msg
        );
        
        // Verify account still exists
        let account = runner.svm.get_account(&uba_pda);
        assert!(account.is_some(), "Account should still exist");
        
        println!("✅ Correctly prevents closing active burn allowance!");
    }

    #[test]
    fn test_close_user_burn_allowance_pool_owner_vs_user() {
        // Test that pool_owner flag creates different PDAs
        let mut runner = TestRunner::new();
        let user = Keypair::new().pubkey();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(
            &payer,
            10, 10, 1000, 2000,
            43200, // burn_reset_time_of_day_seconds (noon)
            100, 100, 100,
        );
        
        let midnight = 1_700_000_000 - (1_700_000_000 % 86400);
        let last_burn_time = midnight + 11 * 3600; // Before reset
        let current_time = midnight + 13 * 3600;   // After reset
        
        // Create two burn allowances: one for user, one for pool owner
        let uba_user_pda = runner.create_user_burn_allowance_mock(
            user,
            payer.pubkey(),
            2,
            last_burn_time,
            false, // pool_owner = false
        );
        
        let uba_pool_owner_pda = runner.create_user_burn_allowance_mock(
            user,
            payer.pubkey(),
            5,
            last_burn_time,
            true, // pool_owner = true
        );
        
        // Verify they have different addresses
        assert_ne!(
            uba_user_pda, uba_pool_owner_pda,
            "User and pool owner burn allowances should have different PDAs"
        );
        
        runner.svm.set_sysvar(&solana_sdk::sysvar::clock::Clock {
            slot: 0,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: current_time,
        });
        
        // Close user burn allowance
        runner.close_user_burn_allowance(&payer, user, false)
            .expect("Should close user burn allowance");
        
        // Verify user one is closed, pool owner one still exists
        assert!(runner.svm.get_account(&uba_user_pda).is_none());
        assert!(runner.svm.get_account(&uba_pool_owner_pda).is_some());
        
        // Close pool owner burn allowance
        runner.close_user_burn_allowance(&payer, user, true)
            .expect("Should close pool owner burn allowance");
        
        assert!(runner.svm.get_account(&uba_pool_owner_pda).is_none());
        
        println!("✅ Different PDAs for pool_owner flag work correctly!");
    }

    #[test]
    fn test_close_user_burn_allowance_fails_without_central_state() {
        // Test that closing fails if CentralState doesn't exist
        let mut runner = TestRunner::new();
        let user = Keypair::new().pubkey();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        let midnight = 1_700_000_000 - (1_700_000_000 % 86400);
        let last_burn_time = midnight + 11 * 3600;
        let current_time = midnight + 13 * 3600;
        
        // Create burn allowance WITHOUT CentralState
        runner.create_user_burn_allowance_mock(
            user,
            payer.pubkey(),
            2,
            last_burn_time,
            false,
        );
        
        runner.svm.set_sysvar(&solana_sdk::sysvar::clock::Clock {
            slot: 0,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: current_time,
        });
        
        // Try to close without CentralState
        let result = runner.close_user_burn_allowance(&payer, user, false);
        
        assert!(result.is_err(), "Should fail without CentralState");
        
        println!("✅ Correctly requires CentralState!");
    }

    #[test]
    fn test_close_user_burn_allowance_before_reset_time() {
        // Test closing when current time is BEFORE reset (should fail)
        let mut runner = TestRunner::new();
        let user = Keypair::new().pubkey();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        // Reset at noon
        runner.create_central_state_mock(
            &payer,
            10, 10, 1000, 2000,
            43200, // burn_reset_time_of_day_seconds (noon)
            100, 100, 100,
        );
        
        let midnight = 1_700_000_000 - (1_700_000_000 % 86400);
        let last_burn_time = midnight + 8 * 3600;  // 8:00 AM (before reset)
        let current_time = midnight + 10 * 3600;   // 10:00 AM (ALSO before reset!)
        
        runner.create_user_burn_allowance_mock(
            user,
            payer.pubkey(),
            3,
            last_burn_time,
            false,
        );
        
        runner.svm.set_sysvar(&solana_sdk::sysvar::clock::Clock {
            slot: 0,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: current_time,
        });
        
        // Try to close when current time is before reset
        let result = runner.close_user_burn_allowance(&payer, user, false);
        
        // Should fail because current time is before reset
        assert!(result.is_err(), "Should fail when current time is before reset");
        
        println!("✅ Cannot close before reset time!");
    }
}
