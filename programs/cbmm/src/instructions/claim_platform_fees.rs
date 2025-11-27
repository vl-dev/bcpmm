use crate::errors::BcpmmError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

#[derive(Accounts)]
pub struct ClaimPlatformFees<'info> {
    #[account(mut, address = platform_config.admin @ BcpmmError::InvalidPlatformAdmin)]
    pub admin: Signer<'info>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = platform_config.admin,
        associated_token::token_program = token_program
    )]
    pub admin_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [PLATFORM_CONFIG_SEED, platform_config.creator.key().as_ref()], bump = platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut,
        associated_token::mint = a_mint,
        associated_token::authority = platform_config,
        associated_token::token_program = token_program
    )]
    pub platform_config_ata: InterfaceAccount<'info, TokenAccount>,

    pub a_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_platform_fees(ctx: Context<ClaimPlatformFees>) -> Result<()> {
    // Transfer from platform config's ATA to admin's ATA.
    let token_balance = ctx.accounts.platform_config_ata.amount;
    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.platform_config_ata.to_account_info(),
        to: ctx.accounts.admin_ata.to_account_info(),
        authority: ctx.accounts.platform_config.to_account_info(),
    };
    let creator_key = ctx.accounts.platform_config.creator.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        PLATFORM_CONFIG_SEED,
        creator_key.as_ref(),
        &[ctx.accounts.platform_config.bump],
    ]];
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts)
        .with_signer(signer_seeds);
    let decimals = ctx.accounts.a_mint.decimals;
    transfer_checked(cpi_ctx, token_balance, decimals)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_utils::TestRunner;
    use anchor_lang::solana_program::program_pack::Pack;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::{Keypair, Signer};

    fn setup_test() -> (TestRunner, Keypair, Pubkey, Pubkey, Pubkey) {
        // Parameters
        let platform_fees_balance = 500; // Start with some platform fees available
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let platform_fee_basis_points = 200;

        let mut runner = TestRunner::new();
        let admin = Keypair::new();

        runner.airdrop(&admin.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&admin, 9);
        let admin_ata = runner.create_associated_token_account(&admin, a_mint, &admin.pubkey());

        let platform_config = runner.create_platform_config_mock(
            &admin,
            a_mint,
            5,
            5,
            5,
            2,
            1,
            10000,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
        );
        // platform config ata
        let platform_config_ata =
            runner.create_associated_token_account(&admin, a_mint, &platform_config);
        runner.mint_tokens(&admin, platform_config, a_mint, platform_fees_balance);

        (runner, admin, platform_config_ata, admin_ata, a_mint)
    }

    #[test]
    fn test_claim_platform_fees() {
        let (mut runner, admin, platform_config_ata, admin_ata, a_mint) = setup_test();

        // Claim platform fees
        let result = runner.claim_platform_fees(&admin, admin.pubkey(), admin_ata, a_mint);
        assert!(result.is_ok());

        // Check that platform fees_balance was subtracted from platform config pda
        let platform_config_account = runner.svm.get_account(&platform_config_ata).unwrap();
        let platform_config_balance =
            anchor_spl::token::spl_token::state::Account::unpack(&platform_config_account.data)
                .unwrap()
                .amount;
        assert_eq!(platform_config_balance, 0);

        // Check that admin ATA balance increased by the claimed amount
        let admin_ata_account = runner.svm.get_account(&admin_ata).unwrap();
        let final_admin_balance =
            anchor_spl::token::spl_token::state::Account::unpack(&admin_ata_account.data)
                .unwrap()
                .amount;
        assert_eq!(final_admin_balance, 500);
    }

    #[test]
    fn test_claim_platform_fees_wrong_authority() {
        let (mut runner, _, _, _, _) = setup_test();

        let other_user = Keypair::new();
        runner.airdrop(&other_user.pubkey(), 10_000_000_000);
        let a_mint = runner.create_mint(&other_user, 9);
        let other_user_ata =
            runner.create_associated_token_account(&other_user, a_mint, &other_user.pubkey());

        // Claim platform fees
        let result =
            runner.claim_platform_fees(&other_user, other_user.pubkey(), other_user_ata, a_mint);
        assert!(result.is_err());
    }
}
