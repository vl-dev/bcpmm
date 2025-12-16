use crate::errors::CbmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[event]
pub struct BurnEvent {
    pub burn_amount: u64,

    pub topup_accrued: u64,

    pub new_b_reserve: u64,
    pub new_a_reserve: u64,

    pub new_virtual_reserve: u64,
    pub new_buyback_fees_balance: u64,

    pub burner: Pubkey,
    pub pool: Pubkey,
}

#[derive(Accounts)]
pub struct BurnVirtualToken<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [
            CBMM_POOL_SEED,
            pool.pool_index.to_le_bytes().as_ref(),
            pool.creator.as_ref(),
            platform_config.key().as_ref(),
        ],
        bump = pool.bump,        
    )]
    pub pool: Account<'info, CbmmPool>,

    // We don't have to check all cases of the user_burn_allowance here,
    // because we restrict the invalid combinations from creation
    #[account(mut, seeds = [
        USER_BURN_ALLOWANCE_SEED,
        signer.key().as_ref(),
        platform_config.key().as_ref(),
        &[user_burn_allowance.burn_tier_index],
        platform_config.burn_tiers_updated_at.to_le_bytes().as_ref(),
    ], bump = user_burn_allowance.bump)]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    pub platform_config: Account<'info, PlatformConfig>,

    /// Optional burn authority. Required and must match `platform_config.burn_authority`
    /// if that field is set; otherwise this account is ignored.
    pub burn_authority: Option<Signer<'info>>,
}

