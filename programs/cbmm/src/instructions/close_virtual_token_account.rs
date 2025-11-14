use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseVirtualTokenAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        close = owner,
        has_one = owner @ BcpmmError::InvalidOwner,
        constraint = virtual_token_account.balance == 0 @ BcpmmError::NonzeroBalance
    )]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
}

pub fn close_virtual_token_account(ctx: Context<CloseVirtualTokenAccount>) -> Result<()> {
    msg!(
        "Closing virtual token account, collected fees: {}",
        ctx.accounts.virtual_token_account.fees_paid
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_utils::TestRunner;
    use solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_close_virtual_token_account_basic() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let owner = Keypair::new();
        runner.airdrop(&owner.pubkey(), 10_000_000_000);
        
        // Create VTA with zero balance (ready to close)
        let pool = Keypair::new().pubkey();
        let vta_pda = runner.create_virtual_token_account_mock(
            owner.pubkey(),
            pool,
            0,  // balance = 0 ✅
            0,  // fees_paid = 0
        );
        
        // Get owner's initial balance
        let owner_balance_before = runner.svm.get_balance(&owner.pubkey()).unwrap_or(0);
        
        // 2. ACT - Close the account
        runner.close_virtual_token_account(&owner, vta_pda)
            .expect("Should successfully close VTA with zero balance");
        
        // 3. ASSERT
        // Verify account is closed (doesn't exist)
        let account = runner.svm.get_account(&vta_pda);
        assert!(account.is_none(), "Account should be closed");
        
        // Verify rent was refunded to owner (owner's balance increased)
        let owner_balance_after = runner.svm.get_balance(&owner.pubkey()).unwrap_or(0);
        assert!(
            owner_balance_after > owner_balance_before,
            "Owner should receive rent refund"
        );
        
        println!("✅ VirtualTokenAccount closed successfully!");
    }

    #[test]
    fn test_close_virtual_token_account_fails_with_nonzero_balance() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let owner = Keypair::new();
        runner.airdrop(&owner.pubkey(), 10_000_000_000);
        
        let pool = Keypair::new().pubkey();
        let vta_pda = runner.create_virtual_token_account_mock(
            owner.pubkey(),
            pool,
            1_000,  // balance > 0 ❌
            0,
        );
        
        // 2. ACT - Try to close with non-zero balance
        let result = runner.close_virtual_token_account(&owner, vta_pda);
        
        // 3. ASSERT - Should fail
        assert!(result.is_err(), "Should fail with non-zero balance");
        
        let error_msg = result.unwrap_err().message;
        assert!(
            error_msg.contains("NonzeroBalance") || error_msg.contains("6002"),
            "Expected NonzeroBalance error, got: {}",
            error_msg
        );
        
        // Verify account still exists
        let account = runner.svm.get_account(&vta_pda);
        assert!(account.is_some(), "Account should still exist after failed close");
        
        println!("✅ Correctly prevents closing with non-zero balance!");
    }

    #[test]
    fn test_close_virtual_token_account_fails_with_wrong_owner() {
        // 1. ARRANGE
        let mut runner = TestRunner::new();
        let real_owner = Keypair::new();
        let fake_owner = Keypair::new();
        runner.airdrop(&real_owner.pubkey(), 10_000_000_000);
        runner.airdrop(&fake_owner.pubkey(), 10_000_000_000);
        
        let pool = Keypair::new().pubkey();
        let vta_pda = runner.create_virtual_token_account_mock(
            real_owner.pubkey(),  // Real owner
            pool,
            0,
            0,
        );
        
        // 2. ACT - Try to close with wrong owner
        let result = runner.close_virtual_token_account(&fake_owner, vta_pda);
        
        // 3. ASSERT - Should fail
        assert!(result.is_err(), "Should fail with wrong owner");
        
        let error_msg = result.unwrap_err().message;
        assert!(
            error_msg.contains("InvalidOwner") || error_msg.contains("6001") || error_msg.contains("has_one"),
            "Expected InvalidOwner error, got: {}",
            error_msg
        );
        
        // Verify account still exists
        let account = runner.svm.get_account(&vta_pda);
        assert!(account.is_some(), "Account should still exist after failed close");
        
        println!("✅ Correctly prevents unauthorized closing!");
    }

}
