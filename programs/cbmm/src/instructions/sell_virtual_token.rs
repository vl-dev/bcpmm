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

    pub fees: u64,

    pub topup_paid: u64,

    pub new_base_reserve: u64,
    pub new_quote_reserve: u64,

    pub seller: Pubkey,
    pub pool: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SellVirtualTokenArgs {
    pub base_amount: u64,
    pub min_quote_amount: u64,
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

    #[account(mut,
        seeds = [
            VIRTUAL_TOKEN_ACCOUNT_SEED,
            pool.key().as_ref(),
            payer.key().as_ref()
        ],
        bump = virtual_token_account.bump,
    )]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,

    #[account(mut,
        seeds = [
            CBMM_POOL_SEED,
            pool.pool_index.to_le_bytes().as_ref(),
            pool.creator.as_ref(),
            platform_config.key().as_ref(),
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

    pub platform_config: Account<'info, PlatformConfig>,

    #[account(address = pool.quote_mint @ CbmmError::InvalidMint)]
    pub quote_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn sell_virtual_token(
    ctx: Context<SellVirtualToken>,
    args: SellVirtualTokenArgs,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;
    
    require_gte!(virtual_token_account.balance, args.base_amount, CbmmError::InsufficientVirtualTokenBalance);
    
    // Calculate swap
    let swap_result = pool.base_to_quote(args.base_amount)?;
    let gross_output = swap_result.quote_amount;

    // Calculate fees on output
    let net_output = pool.collect_fees(gross_output)?;
    require_gt!(net_output, 0, CbmmError::AmountTooSmall);
    require_gte!(
        net_output,
        args.min_quote_amount,
        CbmmError::SlippageExceeded
    );

    let fees = gross_output - net_output;
    let topup_amount = pool.topup()?;

    // Update user virtual balance
    virtual_token_account.sub(args.base_amount)?;

    // Transfer Quote tokens from pool to user
    let pool_account_info = pool.to_account_info();
    pool.transfer_out(
        net_output,
        &pool_account_info,
        &ctx.accounts.quote_mint,
        &ctx.accounts.pool_ata,
        &ctx.accounts.payer_ata,
        &ctx.accounts.token_program
    )?;

    emit!(SellEvent {
        base_input: args.base_amount,
        quote_output: net_output,
        fees,
        topup_paid: topup_amount,
        new_base_reserve: pool.base_reserve,
        new_quote_reserve: pool.quote_reserve,
        seller: ctx.accounts.payer.key(),
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
    use solana_sdk::pubkey::Pubkey;

    fn setup_test() -> (TestRunner, Keypair, Keypair, TestPool, Pubkey, Pubkey) {
        // Parameters
        let quote_reserve = 2000;
        let quote_virtual_reserve = 2000;
        let base_reserve = 500;
        let base_total_supply = 1000;
        let base_mint_decimals = 6;
        let creator_fee_bp = 200;
        let buyback_fee_bp = 600;
        let platform_fee_bp = 200;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 150; // Pre-fund buyback fees for topup test
        let quote_outstanding_topup = 150;

        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let another_wallet = Keypair::new();
        
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        runner.airdrop(&another_wallet.pubkey(), 10_000_000_000);
        let quote_mint = runner.create_mint(&payer, 9);
        let payer_ata = runner.create_associated_token_account(&payer, quote_mint, &payer.pubkey());
        runner.mint_to(&payer, &quote_mint, payer_ata, 10_000_000_000);
        
        let platform_config = runner.create_platform_config_mock(
            &payer,
            quote_mint,
            5,
            5,
            2,
            1,
            creator_fee_bp,
            buyback_fee_bp,
            platform_fee_bp,
            None,
        );

        // platform config ata
        runner.create_associated_token_account(&payer, quote_mint, &platform_config);

        let test_pool = runner.create_pool_mock(
            &payer,
            platform_config,
            quote_mint,
            quote_reserve,
            quote_virtual_reserve,
            base_reserve,
            base_total_supply,
            base_mint_decimals,
            creator_fee_bp,
            buyback_fee_bp,
            platform_fee_bp,
            creator_fees_balance,
            buyback_fees_balance,
            quote_outstanding_topup,
        );

        // pool ata
        runner.create_associated_token_account(&payer, quote_mint, &test_pool.pool);
        runner.mint_tokens(&payer, test_pool.pool, quote_mint, quote_reserve + buyback_fees_balance); // Mint reserve + fees
        (runner, payer, another_wallet, test_pool, payer_ata, quote_mint)
    }

    #[test]
    fn test_sell_virtual_token_success() {
        let (mut runner, payer, _, pool, payer_ata, quote_mint) = setup_test();        
        
        let base_amount = 1000; // User has
        let base_sell_amount = 500;
        let base_total_supply = 1000;

        // Create virtual token account with some balance to sell
        let virtual_token_account = runner.create_virtual_token_account_mock(
            payer.pubkey(),
            pool.pool,
            base_amount, // Start with balance to sell
        );

        let result_sell = runner.sell_virtual_token(
            &payer,
            payer_ata,
            quote_mint,
            pool.pool,
            virtual_token_account,
            base_sell_amount,
            0, // min_quote_amount = 0 for success test
        );
        result_sell.unwrap();
        // assert!(result_sell.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();

        // Verify state changes
        // Topup should have happened
        // Reserve increased by topup, decreased by gross output
        // Base reserve increased by sell amount
        assert_eq!(pool_data.base_reserve, base_total_supply);
        
        // Fees
        // creator fees should increase
        assert!(pool_data.creator_fees_balance > 0);
        assert!(pool_data.buyback_fees_balance > 0); // from new fees
        assert!(pool_data.platform_fees_balance > 0);
        
        // Balance
        // Check virtual balance updated
        let vta_account = runner.svm.get_account(&virtual_token_account).unwrap();
        let vta_data: crate::state::VirtualTokenAccount = crate::state::VirtualTokenAccount::try_deserialize(&mut vta_account.data.as_slice()).unwrap();
        assert_eq!(vta_data.balance, base_amount - base_sell_amount);
    }

    #[test]
    fn test_sell_virtual_token_insufficient_balance() {
        let (mut runner, _, another_wallet, pool, payer_ata, quote_mint) = setup_test();
        
        let base_amount = 1000;

        // Create virtual token account with insufficient balance
        let virtual_token_account_insufficient = runner.create_virtual_token_account_mock(
            another_wallet.pubkey(),
            pool.pool,
            base_amount - 1, // Insufficient balance
        );

        let result_sell_insufficient = runner.sell_virtual_token(
            &another_wallet,
            payer_ata,
            quote_mint,
            pool.pool,
            virtual_token_account_insufficient,
            base_amount,
            0,
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
            pool.pool,
            base_amount,
        );

        let result_sell_wrong_owner = runner.sell_virtual_token(
            &payer, // payer is different from virtual token account owner
            payer_ata,
            quote_mint,
            pool.pool,
            virtual_token_account_wrong_owner,
            base_amount,
            0,
        );
        assert!(result_sell_wrong_owner.is_err());
    }

    #[test]
    fn test_sell_virtual_token_slippage_exceeded() {
        let (mut runner, payer, _, pool, payer_ata, quote_mint) = setup_test();
        
        let base_amount = 1000;
        let base_sell_amount = 500;

        let virtual_token_account = runner.create_virtual_token_account_mock(
            payer.pubkey(),
            pool.pool,
            base_amount, 
        );

        // Try with impossibly high min_quote_amount
        let result_sell_slippage = runner.sell_virtual_token(
            &payer,
            payer_ata,
            quote_mint,
            pool.pool,
            virtual_token_account,
            base_sell_amount,
            100_000, // Too high
        );
        assert!(result_sell_slippage.is_err());
    }
}
