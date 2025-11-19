use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializeVirtualTokenAccount<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: No check needed, owner can be any account
    pub owner: AccountInfo<'info>,
    #[account(init, payer = payer, space = VirtualTokenAccount::INIT_SPACE + 8, seeds = [VIRTUAL_TOKEN_ACCOUNT_SEED, pool.key().as_ref(), payer.key().as_ref()], bump)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
    pub pool: Account<'info, BcpmmPool>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_virtual_token_account(ctx: Context<InitializeVirtualTokenAccount>) -> Result<()> {
    ctx.accounts
        .virtual_token_account
        .set_inner(VirtualTokenAccount::try_new(
            ctx.bumps.virtual_token_account,
            ctx.accounts.pool.key(),
            ctx.accounts.owner.key(),
        ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestRunner;
    use solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_initialize_virtual_token_account_basic() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        // Create prerequisites (not what we're testing)
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        let mint = runner.create_mint_mock(); // ✅ Mock mint for pure unit test
        let pool_pda = runner.create_pool_mock(
            &payer,
            mint,
            0, 1_000_000_000, 1_000_000_000_000_000,
            6, 100, 200, 300, 0, 0, 0,
        ).pool;
        
        // 2. ACT - Initialize virtual token account
        let vta_pda = runner.initialize_virtual_token_account(
            &payer,
            owner.pubkey(),
            pool_pda,
        ).expect("Should successfully initialize virtual token account");
        
        // 3. ASSERT
        let account = runner.svm.get_account(&vta_pda)
            .expect("VirtualTokenAccount should exist");
        
        // Verify ownership
        assert_eq!(account.owner, runner.program_id);
        
        // Verify size
        assert_eq!(
            account.data.len(),
            crate::state::VirtualTokenAccount::INIT_SPACE + 8
        );
        
        // Deserialize and verify fields
        let vta_data = crate::state::VirtualTokenAccount::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(
            vta_data.pool,
            anchor_lang::prelude::Pubkey::from(pool_pda.to_bytes())
        );
        assert_eq!(
            vta_data.owner,
            anchor_lang::prelude::Pubkey::from(owner.pubkey().to_bytes())
        );
        assert_eq!(vta_data.balance, 0, "Initial balance should be 0");
        assert_eq!(vta_data.fees_paid, 0, "Initial fees_paid should be 0");
        
        println!("✅ VirtualTokenAccount initialized successfully!");
    }

    #[test]
    fn test_initialize_virtual_token_account_payer_as_owner() {
        // Test when payer and owner are the same
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        let mint = runner.create_mint_mock();
        let pool_pda = runner.create_pool_mock(
            &payer,
            mint,
            0, 1_000_000_000, 1_000_000_000_000_000,
            6, 100, 200, 300, 0, 0, 0,
        ).pool;
        
        // Payer is also the owner
        let vta_pda = runner.initialize_virtual_token_account(
            &payer,
            payer.pubkey(), // Same as payer
            pool_pda,
        ).expect("Should allow payer to be owner");
        
        let account = runner.svm.get_account(&vta_pda).unwrap();
        let vta_data = crate::state::VirtualTokenAccount::try_deserialize(
            &mut account.data.as_slice()
        ).unwrap();
        
        assert_eq!(
            vta_data.owner,
            anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes())
        );
        
        println!("✅ Payer can be the owner!");
    }

    #[test]
    fn test_initialize_virtual_token_account_fails_when_already_initialized() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        let mint = runner.create_mint_mock();
        let pool_pda = runner.create_pool_mock(
            &payer,
            mint,
            0, 1_000_000_000, 1_000_000_000_000_000,
            6, 100, 200, 300, 0, 0, 0,
        ).pool;
        
        // Initialize once
        runner.initialize_virtual_token_account(
            &payer,
            owner.pubkey(),
            pool_pda,
        ).expect("First initialization should succeed");
        
        // 2. ACT - Try to initialize again
        let result = runner.initialize_virtual_token_account(
            &payer,
            owner.pubkey(),
            pool_pda,
        );
        
        // 3. ASSERT - Should fail
        assert!(result.is_err(), "Should not allow re-initialization");
        
        println!("✅ Correctly prevented double initialization!");
    }

    #[test]
    fn test_initialize_virtual_token_account_multiple_owners_same_pool() {
        // Test that multiple users can have VTAs for the same pool
        let mut runner = TestRunner::new();
        let payer1 = Keypair::new();
        let payer2 = Keypair::new();
        let owner1 = Keypair::new();
        let owner2 = Keypair::new();
        
        runner.airdrop(&payer1.pubkey(), 10_000_000_000);
        runner.airdrop(&payer2.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer1, 10, 5, 1000, 500, 43200, 100, 200, 300);
        let mint = runner.create_mint_mock();
        let pool_pda = runner.create_pool_mock(
            &payer1,
            mint,
            0, 1_000_000_000, 1_000_000_000_000_000,
            6, 100, 200, 300, 0, 0, 0,
        ).pool;
        
        // Create VTA for first user
        let vta1 = runner.initialize_virtual_token_account(
            &payer1,
            owner1.pubkey(),
            pool_pda,
        ).expect("Should create VTA for user 1");
        
        // Create VTA for second user (different payer = different PDA)
        let vta2 = runner.initialize_virtual_token_account(
            &payer2,
            owner2.pubkey(),
            pool_pda,
        ).expect("Should create VTA for user 2");
        
        // Verify they are different accounts
        assert_ne!(vta1, vta2, "VTAs should have different addresses");
        
        // Verify both exist with correct data
        let vta1_data = crate::state::VirtualTokenAccount::try_deserialize(
            &mut runner.svm.get_account(&vta1).unwrap().data.as_slice()
        ).unwrap();
        
        let vta2_data = crate::state::VirtualTokenAccount::try_deserialize(
            &mut runner.svm.get_account(&vta2).unwrap().data.as_slice()
        ).unwrap();
        
        assert_eq!(
            vta1_data.owner,
            anchor_lang::prelude::Pubkey::from(owner1.pubkey().to_bytes())
        );
        assert_eq!(
            vta2_data.owner,
            anchor_lang::prelude::Pubkey::from(owner2.pubkey().to_bytes())
        );
        
        println!("✅ Multiple users can have VTAs for the same pool!");
    }

    #[test]
    fn test_initialize_virtual_token_account_pda_derivation() {
        // Verify PDA is derived correctly from pool + payer (not owner!)
        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let owner = Keypair::new();
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        
        runner.create_central_state_mock(&payer, 10, 5, 1000, 500, 43200, 100, 200, 300);
        let mint = runner.create_mint_mock();
        let pool_pda = runner.create_pool_mock(
            &payer,
            mint,
            0, 1_000_000_000, 1_000_000_000_000_000,
            6, 100, 200, 300, 0, 0, 0,
        ).pool;
        
        // Manually derive expected PDA
        let (expected_vta_pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
            &[
                crate::state::VIRTUAL_TOKEN_ACCOUNT_SEED,
                pool_pda.as_ref(),
                payer.pubkey().as_ref(), // Note: payer, not owner!
            ],
            &runner.program_id,
        );
        
        // Initialize VTA
        let actual_vta_pda = runner.initialize_virtual_token_account(
            &payer,
            owner.pubkey(),
            pool_pda,
        ).expect("Should initialize");
        
        // Verify PDA matches
        assert_eq!(
            actual_vta_pda,
            expected_vta_pda,
            "PDA should be derived from pool + payer"
        );
        
        println!("✅ PDA derived correctly from pool + payer!");
    }
}