pub fn burn_virtual_token(ctx: Context<BurnVirtualToken>) -> Result<()> {
    let user_burn_allowance = &mut ctx.accounts.user_burn_allowance;
    let user_daily_burn_index = user_burn_allowance.pop()?;
    let platform_config = &ctx.accounts.platform_config;
    let burn_tier_index = user_burn_allowance.burn_tier_index;
    require_gt!(
        platform_config.burn_tiers.len() as u8,
        burn_tier_index,
        CbmmError::InvalidBurnTierIndex
    );
    let burn_tier = &platform_config.burn_tiers[burn_tier_index as usize];

    // If a global burn authority is configured, require it to sign every burn.
    platform_config.check_burn_authority(
        ctx.accounts
            .burn_authority
            .as_ref()
            .map(|authority| authority.key()),
    )?;

    if let BurnRole::PoolOwner = burn_tier.role {
        require_keys_eq!(ctx.accounts.pool.creator, ctx.accounts.signer.key(), CbmmError::InvalidPoolCreator);
    }

    require_gte!(
        burn_tier.max_daily_burns,
        user_daily_burn_index,
        CbmmError::BurnLimitReached
    );

    let requested_amount = burn_tier.burn_bp_x100;

    let config = &platform_config.burn_rate_config;
    let burn_result = ctx.accounts.pool.burn(config, requested_amount)?;
    let topup_accrued = ctx.accounts.pool.topup()?;

    emit!(BurnEvent {
        burn_amount: burn_result.burn_amount,
        topup_accrued: topup_accrued,
        new_b_reserve: ctx.accounts.pool.base_reserve,
        new_a_reserve: ctx.accounts.pool.quote_reserve,
        new_virtual_reserve: ctx.accounts.pool.quote_virtual_reserve,
        new_buyback_fees_balance: ctx.accounts.pool.buyback_fees_balance,
        burner: ctx.accounts.signer.key(),
        pool: ctx.accounts.pool.key(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::state::CbmmPool;
    use crate::test_utils::{TestPool, TestRunner};
    use anchor_lang::prelude::*;
    use solana_sdk::signature::{Keypair, Signer};

    fn setup_test(burn_authority: Option<Pubkey>) -> (TestRunner, Keypair, Keypair, TestPool) {
        // Parameters
        let quote_reserve = 0;
        let quote_virtual_reserve = 500_000;
        let base_reserve = 1_000_000;
        let base_mint_decimals = 6;
        let creator_fee_bp = 200;
        let buyback_fee_bp = 600;
        let platform_fee_bp = 200;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;
        let quote_outstanding_topup = 0;

        let mut runner = TestRunner::new();
        let payer = Keypair::new();

        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        let quote_mint = runner.create_mint(&payer, 9);
        let platform_config = runner.create_platform_config_mock(
            &payer,
            quote_mint,
            5,
            5,
            1_000, // 0.1% (max allowed for Anyone role)
            20_000, // 2%
            creator_fee_bp,
            buyback_fee_bp,
            platform_fee_bp,
            burn_authority,
        );
        let pool = runner.create_pool_mock(
            &payer,
            platform_config,
            quote_mint,
            quote_reserve,
            quote_virtual_reserve,
            base_reserve,
            base_reserve,
            base_mint_decimals,
            creator_fee_bp,
            buyback_fee_bp,
            platform_fee_bp,
            creator_fees_balance,
            buyback_fees_balance,
            quote_outstanding_topup,
        );

        let user = Keypair::new();
        runner.airdrop(&user.pubkey(), 10_000_000_000);

        (runner, payer, user, pool)
    }

    #[test]
    fn test_burn_virtual_token_as_pool_owner() {
        let (mut runner, pool_owner, _, pool) = setup_test(None);

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        let owner_burn_allowance = runner
            .initialize_user_burn_allowance(
                &pool_owner,
                pool_owner.pubkey(),
                platform_config_sdk,
                true,
            )
            .unwrap();

        // Can initialize also user burn allowance for other pools
        let user_burn_allowance = runner.initialize_user_burn_allowance(
            &pool_owner,
            pool_owner.pubkey(),
            platform_config_sdk,
            false,
        );
        assert!(user_burn_allowance.is_ok());

        // Burn at a certain timestamp
        runner.set_system_clock(1682899200);
        let burn_result =
            runner.burn_virtual_token(&pool_owner, pool.pool, owner_burn_allowance, None);
        if let Err(ref e) = burn_result {
            eprintln!("Burn error: {:?}", e);
        }
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.base_reserve, 980000);
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
        assert_eq!(pool_data.quote_virtual_reserve, 490000);
    }

    #[test]
    fn test_burn_virtual_token_as_user() {
        let (mut runner, _pool_owner, user, pool) = setup_test(None);

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Burn at a certain timestamp
        runner.set_system_clock(1682899200);
        let user_burn_allowance = runner
            .initialize_user_burn_allowance(&user, user.pubkey(), platform_config_sdk, false)
            .unwrap();

        let burn_result =
            runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, None);
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.base_reserve, 999000);
        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance)
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 1);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682899200);

        // 1 percent of virtual reserve is burned and required as topup
        assert_eq!(pool_data.quote_virtual_reserve, 499500);
    }

    #[test]
    fn test_burn_virtual_token_twice() {
        let (mut runner, _pool_owner, user, pool) = setup_test(None);

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
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
            one_hour_ago,
        );

        // Burn at current timestamp
        runner.set_system_clock(1682899200);
        let burn_result =
            runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, None);
        assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.base_reserve, 999000);

        // Check that user burn allowance shows 2 burns for today
        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance)
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 2);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682899200);
    }

    #[test]
    fn test_burn_virtual_token_after_reset() {
        let (mut runner, _pool_owner, user, pool) = setup_test(None);

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
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
            one_hour_ago,
        );

        // Burn at 10:00:01 AM
        runner.set_system_clock(1682935201);
        let burn_result =
            runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, None);
        burn_result.unwrap();
        // assert!(burn_result.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.base_reserve, 999000);

        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance)
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 2);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682935201);
    }

    #[test]
    fn test_burn_virtual_token_past_limit_no_reset() {
        let (mut runner, _pool_owner, user, pool) = setup_test(None);

        // Get platform_config from pool
        let pool_account: solana_sdk::account::Account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
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
            one_hour_ago,
        );

        // Burn at current timestamp
        runner.set_system_clock(1682899200);
        let burn_result =
            runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, None);
        assert!(burn_result.is_err());
    }

    #[test]
    fn test_burn_virtual_token_past_limit_after_reset() {
        let (mut runner, _pool_owner, user, pool) = setup_test(None );

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Set up user burn allowance with 5 burns already recorded (1 hour ago)
        let one_hour_ago = 1682899200 - 3600; // 1 hour before the test timestamp
        let two_days_ago = 1682899200 - 3600 * 24 * 2;
        let user_burn_allowance = runner.create_user_burn_allowance_mock(
            user.pubkey(),
            user.pubkey(),
            platform_config_sdk,
            5,
            one_hour_ago,
            false,
            two_days_ago,
        );

        // Burn at 10:00:01 AM, should succeed because we've passed the reset time
        runner.set_system_clock(1682935201);
        let burn_result =
            runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, None);
        burn_result.unwrap();

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.base_reserve, 999000);

        // Check that user burn allowance was reset
        let user_burn_allowance_data = runner
            .get_user_burn_allowance(&user_burn_allowance)
            .unwrap();
        assert_eq!(user_burn_allowance_data.burns_today, 1);
        assert_eq!(user_burn_allowance_data.last_burn_timestamp, 1682935201);
    }

    #[test]
    fn test_burn_virtual_token_with_burn_authority_succeeds() {
        let burn_authority = Keypair::new();
        let (mut runner, _payer, user, pool) = setup_test(Some(
            anchor_lang::prelude::Pubkey::new_from_array(burn_authority.pubkey().to_bytes()),
        ));

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Initialize user burn allowance
        runner.set_system_clock(1682899200);
        let user_burn_allowance = runner
            .initialize_user_burn_allowance(&user, user.pubkey(), platform_config_sdk, false)
            .unwrap();

        // Burn must succeed when burn authority is provided
        let burn_result = runner.burn_virtual_token(
            &user,
            pool.pool,
            user_burn_allowance,
            Some(&burn_authority),
        );
        assert!(burn_result.is_ok());
    }

    #[test]
    fn test_burn_virtual_token_missing_burn_authority_fails() {
        let burn_authority = Keypair::new();
        let (mut runner, _payer, user, pool) = setup_test(Some(
            anchor_lang::prelude::Pubkey::new_from_array(burn_authority.pubkey().to_bytes()),
        ));

        // Get platform_config from pool
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = pool_data.platform_config;
        let platform_config_sdk = solana_sdk::pubkey::Pubkey::from(platform_config.to_bytes());

        // Initialize user burn allowance
        runner.set_system_clock(1682899200);
        let user_burn_allowance = runner
            .initialize_user_burn_allowance(&user, user.pubkey(), platform_config_sdk, false)
            .unwrap();

        // Burn without passing burn authority should fail
        let burn_result =
            runner.burn_virtual_token(&user, pool.pool, user_burn_allowance, None);
        assert!(burn_result.is_err());
    }
}
