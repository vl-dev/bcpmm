use crate::errors::CbmmError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[derive(Accounts)]
pub struct ClaimPlatformFees<'info> {
    #[account(mut, address = platform_config.admin @ CbmmError::InvalidPlatformAdmin)]
    pub admin: Signer<'info>,

    #[account(mut,
        associated_token::mint = quote_mint,
        associated_token::authority = admin,
        associated_token::token_program = token_program
    )]
    pub admin_ata: InterfaceAccount<'info, TokenAccount>,

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

    #[account(address = pool.platform_config)]
    pub platform_config: Account<'info, PlatformConfig>,

    pub quote_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_platform_fees(ctx: Context<ClaimPlatformFees>) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let amount = pool.platform_fees_balance;
    if amount == 0 {
        return Ok(()); // No-op
    }
    pool.platform_fees_balance = 0;
    let pool_account_info = pool.to_account_info();
    pool.transfer_out(
        amount,
        &pool_account_info,
        &ctx.accounts.quote_mint,
        &ctx.accounts.pool_ata,
        &ctx.accounts.admin_ata,
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
    use solana_sdk::instruction::AccountMeta;
    use solana_sdk::pubkey::Pubkey as SdkPubkey;
    use solana_sdk::signature::{Keypair, Signer};

    fn setup_test() -> (
        TestRunner,
        Keypair,
        SdkPubkey,
        SdkPubkey,
        SdkPubkey,
        SdkPubkey,
    ) {
        // Parameters
        let platform_fees_balance = 500;
        let creator_fee_bp = 200;
        let buyback_fee_bp = 600;
        let platform_fee_bp = 200;

        let mut runner = TestRunner::new();
        let admin = Keypair::new();
        let creator = Keypair::new();

        runner.airdrop(&admin.pubkey(), 10_000_000_000);
        runner.airdrop(&creator.pubkey(), 10_000_000_000);

        let quote_mint = runner.create_mint(&admin, 9);
        let admin_ata = runner.create_associated_token_account(&admin, quote_mint, &admin.pubkey());

        let platform_config = runner.create_platform_config_mock(
            &admin,
            quote_mint,
            5,
            5,
            2,
            1,
            creator_fee_bp,
            buyback_fee_bp,
            platform_fee_bp,
        );

        let pool_created = runner.create_pool_mock(
            &creator,
            platform_config,
            quote_mint,
            0,
            1_000_000,
            2_000_000,
            2_000_000,
            6,
            creator_fee_bp,
            buyback_fee_bp,
            platform_fee_bp,
            0,
            0,
            0,
        );

        // Manually inject platform fees into pool state
        let pool_account = runner.svm.get_account(&pool_created.pool).unwrap();
        let mut pool_data = CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        pool_data.platform_fees_balance = platform_fees_balance;
        runner.put_account_on_chain(&pool_created.pool, pool_data);

        // Create pool ATA and mint tokens to it to match the balance
        let pool_ata =
            runner.create_associated_token_account(&creator, quote_mint, &pool_created.pool);
        runner.mint_tokens(&admin, pool_created.pool, quote_mint, platform_fees_balance);

        (
            runner,
            admin,
            pool_created.pool,
            pool_ata,
            admin_ata,
            quote_mint,
        )
    }

    #[test]
    fn test_claim_platform_fees() {
        let (mut runner, admin, pool, pool_ata, admin_ata, quote_mint) = setup_test();

        let pool_account = runner.svm.get_account(&pool).unwrap();
        let pool_data = CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        // platform_config is likely SdkPubkey or Anchor Pubkey depending on CbmmPool definition.
        // CbmmPool uses Anchor Pubkey. We need SdkPubkey for AccountMeta.
        let platform_config = SdkPubkey::new_from_array(pool_data.platform_config.to_bytes());

        let accounts = vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(admin_ata, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(pool_ata, false),
            AccountMeta::new_readonly(platform_config, false),
            AccountMeta::new_readonly(quote_mint, false),
            AccountMeta::new_readonly(
                SdkPubkey::new_from_array(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new_readonly(
                SdkPubkey::new_from_array(anchor_spl::associated_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new_readonly(
                SdkPubkey::new_from_array(
                    anchor_lang::solana_program::system_program::ID.to_bytes(),
                ),
                false,
            ),
        ];

        let result = runner.send_instruction("claim_platform_fees", accounts, (), &[&admin]);
        if let Err(ref e) = result {
            eprintln!("claim_platform_fees error: {:?}", e);
        }
        assert!(result.is_ok());

        // Check that platform fees_balance was subtracted from pool
        let pool_account = runner.svm.get_account(&pool).unwrap();
        let pool_data = CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.platform_fees_balance, 0);

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
        let (mut runner, _, pool, pool_ata, _, quote_mint) = setup_test();

        let other_user = Keypair::new();
        runner.airdrop(&other_user.pubkey(), 10_000_000_000);
        let other_user_ata =
            runner.create_associated_token_account(&other_user, quote_mint, &other_user.pubkey());

        let pool_account = runner.svm.get_account(&pool).unwrap();
        let pool_data = CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config = SdkPubkey::new_from_array(pool_data.platform_config.to_bytes());

        let accounts = vec![
            AccountMeta::new(other_user.pubkey(), true),
            AccountMeta::new(other_user_ata, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(pool_ata, false),
            AccountMeta::new_readonly(platform_config, false),
            AccountMeta::new_readonly(quote_mint, false),
            AccountMeta::new_readonly(
                SdkPubkey::new_from_array(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new_readonly(
                SdkPubkey::new_from_array(anchor_spl::associated_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new_readonly(
                SdkPubkey::new_from_array(
                    anchor_lang::solana_program::system_program::ID.to_bytes(),
                ),
                false,
            ),
        ];

        let result = runner.send_instruction("claim_platform_fees", accounts, (), &[&other_user]);
        assert!(result.is_err());
    }
}
