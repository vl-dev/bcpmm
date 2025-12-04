use crate::errors::CbmmError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    Mint, TokenAccount, TokenInterface,
};

#[event]
pub struct SellEvent {
    pub base_input: u64,
    pub quote_output: u64,

    pub creator_fees: u64,
    pub buyback_fees: u64,
    pub platform_fees: u64,

    pub topup_paid: u64,

    pub new_b_reserve: u64,
    pub new_a_reserve: u64,
    pub new_outstanding_topup: u64,
    pub new_creator_fees_balance: u64,
    pub new_buyback_fees_balance: u64,

    pub seller: Pubkey,
    pub pool: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SellVirtualTokenArgs {
    pub base_amount: u64,
}

#[derive(Accounts)]
pub struct SellVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut,
        associated_token::mint = quote_mint,
        associated_token::authority = payer,
        associated_token::token_program = token_program        
    )]
    pub payer_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [VIRTUAL_TOKEN_ACCOUNT_SEED, pool.key().as_ref(), payer.key().as_ref()], bump = virtual_token_account.bump)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,

    #[account(mut, seeds = [CBMM_POOL_SEED, pool.pool_index.to_le_bytes().as_ref(), pool.creator.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, CbmmPool>,

    #[account(mut,
        associated_token::mint = quote_mint,
        associated_token::authority = pool,
        associated_token::token_program = token_program        
    )]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut,
        associated_token::mint = quote_mint,
        associated_token::authority = platform_config,
        associated_token::token_program = token_program
    )]
    pub platform_config_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, address = pool.platform_config)]
    pub platform_config: Account<'info, PlatformConfig>,

    pub quote_mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}
