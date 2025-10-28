use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use crate::state::*;

#[derive(Accounts)]
pub struct ClaimAdminFees<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = central_state.admin,
        associated_token::token_program = token_program        
    )]
    pub admin_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [CENTRAL_STATE_SEED], bump = central_state.bump)]
    pub central_state: Account<'info, CentralState>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = central_state,
        associated_token::token_program = token_program        
    )]
    pub central_state_ata: InterfaceAccount<'info, TokenAccount>,

    pub a_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_admin_fees(ctx: Context<ClaimAdminFees>) -> Result<()> {
    
    // Transfer from central state's ATA to admin's ATA.
    let token_balance = ctx.accounts.central_state_ata.amount;
    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.central_state_ata.to_account_info(),
        to: ctx.accounts.admin_ata.to_account_info(),
        authority: ctx.accounts.central_state.to_account_info(),
    };
    let signer_seeds: &[&[&[u8]]] =
        &[&[CENTRAL_STATE_SEED, &[ctx.accounts.central_state.bump]]];
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts)
        .with_signer(signer_seeds);
    let decimals = ctx.accounts.a_mint.decimals;
    transfer_checked(cpi_ctx, token_balance, decimals)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_utils::TestRunner;
    use solana_program::program_pack::Pack;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::pubkey::Pubkey;
    use test_case::test_case;

    fn setup_test() -> (TestRunner, Keypair, Pubkey, Pubkey, Pubkey) {
        // Parameters
        let admin_fees_balance = 500; // Start with some admin fees available

        let mut runner = TestRunner::new();
        let admin = Keypair::new();
        
        runner.airdrop(&admin.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&admin, 9);
        let admin_ata = runner.create_associated_token_account(&admin, a_mint, &admin.pubkey());

        let central_state = runner.create_central_state_mock(&admin, 5, 5, 2, 1, 10000);
        // central state ata
        let central_state_ata = runner.create_associated_token_account(&admin, a_mint, &central_state);
        runner.mint_tokens(&admin, central_state, a_mint, admin_fees_balance);

        (runner, admin, central_state_ata, admin_ata, a_mint)
    }

    #[test]
    fn test_claim_admin_fees() {
        let (mut runner, admin, central_state_ata, admin_ata, a_mint) = setup_test();

        // Claim admin fees
        let result = runner.claim_admin_fees(
            &admin,
            admin_ata,
            a_mint,
        );
        assert!(result.is_ok());

        // Check that admin fees_balance was subtracted from central state pda
        let central_state_account = runner.svm.get_account(&central_state_ata).unwrap();
        let central_state_balance = anchor_spl::token::spl_token::state::Account::unpack(&central_state_account.data).unwrap().amount;
        assert_eq!(central_state_balance, 0);

        // Check that admin ATA balance increased by the claimed amount
        let admin_ata_account = runner.svm.get_account(&admin_ata).unwrap();
        let final_admin_balance = anchor_spl::token::spl_token::state::Account::unpack(&admin_ata_account.data).unwrap().amount;
        assert_eq!(final_admin_balance, 500);
    }

    #[test]
    fn test_claim_admin_fees_wrong_authority() {
        let (mut runner, _, _, _, _) = setup_test();

        let other_user = Keypair::new();
        runner.airdrop(&other_user.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&other_user, 9);
        let other_user_ata = runner.create_associated_token_account(&other_user, a_mint, &other_user.pubkey());

        // Claim admin fees
        let result = runner.claim_admin_fees(
            &other_user,
            other_user_ata,
            a_mint,
        );
        assert!(result.is_err());
    }
}
