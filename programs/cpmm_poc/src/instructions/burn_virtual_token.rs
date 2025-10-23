use crate::helpers::{calculate_burn_amount, calculate_new_virtual_reserve};
use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct BurnVirtualToken<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut, seeds = [BCPMM_POOL_SEED, pool.b_mint_index.to_le_bytes().as_ref()], bump = pool.bump)]
    pub pool: Account<'info, BcpmmPool>,

    #[account(mut, seeds = [USER_BURN_ALLOWANCE_SEED, signer.key().as_ref()], bump)]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>, // separate init

    #[account(mut, seeds = [CENTRAL_STATE_SEED], bump)]
    pub central_state: Account<'info, CentralState>,
}

pub fn burn_virtual_token(
    ctx: Context<BurnVirtualToken>,
) -> Result<()> {

    let is_pool_owner = ctx.accounts.pool.creator == ctx.accounts.signer.key();
    let burn_bp_x100 = if is_pool_owner { ctx.accounts.central_state.creator_burn_bp_x100 } else { ctx.accounts.central_state.user_burn_bp_x100 };

    // Check if we should reset the daily burn count
    // We reset it if we have passed the burn reset window and previous burn was before the reset
    let now = Clock::get()?.unix_timestamp;
    if ctx.accounts.central_state.is_after_burn_reset(now)? &&
      !ctx.accounts.central_state.is_after_burn_reset(ctx.accounts.user_burn_allowance.last_burn_timestamp)? {

        ctx.accounts.user_burn_allowance.burns_today = 1;

    // If not resetting, check we have enough burn allowance.
    } else if ctx.accounts.user_burn_allowance.burns_today >= ctx.accounts.central_state.daily_burn_allowance {
        return Err(BcpmmError::InsufficientBurnAllowance.into());

    // Not resetting and enough allowance, increment the burn count for today.
    } else {
        ctx.accounts.user_burn_allowance.burns_today += 1;
    }
    ctx.accounts.user_burn_allowance.last_burn_timestamp = now;

    // Check if we should reset the pool's daily burn count
    if ctx.accounts.central_state.is_after_burn_reset(now)? &&
      !ctx.accounts.central_state.is_after_burn_reset(ctx.accounts.pool.last_burn_timestamp)? {
        ctx.accounts.pool.burns_today = 1;

    // Not resetting so just increment the burn count for today.
    } else {
        ctx.accounts.pool.burns_today += 1;
    }
    ctx.accounts.pool.last_burn_timestamp = now;

    let burn_amount =
        calculate_burn_amount(burn_bp_x100, ctx.accounts.pool.b_reserve);
    let new_virtual_reserve = calculate_new_virtual_reserve(
        ctx.accounts.pool.a_virtual_reserve,
        ctx.accounts.pool.b_reserve,
        burn_amount,
    );

    // Update the pool state
    ctx.accounts.pool.a_remaining_topup +=
        ctx.accounts.pool.a_virtual_reserve - new_virtual_reserve;
    ctx.accounts.pool.a_virtual_reserve = new_virtual_reserve;
    ctx.accounts.pool.b_reserve -= burn_amount;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::helpers::{calculate_buy_output_amount, calculate_fees};
    use crate::state::BcpmmPool;
    use crate::test_utils::TestRunner;
    use solana_sdk::clock::Clock;
    use anchor_lang::prelude::*;
    use solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_burn_virtual_token() {
        // Parameters
        let a_reserve = 1_000_000;
        let a_virtual_reserve = 500_000;
        let b_reserve = 1_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;

        let mut runner = TestRunner::new();

        let payer = Keypair::new();
        runner.create_central_state_mock(
            &payer,
            5,
            5,
            20,
            10,
            36_000, // 10AM
        );
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let pool = runner.create_pool_mock(
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
        
        // Burn at a certain timestamp
        runner.set_system_clock(1682899200);
        let user_burn_allowance = runner.initialize_user_burn_allowance(
            &payer, payer.pubkey()).unwrap();

        let burn_result = runner.burn_virtual_token(
            &payer,
            pool.pool,
            user_burn_allowance,
        );
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 999990);
        let user_burn_allowance_data = runner.get_user_burn_allowance(&user_burn_allowance).unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 1);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682899200);
    }

    #[test]
    fn test_burn_virtual_token_twice() {
        // Parameters
        let a_reserve = 1_000_000;
        let a_virtual_reserve = 500_000;
        let b_reserve = 1_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;

        let mut runner = TestRunner::new();

        let payer = Keypair::new();
        runner.create_central_state_mock(
            &payer,
            5,
            5,
            20,
            10,
            36_000, // 10AM
        );
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let pool = runner.create_pool_mock(
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
        
        // Set up user burn allowance with 1 burn already recorded (1 hour ago)
        let one_hour_ago = 1682899200 - 3600; // 1 hour before the test timestamp
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            payer.pubkey(),
            payer.pubkey(),
            1, // burns_today = 1
            one_hour_ago, // last_burn_timestamp = 1 hour ago
        );

        // Burn at current timestamp
        runner.set_system_clock(1682899200);
        let burn_result = runner.burn_virtual_token(
            &payer,
            pool.pool,
            user_burn_allowance,
        );
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 999990);
        
        // Check that user burn allowance shows 2 burns for today
        let user_burn_allowance_data = runner.get_user_burn_allowance(&user_burn_allowance).unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 2);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682899200);
    }

    #[test]
    fn test_burn_virtual_token_after_reset() {
        // Parameters
        let a_reserve = 1_000_000;
        let a_virtual_reserve = 500_000;
        let b_reserve = 1_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;

        let mut runner = TestRunner::new();

        let payer = Keypair::new();
        runner.create_central_state_mock(
            &payer,
            5,
            5,
            20,
            10,
            36_000, // 10AM
        );
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let pool = runner.create_pool_mock(
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
        
        // Set up user burn allowance with 1 burn already recorded
        let one_hour_ago = 1682899200;
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            payer.pubkey(),
            payer.pubkey(),
            1, // burns_today = 1
            one_hour_ago, // last_burn_timestamp = 1 hour ago
        );

        // Burn at 10:00:01 AM
        runner.set_system_clock( 1682935201);
        let burn_result = runner.burn_virtual_token(
            &payer,
            pool.pool,
            user_burn_allowance,
        );
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 999990);
        
        // Check that user burn allowance was reset
        let user_burn_allowance_data = runner.get_user_burn_allowance(&user_burn_allowance).unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 1);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682935201);
    }

    #[test]
   fn test_burn_virtual_token_past_limit() {
        // Parameters
        let a_reserve = 1_000_000;
        let a_virtual_reserve = 500_000;
        let b_reserve = 1_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;

        let mut runner = TestRunner::new();

        let payer = Keypair::new();
        runner.create_central_state_mock(
            &payer,
            5,
            5,
            20,
            10,
            36_000, // 10AM
        );
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let pool = runner.create_pool_mock(
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
        
        // Set up user burn allowance with 5 burns already recorded (1 hour ago)
        let one_hour_ago = 1682899200 - 3600; // 1 hour before the test timestamp
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            payer.pubkey(),
            payer.pubkey(),
            5,
            one_hour_ago,
        );

        // Burn at current timestamp
        runner.set_system_clock(1682899200);
        let burn_result = runner.burn_virtual_token(
            &payer,
            pool.pool,
            user_burn_allowance,
        );
        assert!(burn_result.is_err());
    }

    #[test]
   fn test_burn_virtual_token_past_limit_after_reset() {
        // Parameters
        let a_reserve = 1_000_000;
        let a_virtual_reserve = 500_000;
        let b_reserve = 1_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;

        let mut runner = TestRunner::new();

        let payer = Keypair::new();
        runner.create_central_state_mock(
            &payer,
            5,
            5,
            20,
            10,
            36_000, // 10AM
        );
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        let pool = runner.create_pool_mock(
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
        
        // Set up user burn allowance with 5 burns already recorded (1 hour ago)
        let one_hour_ago = 1682899200 - 3600; // 1 hour before the test timestamp
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            payer.pubkey(),
            payer.pubkey(),
            5,
            one_hour_ago,
        );

        // Burn at 10:00:01 AM, should succeed because we've passed the reset time
        runner.set_system_clock( 1682935201);
        let burn_result = runner.burn_virtual_token(
            &payer,
            pool.pool,
            user_burn_allowance,
        );
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 999990);
        
        // Check that user burn allowance was reset
        let user_burn_allowance_data = runner.get_user_burn_allowance(&user_burn_allowance).unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 1);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682935201);
    }
}