use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use crate::state::*;
use crate::errors::CbmmError;

#[derive(Accounts)]
pub struct ClaimCreatorFees<'info> {
    #[account(mut, address = pool.creator @ CbmmError::InvalidPoolOwner)]
    pub owner: Signer<'info>,

    #[account(mut,
        associated_token::mint = quote_mint,
        associated_token::authority = owner,
        associated_token::token_program = token_program        
    )]
    pub owner_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            CBMM_POOL_SEED,
            pool.pool_index.to_le_bytes().as_ref(),
            pool.creator.as_ref(),
            pool.platform_config.as_ref(),
        ],
        bump = pool.bump,
    )]
    pub pool: Account<'info, CbmmPool>,

    #[account(mut,
        associated_token::mint = quote_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program        
    )]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,

    pub quote_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn claim_creator_fees(ctx: Context<ClaimCreatorFees>) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let amount = pool.creator_fees_balance;
    // Subtract the claimed amount and transfer to owner
    pool.creator_fees_balance -= amount;
    let pool_account_info = pool.to_account_info();
    pool.transfer_out(
        amount,
        &pool_account_info,
        &ctx.accounts.quote_mint,
        &ctx.accounts.pool_ata,
        &ctx.accounts.owner_ata,
        &ctx.accounts.token_program,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::state::CbmmPool;
    use crate::test_utils::TestRunner;
    use anchor_lang::prelude::*;
    use anchor_lang::solana_program::program_pack::Pack;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::pubkey::Pubkey;
    use test_case::test_case;

    fn setup_test() -> (TestRunner, Keypair, Pubkey, Pubkey, Pubkey) {
        // Parameters
        let quote_reserve = 0;
        let quote_virtual_reserve = 1_000_000;
        let base_reserve = 2_000_000;
        let base_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let platform_fee_basis_points = 200;
        let creator_fees_balance = 1000; // Start with some creator fees available
        let buyback_fees_balance = 0;
        let quote_outstanding_topup = 0;

        let mut runner = TestRunner::new();
        let owner = Keypair::new();
        
        runner.airdrop(&owner.pubkey(), 10_000_000_000);
        let quote_mint = runner.create_mint(&owner, 9);
        let owner_ata = runner.create_associated_token_account(&owner, quote_mint, &owner.pubkey());

        runner.create_platform_config_mock(&owner,
            quote_mint,
            5,
            5,
            2,
            1,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
        );

        let pool_created = runner.create_pool_mock(
            &owner,
            quote_mint,
            quote_reserve,
            quote_virtual_reserve,
            base_reserve,
            base_mint_decimals,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
            creator_fees_balance,
            buyback_fees_balance,
            quote_outstanding_topup,
        );

        // pool ata
        runner.create_associated_token_account(&owner, quote_mint, &pool_created.pool);
        runner.mint_tokens(&owner, pool_created.pool, quote_mint, creator_fees_balance);

        (runner, owner, pool_created.pool, owner_ata, quote_mint)
    }

    #[test_case(500, true)]
    #[test_case(1000, true)]
    #[test_case(1001, false)]
    #[test_case(0, false)]
    fn test_claim_creator_fees(claim_amount: u64, success: bool) {
        let (mut runner, owner, pool, owner_ata, quote_mint) = setup_test();
        let initial_creator_fees = 1000;

        // Claim creator fees
        let result = runner.claim_creator_fees(
            &owner,
            owner_ata,
            quote_mint,
            pool,
            claim_amount,
        );
        assert!(result.is_ok() == success);

        if success {

          // Check that creator fees were subtracted from pool balance
          let pool_account = runner.svm.get_account(&pool).unwrap();
          let final_pool_data: CbmmPool =
              CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
          assert_eq!(final_pool_data.creator_fees_balance, initial_creator_fees - claim_amount);

          // Check that owner ATA balance increased by the claimed amount
          let owner_ata_account = runner.svm.get_account(&owner_ata).unwrap();
          let final_owner_balance = anchor_spl::token::spl_token::state::Account::unpack(&owner_ata_account.data).unwrap().amount;
          assert_eq!(final_owner_balance, claim_amount);
        }
    }

    #[test]
    fn test_claim_creator_fees_wrong_owner() {
        let (mut runner, _, pool, _, _) = setup_test();
        let claim_amount = 500;

        let other_user = Keypair::new();
        runner.airdrop(&other_user.pubkey(), 10_000_000_000);
        let quote_mint = runner.create_mint(&other_user, 9);
        let other_user_ata = runner.create_associated_token_account(&other_user, quote_mint, &other_user.pubkey());

        // Claim creator fees
        let result = runner.claim_creator_fees(
            &other_user,
            other_user_ata,
            quote_mint,
            pool,
            claim_amount,
        );
        assert!(result.is_err());
    }
}