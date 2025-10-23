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
        pub program_id: Pubkey,
        pub b_mint_index: u64,
    }

    pub struct TestPool {
        pub pool: Pubkey,
        pub b_mint_index: u64,
    }

    impl TestRunner {
        pub fn new() -> Self {
            let mut svm = LiteSVM::new();

            // Deploy your program to the test environment
            let program_id = Pubkey::from(crate::ID.to_bytes());
            let program_bytes = include_bytes!("../../../../target/deploy/cpmm_poc.so");
            svm.add_program(program_id, program_bytes).unwrap();

            Self {
                svm,
                program_id,
                b_mint_index: 0,
            }
        }

        pub fn create_mint(&mut self, payer: &Keypair, a_mint_decimals: u8) -> Pubkey {
            let a_mint = CreateMint::new(&mut self.svm, &payer)
                .authority(&payer.pubkey())
                .decimals(a_mint_decimals)
                .send()
                .unwrap();
            return a_mint;
        }

        pub fn mint_to(&mut self, payer: &Keypair, mint: &Pubkey, payer_ata: Pubkey, amount: u64) {
            MintTo::new(&mut self.svm, &payer, &mint, &payer_ata, amount)
                .owner(&payer)
                .send()
                .unwrap();
        }

        pub fn create_associated_token_account(
            &mut self,
            payer: &Keypair,
            mint: Pubkey,
            owner: &Pubkey,
        ) -> Pubkey {
            let ata = CreateAssociatedTokenAccount::new(&mut self.svm, &payer, &mint)
                .owner(owner)
                .send()
                .unwrap();
            return ata;
        }

        pub fn create_central_state_mock(
            &mut self,
            payer: &Keypair,
            daily_burn_allowance: u64,
            creator_daily_burn_allowance: u64,
            user_burn_bp: u16,
            creator_burn_bp: u16,
            burn_reset_time: u64,
        ) -> Pubkey {
            let (central_state_pda, central_state_bump) =
                Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);
            let central_state = cpmm_state::CentralState::new(
                central_state_bump,
                anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes()),
                daily_burn_allowance,
                creator_daily_burn_allowance,
                user_burn_bp,
                creator_burn_bp,
                burn_reset_time,
            );
            let mut central_state_data = Vec::new();
            central_state
                .try_serialize(&mut central_state_data)
                .unwrap();
            self.svm
                .set_account(
                    central_state_pda,
                    solana_sdk::account::Account {
                        lamports: 1_000_000,
                        data: central_state_data,
                        owner: self.program_id,
                        executable: false,
                        rent_epoch: 0,
                    },
                )
                .unwrap();
            central_state_pda
        }

        pub fn airdrop(&mut self, receiver: &Pubkey, amount: u64) {
            self.svm.airdrop(receiver, amount).unwrap();
        }

        pub fn create_pool_mock(
            &mut self,
            payer: &Keypair,
            a_mint: Pubkey,
            a_reserve: u64,
            a_virtual_reserve: u64,
            b_reserve: u64,
            b_mint_decimals: u8,
            creator_fee_basis_points: u16,
            buyback_fee_basis_points: u16,
            creator_fees_balance: u64,
            buyback_fees_balance: u64,
        ) -> TestPool {
            let (central_state_pda, _central_state_bump) =
                Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

            let central_state = self.svm.get_account(&central_state_pda).unwrap();

            let mut central_state_data =
                cpmm_state::CentralState::try_deserialize(&mut central_state.data.as_slice())
                    .unwrap();

            let b_mint_index = central_state_data.b_mint_index;
            central_state_data.b_mint_index += 1;
            let mut central_state_data_vec = Vec::new();
            central_state_data
                .try_serialize(&mut central_state_data_vec)
                .unwrap();
            self.svm
                .set_account(
                    central_state_pda,
                    solana_sdk::account::Account {
                        lamports: 1_000_000,
                        data: central_state_data_vec,
                        owner: self.program_id,
                        executable: false,
                        rent_epoch: 0,
                    },
                )
                .unwrap();

            // Setup PDAs consistent with on-chain seeds
            let (pool_pda, pool_bump) = Pubkey::find_program_address(
                &[
                    cpmm_state::BCPMM_POOL_SEED,
                    b_mint_index.to_le_bytes().as_ref(),
                ],
                &self.program_id,
            );

            // Create pool PDA account with BcpmmPool structure
            let pool_data = cpmm_state::BcpmmPool {
                bump: pool_bump,
                creator: anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes()),
                a_mint: anchor_lang::prelude::Pubkey::from(a_mint.to_bytes()),
                a_reserve,
                a_virtual_reserve,
                a_remaining_topup: 0,
                b_mint_index,
                b_mint_decimals,
                b_reserve,
                creator_fees_balance,
                buyback_fees_balance,
                creator_fee_basis_points,
                buyback_fee_basis_points,
                burns_today: 0,
                last_burn_timestamp: 0,
            };

            let mut pool_account_data = Vec::new();
            pool_data.try_serialize(&mut pool_account_data).unwrap();

            let pool_ata_pubkey = CreateAssociatedTokenAccount::new(&mut self.svm, &payer, &a_mint)
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
                &payer,
                &a_mint,
                &pool_ata_pubkey,
                needed_balance,
            )
            .owner(&payer)
            .send()
            .unwrap();

            TestPool {
                pool: pool_pda,
                b_mint_index,
            }
        }

        pub fn create_virtual_token_account_mock(
            &mut self,
            owner: Pubkey,
            pool: Pubkey,
            balance: u64,
            fees_paid: u64,
        ) -> Pubkey {
            // Derive the VirtualTokenAccount PDA using pool + owner seeds
            let (vta_pda, vta_bump) = Pubkey::find_program_address(
                &[
                    cpmm_state::VIRTUAL_TOKEN_ACCOUNT_SEED,
                    pool.as_ref(),
                    owner.as_ref(),
                ],
                &self.program_id,
            );

            // Create VTA PDA account with VirtualTokenAccount structure
            let vta_data = cpmm_state::VirtualTokenAccount {
                bump: vta_bump,
                pool: anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
                owner: anchor_lang::prelude::Pubkey::from(owner.to_bytes()),
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
            payer: &Keypair,
            payer_ata: Pubkey,
            mint: Pubkey,
            pool: Pubkey,
            virtual_token_account: Pubkey,
            a_amount: u64,
            b_amount_min: u64,
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

            let pool_ata = anchor_spl::associated_token::get_associated_token_address(
                &anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
                &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
            );

            let accounts = vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(payer_ata, false),
                AccountMeta::new(virtual_token_account, false),
                AccountMeta::new(pool, false),
                AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false),
                AccountMeta::new(mint, false),
                AccountMeta::new_readonly(
                    Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                    false,
                ),
                AccountMeta::new(solana_sdk_ids::system_program::ID, false),
            ];

            let args = BuyVirtualTokenArgs {
                a_amount,
                b_amount_min,
            };

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
                Some(&payer.pubkey()),
                &[&payer],
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

        pub fn sell_virtual_token(
            &mut self,
            payer: &Keypair,
            payer_ata: Pubkey,
            mint: Pubkey,
            pool: Pubkey,
            virtual_token_account: Pubkey,
            b_amount: u64,
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

            // Derive the pool ATA using the same logic as create_pool_mock
            let pool_ata = anchor_spl::associated_token::get_associated_token_address(
                &anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
                &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
            );
            let pool_ata_account_meta = AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false); // Use the derived pool ATA

            let accounts = vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(payer_ata, false),
                AccountMeta::new(virtual_token_account, false),
                AccountMeta::new(pool, false),
                pool_ata_account_meta,
                AccountMeta::new(mint, false),
                AccountMeta::new(solana_sdk_ids::system_program::ID, false),
                AccountMeta::new_readonly(
                    Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                    false,
                ),
            ];

            let args = crate::instructions::SellVirtualTokenArgs { b_amount };

            let instruction = Instruction {
                program_id: self.program_id,
                accounts: accounts,
                data: {
                    let mut data = Vec::new();
                    data.extend_from_slice(&get_discriminator("sell_virtual_token"));
                    args.serialize(&mut data).unwrap();
                    data
                },
            };

            let tx = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[&payer],
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

        pub fn initialize_user_burn_allowance(
            &mut self,
            payer: &Keypair,
            owner: Pubkey,
        ) -> Result<Pubkey> {
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

            // Derive the CentralState PDA
            let (central_state_pda, _central_bump) = Pubkey::find_program_address(
                &[cpmm_state::CENTRAL_STATE_SEED],
                &self.program_id,
            );

            // Derive the UserBurnAllowance PDA
            let (user_burn_allowance_pda, _bump) = Pubkey::find_program_address(
                &[cpmm_state::USER_BURN_ALLOWANCE_SEED, owner.as_ref()],
                &self.program_id,
            );

            let accounts = vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(owner, false),
                AccountMeta::new_readonly(central_state_pda, false),
                AccountMeta::new(user_burn_allowance_pda, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
            ];

            let instruction = Instruction {
                program_id: self.program_id,
                accounts: accounts,
                data: {
                    let mut data = Vec::new();
                    data.extend_from_slice(&get_discriminator("initialize_user_burn_allowance"));
                    data
                },
            };

            let tx = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[&payer],
                self.svm.latest_blockhash(),
            );

            self.svm.send_transaction(tx).map_err(|err| {
                println!("Initialize user burn allowance failed: {:?}", err);
                anchor_lang::error::Error::from(
                    anchor_lang::error::ErrorCode::AccountDidNotDeserialize,
                )
            })?;

            Ok(user_burn_allowance_pda)
        }

        pub fn burn_virtual_token(
            &mut self,
            payer: &Keypair,
            pool: Pubkey,
            user_burn_allowance: Pubkey,
            b_amount_basis_points: u16,
        ) -> Result<()> {

            // Derive the CentralState PDA
            let (central_state_pda, _central_bump) = Pubkey::find_program_address(
                &[cpmm_state::CENTRAL_STATE_SEED],
                &self.program_id,
            );

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
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(pool, false),
                AccountMeta::new(user_burn_allowance, false),
                AccountMeta::new(central_state_pda, false),
            ];

            let args = crate::instructions::BurnVirtualTokenArgs {
                b_amount_basis_points,
            };

            let instruction = Instruction {
                program_id: self.program_id,
                accounts: accounts,
                data: {
                    let mut data = Vec::new();
                    data.extend_from_slice(&get_discriminator("burn_virtual_token"));
                    args.serialize(&mut data).unwrap();
                    data
                },
            };

            let tx = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[&payer],
                self.svm.latest_blockhash(),
            );

            self.svm.send_transaction(tx).map_err(|err| {
                println!("Burn virtual token failed: {:?}", err);
                anchor_lang::error::Error::from(
                    anchor_lang::error::ErrorCode::AccountDidNotDeserialize,
                )
            })?;

            Ok(())
        }

        pub fn get_user_burn_allowance(&self, user_burn_allowance: &Pubkey) -> Result<cpmm_state::UserBurnAllowance> {
            let account = self.svm.get_account(user_burn_allowance)
                .ok_or_else(|| anchor_lang::error::Error::from(
                    anchor_lang::error::ErrorCode::AccountDidNotDeserialize,
                ))?;

            // Skip the first 8 bytes (discriminator) and deserialize the UserBurnAllowance
            let mut data = &account.data[8..];
            cpmm_state::UserBurnAllowance::try_deserialize(&mut data)
                .map_err(|_| anchor_lang::error::Error::from(
                    anchor_lang::error::ErrorCode::AccountDidNotDeserialize,
                ))
        }
    }
}

#[cfg(test)]
pub use test_runner::TestRunner;
