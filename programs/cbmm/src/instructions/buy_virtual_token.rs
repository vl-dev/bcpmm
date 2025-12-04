use crate::errors::CbmmError;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

#[event]
pub struct BuyEvent {
    pub quote_input: u64,
    pub base_output: u64,

    pub creator_fees: u64,
    pub buyback_fees: u64,
    pub platform_fees: u64,

    pub topup_paid: u64,

    pub new_b_reserve: u64,
    pub new_a_reserve: u64,
    pub new_outstanding_topup: u64,
    pub new_creator_fees_balance: u64,
    pub new_buyback_fees_balance: u64,

    pub buyer: Pubkey,
    pub pool: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BuyVirtualTokenArgs {
    /// quote_amount is the amount of Mint A to swap for Mint B. Includes decimals.
    pub quote_amount: u64,

    /// The minimum amount of Mint B to receive. If below this, the transaction will fail.
    pub base_amount_min: u64,
}

#[derive(Accounts)]
pub struct BuyVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut,
        associated_token::mint = quote_mint,
        associated_token::authority = payer,
        associated_token::token_program = token_program        
    )]
    pub payer_ata: InterfaceAccount<'info, TokenAccount>,
    // todo check owner (or maybe not? can buy for other user)
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

    #[account(address = pool.quote_mint @ CbmmError::InvalidMint)]
    pub quote_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn buy_virtual_token(ctx: Context<BuyVirtualToken>, args: BuyVirtualTokenArgs) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;

    let fees = pool.calculate_fees(args.a_amount)?;
    let real_swap_amount = args.a_amount - fees.total_fees_amount();

    let output_amount = pool.calculate_base_output_amount(real_swap_amount);
    require_gt!(output_amount, 0, CbmmError::AmountTooSmall);
    require_gte!(output_amount, args.b_amount_min, CbmmError::SlippageExceeded);

    virtual_token_account.add(output_amount, &fees)?;

    // Update the pool state
    let real_topup_amount = pool.quote_outstanding_topup.min(fees.buyback_fees_amount);
    pool.quote_outstanding_topup -= real_topup_amount;    
    pool.buyback_fees_balance += fees.buyback_fees_amount - real_topup_amount;
    pool.creator_fees_balance += fees.creator_fees_amount;
    pool.quote_reserve += real_swap_amount + real_topup_amount;
    pool.base_reserve -= output_amount;

    // Transfer A tokens to pool ata, excluding platform fees
    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.payer_ata.to_account_info(),
        to: ctx.accounts.pool_ata.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
    transfer_checked(
        cpi_context,
         args.a_amount - fees.platform_fees_amount,
         ctx.accounts.a_mint.decimals)?;


    // Transfer platform fees to platform config ata
    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.payer_ata.to_account_info(),
        to: ctx.accounts.platform_config_ata.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
    transfer_checked(
        cpi_context,
        fees.platform_fees_amount,
        ctx.accounts.a_mint.decimals
    )?;
    emit!(BuyEvent {
        quote_input: args.a_amount,
        base_output: output_amount,
        creator_fees: fees.creator_fees_amount,
        buyback_fees: fees.buyback_fees_amount,
        platform_fees: fees.platform_fees_amount,
        topup_paid: real_topup_amount,
        new_b_reserve: pool.base_reserve,
        new_a_reserve: pool.quote_reserve,
        new_outstanding_topup: pool.quote_outstanding_topup,
        new_creator_fees_balance: pool.creator_fees_balance,
        new_buyback_fees_balance: pool.buyback_fees_balance,
        buyer: ctx.accounts.payer.key(),
        pool: ctx.accounts.pool.key(),
    }); 
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::state::CbmmPool;
    use crate::test_utils::{TestRunner, TestPool};
    use anchor_lang::prelude::*;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::pubkey::Pubkey;

    fn setup_test() -> (TestRunner, Keypair, Keypair, TestPool, Pubkey, Pubkey) {
        // Parameters
        let quote_reserve = 0;
        let quote_virtual_reserve = 1_000_000;
        let base_reserve = 2_000_000;
        let base_mint_decimals = 6;
        let creator_fee_basis_points = 200;
        let buyback_fee_basis_points = 600;
        let platform_fee_basis_points = 200;
        let creator_fees_balance = 0;
        let buyback_fees_balance = 0;
        let quote_outstanding_topup = 100;

        let mut runner = TestRunner::new();
        let payer = Keypair::new();
        let another_wallet = Keypair::new();
        
        runner.airdrop(&payer.pubkey(), 10_000_000_000);
        runner.airdrop(&another_wallet.pubkey(), 10_000_000_000);
        let quote_mint = runner.create_mint(&payer, 9);
        let payer_ata = runner.create_associated_token_account(&payer, quote_mint, &payer.pubkey());
        runner.mint_to(&payer, &a_mint, payer_ata, 10_000_000_000);

        let platform_config = runner.create_platform_config_mock(&payer,
            quote_mint,
            5,
            5,
            2,
            1,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points);
        // platform config ata
        runner.create_associated_token_account(&payer, quote_mint, &platform_config);

        let test_pool = runner.create_pool_mock(
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
        runner.create_associated_token_account(&payer, quote_mint, &test_pool.pool);

        (runner, payer, another_wallet, test_pool, payer_ata, quote_mint)
    }

    #[test]
    fn test_buy_virtual_token_success() {
        let (mut runner, payer, _, pool, payer_ata, quote_mint) = setup_test();
        
        let quote_amount = 5000;
        let quote_virtual_reserve = 1_000_000;
        let base_reserve = 2_000_000;

        let quote_outstanding_topup = 100;
        let creator_fees = 100;
        let buyback_fees = 300;
        let buyback_fees_after_topup = buyback_fees - quote_outstanding_topup;
        let platform_fees = 100;
        let quote_amount_after_fees = quote_amount - creator_fees - buyback_fees_after_topup - platform_fees;

        let calculated_b_amount_min = 8959;
        let virtual_token_account =
            runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);

        let result_buy = runner.buy_virtual_token(
            &payer,
            payer_ata,
            quote_mint,
            pool.pool,
            virtual_token_account,
            quote_amount,
            calculated_b_amount_min,
        );
        result_buy.unwrap();
        //assert!(result_buy.is_ok());

        // Check that the reserves are updated correctly
        let pool_account = runner.svm.get_account(&pool.pool).unwrap();
        let pool_data: CbmmPool =
            CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        assert_eq!(pool_data.quote_reserve, quote_amount_after_fees);
        assert_eq!(pool_data.base_reserve, base_reserve - calculated_b_amount_min);
        assert_eq!(pool_data.quote_virtual_reserve, quote_virtual_reserve); // Unchanged
        assert_eq!(pool_data.buyback_fees_balance, buyback_fees_after_topup);
        assert_eq!(pool_data.creator_fees_balance, creator_fees);
        assert_eq!(pool_data.quote_outstanding_topup, 0);
    }

    #[test]
    fn test_buy_virtual_token_slippage_exceeded() {
        let (mut runner, payer, _, pool, payer_ata, quote_mint) = setup_test();
        
        let quote_amount = 5000;
        let calculated_b_amount_min = 9157;

        let virtual_token_account =
            runner.create_virtual_token_account_mock(payer.pubkey(), pool.pool, 0, 0);

        let result_buy_min_too_high = runner.buy_virtual_token(
            &payer,
            payer_ata,
            quote_mint,
            pool.pool,
            virtual_token_account,
            quote_amount,
            calculated_b_amount_min + 1, // Set minimum too high
        );
        assert!(result_buy_min_too_high.is_err());
    }

    #[test]
    fn test_buy_virtual_token_wrong_virtual_account_owner() {
        let (mut runner, payer, another_wallet, pool, payer_ata, quote_mint) = setup_test();
        
        let quote_amount = 5000;
        let calculated_b_amount_min = 9157;

        let virtual_token_account_another_wallet =
            runner.create_virtual_token_account_mock(another_wallet.pubkey(), pool.pool, 0, 0);

        let result_buy_another_virtual_account = runner.buy_virtual_token(
            &payer,
            payer_ata,
            quote_mint,
            pool.pool,
            virtual_token_account_another_wallet,
            quote_amount,
            calculated_b_amount_min,
        );
        assert!(result_buy_another_virtual_account.is_err());
    }
}
