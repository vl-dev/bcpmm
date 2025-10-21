#[cfg(test)]
mod test_runner {
    use crate::instructions::BuyVirtualTokenArgs;
    use crate::state as cpmm_state;
    use anchor_lang::prelude::*;
    use litesvm::LiteSVM;
    use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        transaction::Transaction,
    };

    pub struct TestRunner {
        pub svm: LiteSVM,
        pub payer: Keypair,
        pub program_id: Pubkey,
        pub a_mint: Pubkey,
        pub payer_ata: Pubkey,
    }

    pub struct TestPool {
        pub pool: Pubkey,
        pub b_mint: Pubkey,
    }

    impl TestRunner {
        pub fn new(a_mint_decimals: u8) -> Self {
            let mut svm = LiteSVM::new();
            let payer = Keypair::new();
            svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

            // Deploy your program to the test environment
            let program_id = Pubkey::from(crate::ID.to_bytes());
            let program_bytes = include_bytes!("../../../../target/deploy/cpmm_poc.so");
            svm.add_program(program_id, program_bytes).unwrap();

            // Create a mint
            let a_mint = CreateMint::new(&mut svm, &payer)
                .authority(&payer.pubkey())
                .decimals(a_mint_decimals)
                .send()
                .unwrap();

            // Create an ATA for the payer
            let payer_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &a_mint)
                .owner(&payer.pubkey())
                .send()
                .unwrap();

            MintTo::new(&mut svm, &payer, &a_mint, &payer_ata, 10_000_000_000)
                .owner(&payer)
                .send()
                .unwrap();

            Self {
                svm,
                payer,
                payer_ata,
                program_id,
                a_mint,
            }
        }

        pub fn create_pool_mock(
            &mut self,
            a_reserve: u64,
            a_virtual_reserve: u64,
            b_reserve: u64,
            b_mint_decimals: u8,
            creator_fee_basis_points: u16,
            buyback_fee_basis_points: u16,
            creator_fees_balance: u64,
            buyback_fees_balance: u64,
        ) -> TestPool {
            let mock_b_mint = Pubkey::new_unique();

            // Setup PDAs consistent with on-chain seeds
            let (pool_pda, _pool_bump) = Pubkey::find_program_address(
                &[cpmm_state::BCPMM_POOL_SEED, mock_b_mint.as_ref()],
                &self.program_id,
            );

            // Create pool PDA account with BcpmmPool structure
            let pool_data = cpmm_state::BcpmmPool {
                creator: anchor_lang::prelude::Pubkey::from(self.payer.pubkey().to_bytes()),
                a_mint: anchor_lang::prelude::Pubkey::from(self.a_mint.to_bytes()),
                a_reserve,
                a_virtual_reserve,
                a_remaining_topup: 0,
                b_mint: anchor_lang::prelude::Pubkey::from(mock_b_mint.to_bytes()),
                b_mint_decimals,
                b_reserve,
                creator_fees_balance,
                buyback_fees_balance,
                creator_fee_basis_points,
                buyback_fee_basis_points,
            };

            let mut pool_account_data = Vec::new();
            pool_data.try_serialize(&mut pool_account_data).unwrap();

            let pool_ata_pubkey =
                CreateAssociatedTokenAccount::new(&mut self.svm, &self.payer, &self.a_mint)
                    .owner(&pool_pda)
                    .send()
                    .unwrap();

            self.svm
                .set_account(
                    pool_pda,
                    solana_sdk::account::Account {
                        lamports: 1_000_000,
                        data: pool_account_data,
                        owner: self.program_id,
                        executable: false,
                        rent_epoch: 0,
                    },
                )
                .unwrap();

            let needed_balance = a_reserve + creator_fees_balance + buyback_fees_balance;
            // mint appropriate amount of A tokens to pool
            MintTo::new(
                &mut self.svm,
                &self.payer,
                &self.a_mint,
                &pool_ata_pubkey,
                needed_balance,
            )
            .owner(&self.payer)
            .send()
            .unwrap();

            TestPool {
                pool: pool_pda,
                b_mint: mock_b_mint,
            }
        }

        pub fn create_virtual_token_account_mock(
            &mut self,
            pool: Pubkey,
            balance: u64,
            fees_paid: u64,
        ) -> Pubkey {
            // Derive the VirtualTokenAccount PDA using pool + owner seeds
            let (vta_pda, _vta_bump) = Pubkey::find_program_address(
                &[
                    cpmm_state::VIRTUAL_TOKEN_ACCOUNT_SEED,
                    pool.as_ref(),
                    self.payer.pubkey().as_ref(),
                ],
                &self.program_id,
            );

            // Create VTA PDA account with VirtualTokenAccount structure
            let vta_data = cpmm_state::VirtualTokenAccount {
                pool: anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
                owner: anchor_lang::prelude::Pubkey::from(self.payer.pubkey().to_bytes()),
                balance,
                fees_paid,
            };

            let mut vta_account_data = Vec::new();
            vta_data.try_serialize(&mut vta_account_data).unwrap();

            self.svm
                .set_account(
                    vta_pda,
                    solana_sdk::account::Account {
                        lamports: 1_000_000,
                        data: vta_account_data,
                        owner: self.program_id,
                        executable: false,
                        rent_epoch: 0,
                    },
                )
                .unwrap();

            vta_pda
        }

        pub fn buy_virtual_token(
            &mut self,
            pool: Pubkey,
            virtual_token_account: Pubkey,
            a_amount: u64,
            b_mint: Pubkey,
        ) -> Result<()> {
            // Helper function to calculate instruction discriminator
            fn get_discriminator(instruction_name: &str) -> [u8; 8] {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(format!("global:{}", instruction_name));
                let result = hasher.finalize();
                let mut discriminator = [0u8; 8];
                discriminator.copy_from_slice(&result[..8]);
                discriminator
            }

            let accounts = vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(self.payer_ata, false),
                AccountMeta::new(virtual_token_account, false),
                AccountMeta::new(pool, false),
                AccountMeta::new(self.payer_ata, false), // pool_ata - using payer_ata for simplicity
                AccountMeta::new(self.a_mint, false),
                AccountMeta::new(b_mint, false),
                AccountMeta::new_readonly(
                    Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                    false,
                ),
                AccountMeta::new(solana_sdk_ids::system_program::ID, false),
            ];

            let args = BuyVirtualTokenArgs { a_amount };

            let instruction = Instruction {
                program_id: self.program_id,
                accounts: accounts,
                data: {
                    let mut data = Vec::new();
                    data.extend_from_slice(&get_discriminator("buy_virtual_token"));
                    args.serialize(&mut data).unwrap();
                    data
                },
            };

            let tx = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&self.payer.pubkey()),
                &[&self.payer],
                self.svm.latest_blockhash(),
            );

            // todo return different errors, now only returns AccountDidNotDeserialize
            self.svm.send_transaction(tx).map_err(|err| {
                println!("Transaction failed: {:?}", err);
                anchor_lang::error::Error::from(
                    anchor_lang::error::ErrorCode::AccountDidNotDeserialize,
                )
            })?;
            Ok(())
        }
    }
}

#[cfg(test)]
pub use test_runner::TestRunner;
