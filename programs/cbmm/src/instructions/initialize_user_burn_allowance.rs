use crate::state::*;
use anchor_lang::prelude::*;


#[derive(Accounts)]
#[instruction(pool_owner: bool)]
pub struct InitializeUserBurnAllowance<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The user for whom the burn allowance is being initialized
    /// CHECK: This is just a pubkey, not an account
    pub owner: UncheckedAccount<'info>,

    #[account(seeds = [CENTRAL_STATE_SEED], bump)]
    pub central_state: Account<'info, CentralState>,

    #[account(
        init,
        payer = payer,
        space = 8 + UserBurnAllowance::INIT_SPACE,
        seeds = [USER_BURN_ALLOWANCE_SEED, owner.key().as_ref(), &[pool_owner as u8]],
        bump
    )]
    pub user_burn_allowance: Account<'info, UserBurnAllowance>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_user_burn_allowance(
    ctx: Context<InitializeUserBurnAllowance>,
    _: bool,
) -> Result<()> {
    ctx.accounts.user_burn_allowance.set_inner(UserBurnAllowance::new(
        ctx.bumps.user_burn_allowance,
        ctx.accounts.owner.key(),
        ctx.accounts.payer.key(),
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestRunner;
    use solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_initialize_user_burn_allowance_basic() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        // Create prerequisite (not what we're testing)
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        
        // 2. ACT - Initialize user burn allowance as non-pool-owner
        let is_pool_owner = false;
        let uba_pda = runner.initialize_user_burn_allowance(
            &payer,
            owner.pubkey(),
            is_pool_owner,
        ).expect("Should successfully initialize user burn allowance");
        
        // 3. ASSERT
        let account = runner.svm.get_account(&uba_pda)
            .expect("UserBurnAllowance account should exist");
        
        // Verify ownership
        assert_eq!(account.owner, runner.program_id);
        
        // Verify size
        assert_eq!(
            account.data.len(),
            8 + crate::state::UserBurnAllowance::INIT_SPACE
        );
        
        // Deserialize and verify fields
        let uba_data = crate::state::UserBurnAllowance::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(
            uba_data.user,
            anchor_lang::prelude::Pubkey::from(owner.pubkey().to_bytes())
        );
        assert_eq!(
            uba_data.payer,
            anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes())
        );
        assert_eq!(uba_data.burns_today, 0, "Initial burns_today should be 0");
        assert_eq!(uba_data.last_burn_timestamp, 0, "Initial last_burn_timestamp should be 0");
        
        println!("✅ UserBurnAllowance initialized successfully!");
    }

    #[test]
    fn test_initialize_user_burn_allowance_as_pool_owner() {
        // Test initialization with is_pool_owner = true
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        
        // Initialize as pool owner
        let is_pool_owner = true;
        let uba_pda = runner.initialize_user_burn_allowance(
            &payer,
            owner.pubkey(),
            is_pool_owner,
        ).expect("Should initialize with is_pool_owner = true");
        
        let account = runner.svm.get_account(&uba_pda).unwrap();
        let uba_data = crate::state::UserBurnAllowance::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(
            uba_data.user,
            anchor_lang::prelude::Pubkey::from(owner.pubkey().to_bytes())
        );
        
        println!("✅ UserBurnAllowance created for pool owner!");
    }

    #[test]
    fn test_initialize_user_burn_allowance_different_pdas_for_pool_owner_flag() {
        // Verify that is_pool_owner flag creates different PDAs
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        
        // Create as non-pool-owner
        let uba_regular = runner.initialize_user_burn_allowance(
            &payer,
            owner.pubkey(),
            false,
        ).expect("Should create regular user allowance");
        
        // Create as pool-owner (same owner, different flag)
        let uba_pool_owner = runner.initialize_user_burn_allowance(
            &payer,
            owner.pubkey(),
            true,
        ).expect("Should create pool owner allowance");
        
        // Verify they are different PDAs
        assert_ne!(
            uba_regular,
            uba_pool_owner,
            "is_pool_owner flag should create different PDAs"
        );
        
        println!("✅ Different PDAs for pool_owner flag!");
    }

    #[test]
    fn test_initialize_user_burn_allowance_payer_as_owner() {
        // Test when payer and owner are the same
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        
        // Payer is also the owner
        let uba_pda = runner.initialize_user_burn_allowance(
            &payer,
            payer.pubkey(), // Same as payer
            false,
        ).expect("Should allow payer to be owner");
        
        let account = runner.svm.get_account(&uba_pda).unwrap();
        let uba_data = crate::state::UserBurnAllowance::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(
            uba_data.user,
            anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes())
        );
        assert_eq!(
            uba_data.payer,
            anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes())
        );
        
        println!("✅ Payer can be the owner!");
    }

    #[test]
    fn test_initialize_user_burn_allowance_fails_when_already_initialized() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        
        // Initialize once
        runner.initialize_user_burn_allowance(
            &payer,
            owner.pubkey(),
            false,
        ).expect("First initialization should succeed");
        
        // 2. ACT - Try to initialize again
        let result = runner.initialize_user_burn_allowance(
            &payer,
            owner.pubkey(),
            false,
        );
        
        // 3. ASSERT - Should fail
        assert!(result.is_err(), "Should not allow re-initialization");
        
        println!("✅ Correctly prevented double initialization!");
    }

    #[test]
    fn test_initialize_user_burn_allowance_pda_derivation() {
        // Verify PDA is derived correctly
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        
        let is_pool_owner = false;
        
        // Manually derive expected PDA
        let (expected_uba_pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
            &[
                crate::state::USER_BURN_ALLOWANCE_SEED,
                owner.pubkey().as_ref(),
                &[is_pool_owner as u8],
            ],
            &runner.program_id,
        );
        
        // Initialize
        let actual_uba_pda = runner.initialize_user_burn_allowance(
            &payer,
            owner.pubkey(),
            is_pool_owner,
        ).expect("Should initialize");
        
        // Verify PDA matches
        assert_eq!(
            actual_uba_pda,
            expected_uba_pda,
            "PDA should be derived from owner + is_pool_owner flag"
        );
        
        println!("✅ PDA derived correctly!");
    }

    #[test]
    fn test_initialize_user_burn_allowance_fails_without_central_state() {
        // Test that initialization fails without CentralState
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        // DO NOT create CentralState
        
        // Try to initialize
        let result = runner.initialize_user_burn_allowance(
            &payer,
            owner.pubkey(),
            false,
        );
        
        assert!(result.is_err(), "Should fail when CentralState doesn't exist");
        
        println!("✅ Correctly requires CentralState to exist!");
    }

    #[test]
    fn test_initialize_user_burn_allowance_multiple_users() {
        // Test that multiple users can have burn allowances
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner1 = Keypair::new();
        let owner2 = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        
        // Create for first user
        let uba1 = runner.initialize_user_burn_allowance(
            &payer,
            owner1.pubkey(),
            false,
        ).expect("Should create for user 1");
        
        // Create for second user
        let uba2 = runner.initialize_user_burn_allowance(
            &payer,
            owner2.pubkey(),
            false,
        ).expect("Should create for user 2");
        
        // Verify they are different accounts
        assert_ne!(uba1, uba2, "Different users should have different PDAs");
        
        // Verify both have correct data
        let uba1_data = crate::state::UserBurnAllowance::try_deserialize(
            &mut runner.svm.get_account(&uba1).unwrap().data.as_slice()
        ).unwrap();
        
        let uba2_data = crate::state::UserBurnAllowance::try_deserialize(
            &mut runner.svm.get_account(&uba2).unwrap().data.as_slice()
        ).unwrap();
        
        assert_eq!(
            uba1_data.user,
            anchor_lang::prelude::Pubkey::from(owner1.pubkey().to_bytes())
        );
        assert_eq!(
            uba2_data.user,
            anchor_lang::prelude::Pubkey::from(owner2.pubkey().to_bytes())
        );
        
        println!("✅ Multiple users can have burn allowances!");
    }
}