pub fn sell_virtual_token(
    ctx: Context<SellVirtualToken>,
    args: SellVirtualTokenArgs,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;
    require_gte!(virtual_token_account.balance, args.b_amount, CbmmError::InsufficientVirtualTokenBalance);

    let output_amount = pool.calculate_quote_output_amount(args.b_amount);
    require_gte!(pool.quote_reserve, output_amount, CbmmError::Underflow);

    let fees = pool.calculate_fees(output_amount)?;
    virtual_token_account.sub(args.b_amount, &fees)?;

    // Update the pool state        
    let real_topup_amount = pool.quote_outstanding_topup.min(fees.buyback_fees_amount);
    pool.quote_outstanding_topup -= real_topup_amount;    
    pool.buyback_fees_balance += fees.buyback_fees_amount - real_topup_amount;
    pool.creator_fees_balance += fees.creator_fees_amount;
    pool.quote_reserve -= output_amount - real_topup_amount;
    pool.base_reserve += args.b_amount;    

    let pool_account_info = pool.to_account_info();
    pool.transfer_out(
        output_amount - fees.total_fees_amount(),
        &pool_account_info,
        &ctx.accounts.a_mint,
        &ctx.accounts.pool_ata,
        &ctx.accounts.payer_ata,
        &ctx.accounts.token_program
    )?;

    pool.transfer_out(
        fees.platform_fees_amount,
        &pool_account_info,
        &ctx.accounts.a_mint,
        &ctx.accounts.pool_ata,
        &ctx.accounts.platform_config_ata,
        &ctx.accounts.token_program,
    )?;
    emit!(SellEvent {
        base_input: args.b_amount,
        quote_output: output_amount,
        creator_fees: fees.creator_fees_amount,
        buyback_fees: fees.buyback_fees_amount,
        platform_fees: fees.platform_fees_amount,
        topup_paid: real_topup_amount,
        new_b_reserve: pool.base_reserve,
        new_a_reserve: pool.quote_reserve,
        new_outstanding_topup: pool.quote_outstanding_topup,
        new_creator_fees_balance: pool.creator_fees_balance,
        new_buyback_fees_balance: pool.buyback_fees_balance,
        seller: ctx.accounts.payer.key(),
        pool: ctx.accounts.pool.key(),
    }); 
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::state::CbmmPool;
    use crate::test_utils::TestRunner;
    use anchor_lang::prelude::*;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::pubkey::Pubkey;

    fn setup_test() -> (TestRunner, Keypair, Keypair, Pubkey, Pubkey, Pubkey) {
        // Parameters
        let quote_reserve = 5000;
        let quote_virtual_reserve = 1_000_000;
        let base_reserve = 2_000_000;
        let base_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let platform_fee_basis_points = 200;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;
        let quote_outstanding_topup = 150;

        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let another_wallet = Keypair::new();
        
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        runner.airdrop(&another_wallet.pubkey(), 10_000_000_000);
        let quote_mint = runner.create_mint(&payer, 9);
        let payer_ata = runner.create_associated_token_account(&payer, quote_mint, &payer.pubkey());
        runner.mint_to(&payer, &a_mint, payer_ata, 10_000_000_000);
        let platform_config = runner.create_platform_config_mock(&payer, quote_mint, 5, 5, 2, 1, creator_fee_basis_points, buyback_fee_basis_points, platform_fee_basis_points);

        // platform config ata
        runner.create_associated_token_account(&payer, quote_mint, &platform_config);

        let created_pool = runner.create_pool_mock(
            &payer,
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
        runner.create_associated_token_account(&payer, quote_mint, &created_pool.pool);
        runner.mint_tokens(&payer, created_pool.pool, quote_mint, quote_reserve);
        (runner, payer, another_wallet, created_pool.pool, payer_ata, quote_mint)
    }

    #[test]
    fn test_sell_virtual_token_success() {
        let (mut runner, payer, _, pool, payer_ata, quote_mint) = setup_test();
        
        let base_amount = 1000;
        let base_sell_amount = 500;
        let quote_reserve = 5000;
        let quote_virtual_reserve = 1_000_000;
        let base_reserve = 2_000_000;
        let buyback_fee_basis_points = 600;
        let quote_outstanding_topup = 150;

        // Create virtual token account with some balance to sell
        let virtual_token_account = runner.create_virtual_token_account_mock(
            payer.pubkey(),
            pool,
            base_amount, // Start with balance to sell
            0,
        );

        // Test successful sell
        let result_sell = runner.sell_virtual_token(
            &payer,
            payer_ata,
            quote_mint,
            pool,
            virtual_token_account,
            base_sell_amount,
        );
        assert!(result_sell.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();

        let expected_output_amount = 251;

        // account for topup repayment from buyback fees on sell (ceil division like in helpers)
        let buyback_fees = ((expected_output_amount as u128 * buyback_fee_basis_points as u128 + 9999) / 10_000) as u64;
        let real_topup = buyback_fees.min(a_outstanding_topup);
        let expected_a_reserve_after = quote_reserve - expected_output_amount + real_topup;
        assert_eq!(pool_data.quote_reserve, expected_a_reserve_after);
        assert_eq!(pool_data.base_reserve, base_reserve + base_sell_amount);
        assert_eq!(pool_data.quote_virtual_reserve, quote_virtual_reserve);
        assert_eq!(pool_data.quote_outstanding_topup, quote_outstanding_topup - buyback_fees);
    }

    #[test]
    fn test_sell_virtual_token_insufficient_balance() {
        let (mut runner, _, another_wallet, pool, payer_ata, quote_mint) = setup_test();
        
        let base_amount = 1000;

        // Create virtual token account with insufficient balance
        let virtual_token_account_insufficient = runner.create_virtual_token_account_mock(
            another_wallet.pubkey(),
            pool,
            base_amount - 1, // Insufficient balance
            0,
        );

        let result_sell_insufficient = runner.sell_virtual_token(
            &another_wallet,
            payer_ata,
            quote_mint,
            pool,
            virtual_token_account_insufficient,
            base_amount,
        );
        assert!(result_sell_insufficient.is_err());
    }

    #[test]
    fn test_sell_virtual_token_wrong_owner() {
        let (mut runner, payer, another_wallet, pool, payer_ata, quote_mint) = setup_test();
        
        let base_amount = 1000;

        // Create virtual token account with wrong owner
        let virtual_token_account_wrong_owner = runner.create_virtual_token_account_mock(
            another_wallet.pubkey(),
            pool,
            base_amount,
            0,
        );

        let result_sell_wrong_owner = runner.sell_virtual_token(
            &payer, // payer is different from virtual token account owner
            payer_ata,
            quote_mint,
            pool,
            virtual_token_account_wrong_owner,
            base_amount,
        );
        assert!(result_sell_wrong_owner.is_err());
    }

    #[test]
    fn test_sell_virtual_token_above_balance() {
        let (mut runner, payer, _, pool, payer_ata, quote_mint) = setup_test();
        
        let base_amount = 1000;

        // Create virtual token account with some balance to sell
        let virtual_token_account = runner.create_virtual_token_account_mock(
            payer.pubkey(),
            pool,
            base_amount, // Start with balance to sell
            0,
        );

        // Test trying to sell more than user has
        let result_sell_above_balance = runner.sell_virtual_token(
            &payer,
            payer_ata,
            quote_mint,
            pool,
            virtual_token_account,
            base_amount + 1,
        );
        assert!(result_sell_above_balance.is_err());
    }
}
