use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use crate::state::*;
use crate::errors::BcpmmError;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ClaimAdminFeesArgs {
    pub amount: u64,
}

#[derive(Accounts)]
pub struct ClaimAdminFees<'info> {
    #[account(mut, address = treasury.authority @ BcpmmError::InvalidTreasuryAuthority)]
    pub admin: Signer<'info>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = admin,
        associated_token::token_program = token_program        
    )]
    pub admin_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [TREASURY_SEED, a_mint.key().as_ref()], bump = treasury.bump)]
    pub treasury: Account<'info, Treasury>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = treasury,
        associated_token::token_program = token_program        
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    pub a_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn claim_admin_fees(ctx: Context<ClaimAdminFees>, args: ClaimAdminFeesArgs) -> Result<()> {
    let treasury = &mut ctx.accounts.treasury;
    
    require!(args.amount <= treasury.fees_available, BcpmmError::InsufficientVirtualTokenBalance);
    require!(args.amount > 0, BcpmmError::AmountTooSmall);

    // Subtract the claimed amount and transfer to admin
    treasury.fees_available -= args.amount;
    
    // Create a dummy pool to use the transfer method
    let mut dummy_pool = BcpmmPool::default();
    dummy_pool.treasury_transfer_out(
        args.amount,
        &ctx.accounts.treasury,
        &ctx.accounts.a_mint,
        &ctx.accounts.treasury_ata,
        &ctx.accounts.admin_ata,
        &ctx.accounts.token_program,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::state::Treasury;
    use crate::test_utils::{TestRunner, TestPool};
    use anchor_lang::prelude::*;
    use solana_program::program_pack::Pack;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::pubkey::Pubkey;
    use test_case::test_case;

    fn setup_test() -> (TestRunner, Keypair, TestPool, Pubkey, Pubkey) {
        // Parameters
        let a_reserve = 0;
        let a_virtual_reserve = 1_000_000;
        let b_reserve = 2_000_000;
        let b_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let creator_fees_balance = 1000;
        let buyback_fees_balance = 0;
        let treasury_fees_available = 500; // Start with some admin fees available

        let mut runner = TestRunner::new();
        let admin = Keypair::new();
        
        runner.airdrop(&admin.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&admin, 9);
        let admin_ata = runner.create_associated_token_account(&admin, a_mint, &admin.pubkey());
        runner.create_treasury_mock(admin.pubkey(), a_mint);
        runner.create_treasury_ata(&admin, a_mint, a_reserve + creator_fees_balance + buyback_fees_balance + treasury_fees_available);

        runner.create_central_state_mock(&admin, 5, 5, 2, 1, 10000);

        let test_pool = runner.create_pool_mock(
            &admin,
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

        // Set treasury fees_available
        let treasury_pda = runner.get_treasury_pda(a_mint);
        let treasury_account = runner.svm.get_account(&treasury_pda).unwrap();
        let mut treasury_data = Treasury::try_deserialize(&mut treasury_account.data.as_slice()).unwrap();
        treasury_data.fees_available = treasury_fees_available;
        let mut data = Vec::new();
        treasury_data.try_serialize(&mut data).unwrap();
        runner.svm.set_account(treasury_pda, solana_sdk::account::Account {
            lamports: treasury_account.lamports,
            data,
            owner: treasury_account.owner,
            executable: treasury_account.executable,
            rent_epoch: treasury_account.rent_epoch,
        }).unwrap();

        (runner, admin, test_pool, admin_ata, a_mint)
    }

    #[test_case(250, true)]
    #[test_case(500, true)]
    #[test_case(501, false)]
    #[test_case(0, false)]
    fn test_claim_admin_fees(claim_amount: u64, success: bool) {
        let (mut runner, admin, _, admin_ata, a_mint) = setup_test();
        let initial_treasury_fees = 500;

        // Claim admin fees
        let result = runner.claim_admin_fees(
            &admin,
            admin_ata,
            a_mint,
            claim_amount,
        );
        assert_eq!(result.is_ok(), success);
        if success {
            // Check that treasury fees_available was subtracted
            let treasury_pda = runner.get_treasury_pda(a_mint);
            let treasury_account = runner.svm.get_account(&treasury_pda).unwrap();
            let final_treasury_data: Treasury =
                Treasury::try_deserialize(&mut treasury_account.data.as_slice()).unwrap();
            assert_eq!(final_treasury_data.fees_available, initial_treasury_fees - claim_amount);

            // Check that admin ATA balance increased by the claimed amount
            let admin_ata_account = runner.svm.get_account(&admin_ata).unwrap();
            let final_admin_balance = anchor_spl::token::spl_token::state::Account::unpack(&admin_ata_account.data).unwrap().amount;
            assert_eq!(final_admin_balance, claim_amount);
        }
    }

    #[test]
    fn test_claim_admin_fees_wrong_authority() {
        let (mut runner, _, _, _, _) = setup_test();
        let claim_amount = 250;

        let other_user = Keypair::new();
        runner.airdrop(&other_user.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&other_user, 9);
        let other_user_ata = runner.create_associated_token_account(&other_user, a_mint, &other_user.pubkey());

        // Claim admin fees
        let result = runner.claim_admin_fees(
            &other_user,
            other_user_ata,
            a_mint,
            claim_amount,
        );
        assert!(result.is_err());
    }
}
