use crate::errors::BcpmmError;
use crate::helpers::{calculate_buy_output_amount, calculate_fees};
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct BuyVirtualTokenArgs {
    /// a_amount is the amount of Mint A to swap for Mint B. Includes decimals.
    pub a_amount: u64,
}

#[derive(Accounts)]
pub struct BuyVirtualToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub payer_ata: InterfaceAccount<'info, TokenAccount>,
    // todo check owner (or maybe not? can buy for other user)
    #[account(mut)]
    pub virtual_token_account: Account<'info, VirtualTokenAccount>,
    #[account(mut, seeds = [BCPMM_POOL_SEED, b_mint.key().as_ref()], bump)]
    pub pool: Account<'info, BcpmmPool>,
    // todo check owner
    #[account(mut)]
    pub pool_ata: InterfaceAccount<'info, TokenAccount>,
    pub a_mint: InterfaceAccount<'info, Mint>,
    /// UNCHECKED: this is a virtual mint so it doesn't really exist
    pub b_mint: AccountInfo<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn buy_virtual_token(ctx: Context<BuyVirtualToken>, args: BuyVirtualTokenArgs) -> Result<()> {
    let virtual_token_account = &mut ctx.accounts.virtual_token_account;

    let fees = calculate_fees(
        args.a_amount,
        ctx.accounts.pool.creator_fee_basis_points,
        ctx.accounts.pool.buyback_fee_basis_points,
    )?;

    let swap_amount = args.a_amount - fees.creator_fees_amount - fees.buyback_fees_amount;

    let output_amount = calculate_buy_output_amount(
        swap_amount,
        ctx.accounts.pool.a_reserve,
        ctx.accounts.pool.b_reserve,
        ctx.accounts.pool.a_virtual_reserve,
    );

    if output_amount == 0 {
        return Err(BcpmmError::AmountTooSmall.into());
    }

    virtual_token_account.balance += output_amount;
    virtual_token_account.fees_paid += fees.creator_fees_amount + fees.buyback_fees_amount;
    ctx.accounts.pool.a_reserve += swap_amount;
    ctx.accounts.pool.b_reserve -= output_amount;
    ctx.accounts.pool.creator_fees_balance += fees.creator_fees_amount;
    let remaining_topup_amount = ctx.accounts.pool.a_remaining_topup;
    if remaining_topup_amount > 0 {
        let buyback_fees_amount = fees.buyback_fees_amount;
        let real_topup_amount = if remaining_topup_amount > buyback_fees_amount {
            buyback_fees_amount
        } else {
            remaining_topup_amount
        };
        ctx.accounts.pool.a_remaining_topup =
            ctx.accounts.pool.a_remaining_topup - real_topup_amount;
        ctx.accounts.pool.a_reserve += real_topup_amount;
    } else {
        ctx.accounts.pool.buyback_fees_balance += fees.buyback_fees_amount;
    }

    let cpi_accounts = TransferChecked {
        mint: ctx.accounts.a_mint.to_account_info(),
        from: ctx.accounts.payer_ata.to_account_info(),
        to: ctx.accounts.pool_ata.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
    transfer_checked(cpi_context, args.a_amount, ctx.accounts.a_mint.decimals)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use litesvm::LiteSVM;
    use litesvm_token::{
        spl_token::{native_mint::DECIMALS, ID as SPL_TOKEN_ID},
        CreateAssociatedTokenAccount, CreateMint, MintTo,
    };
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        transaction::Transaction,
    };
    use solana_sdk_ids::system_program;

    use crate::state::{
        BcpmmPool, VirtualTokenAccount, BCPMM_POOL_SEED, VIRTUAL_TOKEN_ACCOUNT_SEED,
    };
    use sha2::{Digest, Sha256};

    // Helper function to calculate instruction discriminator
    fn get_discriminator(instruction_name: &str) -> [u8; 8] {
        let mut hasher = Sha256::new();
        hasher.update(format!("global:{}", instruction_name));
        let result = hasher.finalize();
        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&result[..8]);
        discriminator
    }

    struct TestData {
        svm: LiteSVM,
        payer: Keypair,
        payer_ata: Pubkey,
        virtual_token_account: Pubkey,
        pool: Pubkey,
        pool_ata: Pubkey,
        a_mint: Pubkey,
        b_mint: Pubkey,
        program_id: Pubkey,
    }

    fn setup() -> TestData {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

        // Create a new SPL token mint with the payer as the mint authority
        let mint = CreateMint::new(&mut svm, &payer)
            .authority(&payer.pubkey())
            .decimals(DECIMALS)
            .send()
            .unwrap();

        // Create an ATA for the payer
        let associated_token_account = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
            .owner(&payer.pubkey())
            .send()
            .unwrap();

        // Mint 1000 tokens to Alice's account
        MintTo::new(&mut svm, &payer, &mint, &associated_token_account, 1000000)
            .owner(&payer)
            .send()
            .unwrap();

        // Deploy your program to the test environment
        let program_id = Pubkey::from(crate::ID.to_bytes());
        let program_bytes = include_bytes!("../../../../target/deploy/cpmm_poc.so");
        svm.add_program(program_id, program_bytes).unwrap();

        // Setup PDAs consistent with on-chain seeds
        // Mock a virtual B mint pubkey just for PDA derivation (B is virtual in program logic)
        let mock_b_mint = Pubkey::new_unique();
        let (pool_pda, pool_bump) =
            Pubkey::find_program_address(&[BCPMM_POOL_SEED, mock_b_mint.as_ref()], &program_id);

        // Derive the VirtualTokenAccount PDA using pool + owner seeds
        let (vta_pda, vta_bump) = Pubkey::find_program_address(
            &[
                VIRTUAL_TOKEN_ACCOUNT_SEED,
                pool_pda.as_ref(),
                payer.pubkey().as_ref(),
            ],
            &program_id,
        );

        // Create pool PDA account with BcpmmPool structure
        let pool_data = BcpmmPool {
            creator: anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes()),
            a_mint: anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
            a_reserve: 0,
            a_virtual_reserve: 1000000,
            a_remaining_topup: 0,
            b_mint: anchor_lang::prelude::Pubkey::from(mock_b_mint.to_bytes()),
            b_mint_decimals: 6,
            b_reserve: 1000000,
            creator_fees_balance: 0,
            buyback_fees_balance: 0,
            creator_fee_basis_points: 100,
            buyback_fee_basis_points: 200,
        };

        let mut pool_account_data = Vec::new();
        pool_data.try_serialize(&mut pool_account_data).unwrap();

        let pool_ata_pubkey = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint)
            .owner(&pool_pda)
            .send()
            .unwrap();

        svm.set_account(
            pool_pda,
            solana_sdk::account::Account {
                lamports: 1_000_000,
                data: pool_account_data,
                owner: program_id,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();

        // Create VTA PDA account with VirtualTokenAccount structure
        let vta_data = VirtualTokenAccount {
            pool: anchor_lang::prelude::Pubkey::from(pool_pda.to_bytes()),
            owner: anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes()),
            balance: 0,
            fees_paid: 0,
        };

        let mut vta_account_data = Vec::new();
        vta_data.try_serialize(&mut vta_account_data).unwrap();

        svm.set_account(
            vta_pda,
            solana_sdk::account::Account {
                lamports: 1_000_000,
                data: vta_account_data,
                owner: program_id,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();

        TestData {
            svm,
            payer,
            payer_ata: associated_token_account,
            virtual_token_account: vta_pda,
            pool: pool_pda,
            pool_ata: pool_ata_pubkey,
            a_mint: mint,
            b_mint: mock_b_mint,
            program_id,
        }
    }

    #[test]
    fn test_my_program() {
        // Initialize the test environment
        let mut test_data = setup();
        // Create and fund test accounts

        // print token program

        let accounts = vec![
            AccountMeta::new(test_data.payer.pubkey(), true),
            AccountMeta::new(test_data.payer_ata, false),
            AccountMeta::new(test_data.virtual_token_account, false),
            AccountMeta::new(test_data.pool, false),
            AccountMeta::new(test_data.pool_ata, false),
            AccountMeta::new(test_data.a_mint, false),
            AccountMeta::new(test_data.b_mint, false),
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new(system_program::ID, false),
        ];

        let args = BuyVirtualTokenArgs { a_amount: 50000 };

        let instruction = Instruction {
            program_id: Pubkey::from(crate::ID.to_bytes()),
            accounts: accounts,
            data: {
                let mut data = Vec::new();
                // Add instruction discriminator for buy_virtual_token
                data.extend_from_slice(&get_discriminator("buy_virtual_token"));
                args.serialize(&mut data).unwrap();
                data
            },
        };
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&test_data.payer.pubkey()),
            &[&test_data.payer],
            test_data.svm.latest_blockhash(),
        );
        let result = test_data.svm.send_transaction(tx);
        if result.is_err() {
            println!("Transaction failed: {:?}", result.err());
            assert!(false);
        } else {
            println!("Transaction logs: {:?}", result.unwrap().logs);
            println!("Program executed successfully!");
            // get pool balance
            let pool_balance = test_data.svm.get_account(&test_data.pool).unwrap().data;
            let pool = BcpmmPool::try_deserialize(&mut &pool_balance[..]).unwrap();
            println!("Pool a reserve: {:?}", pool.a_reserve);
            println!("Pool b reserve: {:?}", pool.b_reserve);
            println!("Pool a virtual reserve: {:?}", pool.a_virtual_reserve);
            println!("Pool a remaining topup: {:?}", pool.a_remaining_topup);
            println!("Pool creator fees balance: {:?}", pool.creator_fees_balance);
            println!("Pool buyback fees balance: {:?}", pool.buyback_fees_balance);
            println!(
                "Pool creator fee basis points: {:?}",
                pool.creator_fee_basis_points
            );
            println!(
                "Pool buyback fee basis points: {:?}",
                pool.buyback_fee_basis_points
            );

            // get virtual token account balance
            let virtual_token_account_balance = test_data
                .svm
                .get_account(&test_data.virtual_token_account)
                .unwrap()
                .data;
            let virtual_token_account =
                VirtualTokenAccount::try_deserialize(&mut &virtual_token_account_balance[..])
                    .unwrap();
            println!(
                "Virtual token account balance: {:?}",
                virtual_token_account.balance
            );
            println!(
                "Virtual token account fees paid: {:?}",
                virtual_token_account.fees_paid
            );
        }
    }
}
