use crate::errors::BcpmmError;
use crate::helpers::{calculate_burn_amount, calculate_new_virtual_reserve};
use crate::state::*;
use anchor_lang::prelude::*;

#[event]
pub struct BurnEvent {
    pub burn_amount: u64,

    pub topup_accrued: u64,

    pub new_b_reserve: u64,
    pub new_a_reserve: u64,
    pub new_outstanding_topup: u64,

    pub new_virtual_reserve: u64,
    pub new_buyback_fees_balance: u64,

    pub burner: Pubkey,
    pub pool: Pubkey,
}


#[derive(Accounts)]
#[instruction(pool_owner: bool)]
pub struct BurnVirtualToken<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut, seeds = [BCPMM_POOL_SEED, pool.pool_index.to_le_bytes().as_ref(), pool.creator.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, BcpmmPool>,

    #[account(mut, seeds = [USER_BURN_ALLOWANCE_SEED, signer.key().as_ref(), pool.platform_config.as_ref(), &[pool_owner as u8]], bump)]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    #[account(mut, address = pool.platform_config)]
    pub platform_config: Account<'info, PlatformConfig>,
}

pub fn burn_virtual_token(ctx: Context<BurnVirtualToken>, pool_owner: bool) -> Result<()> {
    // If burning as a pool owner, the signer must be the pool creator.
    // We are also checking if the creator is trying to burn as a user of their own pool.
    require!(
        pool_owner == (ctx.accounts.pool.creator == ctx.accounts.signer.key()),
        BcpmmError::InvalidPoolOwner
    );
    let burn_bp_x100 = if pool_owner {
        ctx.accounts.platform_config.creator_burn_bp_x100
    } else {
        ctx.accounts.platform_config.user_burn_bp_x100
    };
    let max_daily_burns = if pool_owner {
        ctx.accounts.platform_config.max_creator_daily_burn_count
    } else {
        ctx.accounts.platform_config.max_user_daily_burn_count
    };

    // Check if we should reset the daily burn count
    // We reset it if we have passed the burn reset window and previous burn was before the reset
    let now = Clock::get()?.unix_timestamp;
    if ctx.accounts.platform_config.is_after_burn_reset(now)?
        && !ctx
            .accounts
            .platform_config
            .is_after_burn_reset(ctx.accounts.user_burn_allowance.last_burn_timestamp)?
    {
        ctx.accounts.user_burn_allowance.burns_today = 0;
    } else if ctx.accounts.user_burn_allowance.burns_today >= max_daily_burns {
        return Err(BcpmmError::InsufficientBurnAllowance.into());
    }

    ctx.accounts.user_burn_allowance.burns_today += 1;
    ctx.accounts.user_burn_allowance.last_burn_timestamp = now;

    // Check if we should reset the pool's daily burn count
    if ctx.accounts.platform_config.is_after_burn_reset(now)?
        && !ctx
            .accounts
            .platform_config
            .is_after_burn_reset(ctx.accounts.pool.last_burn_timestamp)?
    {
        ctx.accounts.pool.burns_today = 1;

    // Not resetting so just increment the burn count for today.
    } else {
        ctx.accounts.pool.burns_today += 1;
    }
    ctx.accounts.pool.last_burn_timestamp = now;

    let burn_amount = calculate_burn_amount(burn_bp_x100, ctx.accounts.pool.b_reserve);
    let new_virtual_reserve = calculate_new_virtual_reserve(
        ctx.accounts.pool.a_virtual_reserve,
        ctx.accounts.pool.b_reserve,
        burn_amount,
    );

    let needed_topup_amount = ctx.accounts.pool.a_virtual_reserve - new_virtual_reserve;
    let real_topup_amount = needed_topup_amount.min(ctx.accounts.pool.buyback_fees_balance);
    ctx.accounts.pool.a_outstanding_topup += needed_topup_amount - real_topup_amount;
    ctx.accounts.pool.a_reserve += real_topup_amount;
    ctx.accounts.pool.buyback_fees_balance -= real_topup_amount;
    ctx.accounts.pool.a_virtual_reserve = new_virtual_reserve;
    ctx.accounts.pool.b_reserve -= burn_amount;
    emit!(BurnEvent {
        burn_amount: burn_amount,
        topup_accrued: needed_topup_amount - real_topup_amount,
        new_b_reserve: ctx.accounts.pool.b_reserve,
        new_a_reserve: ctx.accounts.pool.a_reserve,
        new_outstanding_topup: ctx.accounts.pool.a_outstanding_topup,
        new_virtual_reserve: new_virtual_reserve,
        new_buyback_fees_balance: ctx.accounts.pool.buyback_fees_balance,
        burner: ctx.accounts.signer.key(),
        pool: ctx.accounts.pool.key(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::state::BcpmmPool;
    use crate::test_utils::{TestPool, TestRunner};
    use anchor_lang::prelude::*;
    use solana_sdk::signature::{Keypair, Signer};

    fn setup_test() -> (TestRunner, Keypair, Keypair, TestPool) {
        // Parameters
        let a_reserve = 1_000_000;
        let a_virtual_reserve = 500_000;
        let b_reserve = 1_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let platform_fee_basis_points = 200;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;
        let a_outstanding_topup = 0;

        let mut runner = TestRunner::new();
        let payer = Keypair::new();

        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&payer, 9);
        runner.create_platform_config_mock(
            &payer,
            a_mint,
            5,
            5,
            5,
            10_000, // 1%
            20_000, // 2%
            36_000, // 10AM
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
        );
        let pool = runner.create_pool_mock(
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

        let user = Keypair::new();
        runner.airdrop(&user.pubkey(), 10_000_000_000);

        (runner, payer, user, pool)
    }

    #[test]
    fn test_burn_virtual_token_as_pool_owner() {
        let (mut runner, pool_owner, _, pool) = setup_test();

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool = BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        let owner_burn_allowance = runner
            .initialize_user_burn_allowance(&pool_owner, pool_owner.pubkey(), platform_config_sdk, true)
            .unwrap();

        // Can initialize also user burn allowance for other pools
        let user_burn_allowance =
            runner.initialize_user_burn_allowance(&pool_owner, pool_owner.pubkey(), platform_config_sdk, false);
        assert!(user_burn_allowance.is_ok());

        // Burn at a certain timestamp
        runner.set_system_clock(1682899200);
        let burn_result =
            runner.burn_virtual_token(&pool_owner, pool.pool, owner_burn_allowance, true);
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 980000);
        let owner_burn_allowance_data = runner
            .get_user_burn_allowance(&owner_burn_allowance)
            .unwrap();
        assert_eq!(owner_burn_allowance_data.burns_today, 1);
        assert_eq!(owner_burn_allowance_data.last_burn_timestamp, 1682899200);

        // User burn allowance not affected by creator burn
        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance.unwrap())
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 0);

        // 2 percent of virtual reserve is burned and required as topup
        assert_eq!(pool_data.a_virtual_reserve, 490000);
        assert_eq!(pool_data.a_outstanding_topup, 10000);
    }

    #[test]
    fn test_burn_virtual_token_as_user() {
        let (mut runner, _pool_owner, user, pool) = setup_test();

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool = BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Burn at a certain timestamp
        runner.set_system_clock(1682899200);
        let user_burn_allowance = runner
            .initialize_user_burn_allowance(&user, user.pubkey(), platform_config_sdk, false)
            .unwrap();

        let burn_result = runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, false);
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 990000);
        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance)
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 1);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682899200);

        // 1 percent of virtual reserve is burned and required as topup
        assert_eq!(pool_data.a_virtual_reserve, 495000);
        assert_eq!(pool_data.a_outstanding_topup, 5000);
    }

    #[test]
    fn test_burn_virtual_token_twice() {
        let (mut runner, _pool_owner, user, pool) = setup_test();

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool = BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Set up user burn allowance with 1 burn already recorded (1 hour ago)
        let one_hour_ago = 1682899200 - 3600; // 1 hour before the test timestamp
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            user.pubkey(),
            user.pubkey(),
            platform_config_sdk,
            1,
            one_hour_ago,
            false,
        );

        // Burn at current timestamp
        runner.set_system_clock(1682899200);
        let burn_result = runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, false);
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 990000);

        // Check that user burn allowance shows 2 burns for today
        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance)
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 2);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682899200);
    }

    #[test]
    fn test_burn_virtual_token_after_reset() {
        let (mut runner, _pool_owner, user, pool) = setup_test();

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool = BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Set up user burn allowance with 1 burn already recorded
        let one_hour_ago = 1682899200;
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            user.pubkey(),
            user.pubkey(),
            platform_config_sdk,
            1,
            one_hour_ago,
            false,
        );

        // Burn at 10:00:01 AM
        runner.set_system_clock(1682935201);
        let burn_result = runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, false);
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 990000);

        // Check that user burn allowance was reset
        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance)
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 1);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682935201);
    }

    #[test]
    fn test_burn_virtual_token_past_limit() {
        let (mut runner, _pool_owner, user, pool) = setup_test();

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool = BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Set up user burn allowance with 5 burns already recorded (1 hour ago)
        let one_hour_ago = 1682899200 - 3600; // 1 hour before the test timestamp
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            user.pubkey(),
            user.pubkey(),
            platform_config_sdk,
            5,
            one_hour_ago,
            false,
        );

        // Burn at current timestamp
        runner.set_system_clock(1682899200);
        let burn_result = runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, false);
        assert!(burn_result.is_err());
    }

    #[test]
    fn test_burn_virtual_token_past_limit_after_reset() {
        let (mut runner, _pool_owner, user, pool) = setup_test();

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool = BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Set up user burn allowance with 5 burns already recorded (1 hour ago)
        let one_hour_ago = 1682899200 - 3600; // 1 hour before the test timestamp
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            user.pubkey(),
            user.pubkey(),
            platform_config_sdk,
            5,
            one_hour_ago,
            false,
        );

        // Burn at 10:00:01 AM, should succeed because we've passed the reset time
        runner.set_system_clock(1682935201);
        let burn_result = runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, false);
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: BcpmmPool =
            BcpmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.b_reserve, 990000);

        // Check that user burn allowance was reset
        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance)
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 1);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682935201);
    }
}
