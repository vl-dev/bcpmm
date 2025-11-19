use super::compute_metrics::send_and_record;
use crate::instructions::BuyVirtualTokenArgs;
use crate::state::{self as cpmm_state, BCPMM_POOL_INDEX_SEED};
use anchor_lang::prelude::*;
use litesvm::LiteSVM;
use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
use solana_sdk::clock::Clock;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

const POOL_INDEX: u32 = BCPMM_POOL_INDEX_SEED;

#[derive(Debug)]
pub struct TransactionError {
    pub message: String,
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transaction failed: {}", self.message)
    }
}

impl std::error::Error for TransactionError {}

impl From<TransactionError> for anchor_lang::error::Error {
    fn from(_err: TransactionError) -> Self {
        anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountDidNotDeserialize)
    }
}

pub struct TestRunner {
    pub svm: LiteSVM,
    pub program_id: Pubkey,
}

pub struct TestPool {
    pub pool: Pubkey,
}

impl TestRunner {
    pub fn new() -> Self {
        let mut svm = LiteSVM::new();

        // Deploy your program to the test environment
        let program_id = Pubkey::from(crate::ID.to_bytes());
        let program_bytes = include_bytes!("../../../../target/deploy/cbmm.so");
        svm.add_program(program_id, program_bytes).unwrap();

        Self { svm, program_id }
    }

    pub fn create_mint(&mut self, payer: &Keypair, a_mint_decimals: u8) -> Pubkey {
        let a_mint = CreateMint::new(&mut self.svm, &payer)
            .authority(&payer.pubkey())
            .decimals(a_mint_decimals)
            .send()
            .unwrap();
        return a_mint;
    }

    /// Create a mock mint (just a Pubkey) for unit testing
    /// Use this in unit tests instead of create_mint to avoid calling Token Program
    pub fn create_mint_mock(&self) -> Pubkey {
        // Just generate a random pubkey - no need to actually create the mint
        // for unit tests since we're mocking everything
        Keypair::new().pubkey()
    }

    pub fn mint_to(&mut self, payer: &Keypair, mint: &Pubkey, payer_ata: Pubkey, amount: u64) {
        // First check if ATA exists, create it if not
        if self.svm.get_account(&payer_ata).is_none() {
            CreateAssociatedTokenAccount::new(&mut self.svm, &payer, &mint)
                .owner(&payer.pubkey())
                .send()
                .unwrap();
        }
        
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

    pub fn put_account_on_chain<T>(&mut self, account_address: &Pubkey, account_data: T) -> Pubkey
    where
        T: anchor_lang::AccountSerialize,
    {
        let mut serialized_data = Vec::new();
        account_data.try_serialize(&mut serialized_data).unwrap();
        self.svm
            .set_account(
                *account_address,
                solana_sdk::account::Account {
                    lamports: 1_000_000,
                    data: serialized_data,
                    owner: self.program_id,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
        *account_address
    }

    pub fn create_central_state_mock(
        &mut self,
        payer: &Keypair,
        daily_burn_allowance: u16,
        creator_daily_burn_allowance: u16,
        user_burn_bp_x100: u32,
        creator_burn_bp_x100: u32,
        burn_reset_time_of_day_seconds: u32,
        creator_fee_basis_points: u16,
        buyback_fee_basis_points: u16,
        platform_fee_basis_points: u16,
    ) -> Pubkey {
        let (central_state_pda, central_state_bump) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);
        let central_state = cpmm_state::CentralState::new(
            central_state_bump,
            anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes()),
            daily_burn_allowance,
            creator_daily_burn_allowance,
            user_burn_bp_x100,
            creator_burn_bp_x100,
            burn_reset_time_of_day_seconds,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
        );
        self.put_account_on_chain(&central_state_pda, central_state)
    }

    pub fn create_user_burn_allowance_mock(
        &mut self,
        user: Pubkey,
        payer: Pubkey,
        burns_today: u16,
        last_burn_timestamp: i64,
        is_pool_owner: bool,
    ) -> Pubkey {
        let (user_burn_allowance_pda, bump) = Pubkey::find_program_address(
            &[
                cpmm_state::USER_BURN_ALLOWANCE_SEED,
                user.as_ref(),
                &[is_pool_owner as u8],
            ],
            &self.program_id,
        );
        let user_burn_allowance = cpmm_state::UserBurnAllowance {
            bump,
            user: anchor_lang::prelude::Pubkey::from(user.to_bytes()),
            payer: anchor_lang::prelude::Pubkey::from(payer.to_bytes()),
            burns_today,
            last_burn_timestamp,
        };
        self.put_account_on_chain(
            &Pubkey::from(user_burn_allowance_pda.to_bytes()),
            user_burn_allowance,
        )
    }

    pub fn airdrop(&mut self, receiver: &Pubkey, amount: u64) {
        self.svm.airdrop(receiver, amount).unwrap();
    }

    pub fn send_instruction<T>(
        &mut self,
        instruction_name: &str,
        accounts: Vec<AccountMeta>,
        args: T,
        signers: &[&Keypair],
    ) -> std::result::Result<(), TransactionError>
    where
        T: anchor_lang::AnchorSerialize,
    {
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

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: accounts,
            data: {
                let mut data = Vec::new();
                data.extend_from_slice(&get_discriminator(instruction_name));
                args.serialize(&mut data).unwrap();
                data
            },
        };

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&signers[0].pubkey()),
            signers,
            self.svm.latest_blockhash(),
        );

        send_and_record(&mut self.svm, tx, instruction_name).map_err(|err| TransactionError {
            message: format!("{:?}", err),
        })?;
        Ok(())
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
        platform_fee_basis_points: u16,
        creator_fees_balance: u64,
        buyback_fees_balance: u64,
        a_outstanding_topup: u64,
    ) -> TestPool {
        let (central_state_pda, _central_state_bump) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

        let central_state = self.svm.get_account(&central_state_pda).unwrap();

        let central_state_data =
            cpmm_state::CentralState::try_deserialize(&mut central_state.data.as_slice()).unwrap();

        self.put_account_on_chain(&central_state_pda, central_state_data);

        // Setup PDAs consistent with on-chain seeds
        let (pool_pda, pool_bump) = Pubkey::find_program_address(
            &[
                cpmm_state::BCPMM_POOL_SEED,
                POOL_INDEX.to_le_bytes().as_ref(),
                payer.pubkey().as_ref(),
            ],
            &self.program_id,
        );

        // Create pool PDA account with BcpmmPool structure
        let pool_data = cpmm_state::BcpmmPool {
            bump: pool_bump,
            creator: anchor_lang::prelude::Pubkey::from(payer.pubkey().to_bytes()),
            pool_index: POOL_INDEX,
            a_mint: anchor_lang::prelude::Pubkey::from(a_mint.to_bytes()),
            a_reserve,
            a_virtual_reserve,
            a_outstanding_topup,
            b_mint_decimals,
            b_reserve,
            creator_fees_balance,
            buyback_fees_balance,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
            burns_today: 0,
            last_burn_timestamp: 0,
        };

        self.put_account_on_chain(&pool_pda, pool_data);

        TestPool { pool: pool_pda }
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
        self.put_account_on_chain(
            &vta_pda,
            cpmm_state::VirtualTokenAccount {
                bump: vta_bump,
                pool: anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
                owner: anchor_lang::prelude::Pubkey::from(owner.to_bytes()),
                balance,
                fees_paid,
            },
        );

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
    ) -> std::result::Result<(), TransactionError> {
        let pool_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );

        // Derive the CentralState PDA
        let (central_state_pda, _central_bump) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

        let central_state_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(central_state_pda.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );

        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(payer_ata, false),
            AccountMeta::new(virtual_token_account, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false),
            AccountMeta::new(Pubkey::from(central_state_ata.to_bytes()), false),
            AccountMeta::new(central_state_pda, false),
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

        self.send_instruction("buy_virtual_token", accounts, args, &[payer])
    }

    pub fn sell_virtual_token(
        &mut self,
        payer: &Keypair,
        payer_ata: Pubkey,
        mint: Pubkey,
        pool: Pubkey,
        virtual_token_account: Pubkey,
        b_amount: u64,
    ) -> std::result::Result<(), TransactionError> {
        let pool_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );
        let central_state_pda =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id).0;

        let central_state_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(central_state_pda.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );

        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(payer_ata, false),
            AccountMeta::new(virtual_token_account, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false),
            AccountMeta::new(Pubkey::from(central_state_ata.to_bytes()), false),
            AccountMeta::new(central_state_pda, false),
            AccountMeta::new(mint, false),
            AccountMeta::new(solana_sdk_ids::system_program::ID, false),
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
        ];

        let args = crate::instructions::SellVirtualTokenArgs { b_amount };

        self.send_instruction("sell_virtual_token", accounts, args, &[payer])
    }

    /// Initialize a VirtualTokenAccount by calling the actual instruction
    pub fn initialize_virtual_token_account(
        &mut self,
        payer: &Keypair,
        owner: Pubkey,
        pool: Pubkey,
    ) -> std::result::Result<Pubkey, TransactionError> {
        // Derive the VirtualTokenAccount PDA
        let (vta_pda, _) = Pubkey::find_program_address(
            &[
                cpmm_state::VIRTUAL_TOKEN_ACCOUNT_SEED,
                pool.as_ref(),
                payer.pubkey().as_ref(),
            ],
            &self.program_id,
        );

        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),           // payer (signer)
            AccountMeta::new_readonly(owner, false),          // owner (can be any account)
            AccountMeta::new(vta_pda, false),                 // virtual_token_account (will be created)
            AccountMeta::new_readonly(pool, false),           // pool
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false), // system_program
        ];

        self.send_instruction("initialize_virtual_token_account", accounts, (), &[payer])?;

        Ok(vta_pda)
    }

    /// Close a VirtualTokenAccount by calling the actual instruction
    pub fn close_virtual_token_account(
        &mut self,
        owner: &Keypair,
        virtual_token_account: Pubkey,
    ) -> std::result::Result<(), TransactionError> {
        let accounts = vec![
            AccountMeta::new(owner.pubkey(), true),           // owner (signer)
            AccountMeta::new(virtual_token_account, false),   // virtual_token_account (will be closed)
        ];

        self.send_instruction("close_virtual_token_account", accounts, (), &[owner])
    }

    pub fn initialize_user_burn_allowance(
        &mut self,
        payer: &Keypair,
        owner: Pubkey,
        is_pool_owner: bool,
    ) -> std::result::Result<Pubkey, TransactionError> {
        // Derive the CentralState PDA
        let (central_state_pda, _central_bump) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

        // Derive the UserBurnAllowance PDA
        let (user_burn_allowance_pda, _bump) = Pubkey::find_program_address(
            &[
                cpmm_state::USER_BURN_ALLOWANCE_SEED,
                owner.as_ref(),
                &[is_pool_owner as u8],
            ],
            &self.program_id,
        );

        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new_readonly(central_state_pda, false),
            AccountMeta::new(user_burn_allowance_pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ];

        self.send_instruction(
            "initialize_user_burn_allowance",
            accounts,
            is_pool_owner,
            &[payer],
        )?;

        Ok(user_burn_allowance_pda)
    }

    pub fn burn_virtual_token(
        &mut self,
        payer: &Keypair,
        pool: Pubkey,
        user_burn_allowance: Pubkey,
        is_pool_owner: bool,
    ) -> std::result::Result<(), TransactionError> {
        // Derive the CentralState PDA
        let (central_state_pda, _central_bump) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pool, false),
            AccountMeta::new(user_burn_allowance, false),
            AccountMeta::new(central_state_pda, false),
        ];

        self.send_instruction("burn_virtual_token", accounts, is_pool_owner, &[payer])
    }

    pub fn get_user_burn_allowance(
        &self,
        address: &Pubkey,
    ) -> Result<cpmm_state::UserBurnAllowance> {
        let account = self.svm.get_account(address).ok_or_else(|| {
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountDidNotDeserialize)
        })?;

        // Skip the first 8 bytes (discriminator) and deserialize the UserBurnAllowance
        cpmm_state::UserBurnAllowance::try_deserialize(&mut account.data.as_slice()).map_err(|_| {
            anchor_lang::error::Error::from(anchor_lang::error::ErrorCode::AccountDidNotDeserialize)
        })
    }

    pub fn set_system_clock(&mut self, timestamp: i64) {
        let mut initial_clock = self.svm.get_sysvar::<Clock>();
        initial_clock.unix_timestamp = timestamp;
        self.svm.set_sysvar::<Clock>(&initial_clock);
    }

    pub fn mint_tokens(
        &mut self,
        authority: &Keypair,
        recipient: Pubkey,
        mint: Pubkey,
        amount: u64,
    ) {
        let recipient_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(recipient.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );
        let recipient_ata_sdk = solana_sdk::pubkey::Pubkey::from(recipient_ata.to_bytes());

        // Create ATA if it doesn't exist
        if self.svm.get_account(&recipient_ata_sdk).is_none() {
            CreateAssociatedTokenAccount::new(&mut self.svm, &authority, &mint)
                .owner(&recipient)
                .send()
                .unwrap();
        }

        MintTo::new(&mut self.svm, &authority, &mint, &recipient_ata_sdk, amount)
            .owner(authority)
            .send()
            .unwrap();
    }

    // pub fn create_treasury_ata(&mut self, payer: &Keypair, mint: Pubkey, initial_balance: u64) -> Pubkey {
    //     let (treasury_pda, _treasury_bump) = Pubkey::find_program_address(
    //         &[cpmm_state::TREASURY_SEED, mint.as_ref()],
    //         &self.program_id,
    //     );

    //     let treasury_ata = self.create_associated_token_account(payer, mint, &treasury_pda);

    //     // mint appropriate amount of A tokens to pool
    //     MintTo::new(
    //         &mut self.svm,
    //         &payer,
    //         &mint,
    //         &treasury_ata,
    //         initial_balance,
    //     )
    //     .owner(&payer)
    //     .send()
    //     .unwrap();

    //     return treasury_ata;
    // }

    pub fn claim_creator_fees(
        &mut self,
        owner: &Keypair,
        owner_ata: Pubkey,
        mint: Pubkey,
        pool: Pubkey,
        amount: u64,
    ) -> std::result::Result<(), TransactionError> {
        let pool_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );

        let (central_state_pda, _central_bump) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

        let accounts = vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new(central_state_pda, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new(solana_sdk_ids::system_program::ID, false),
        ];

        let args = crate::instructions::ClaimCreatorFeesArgs { amount };

        self.send_instruction("claim_creator_fees", accounts, args, &[owner])
    }

    pub fn claim_admin_fees(
        &mut self,
        admin: &Keypair,
        admin_ata: Pubkey,
        mint: Pubkey,
    ) -> std::result::Result<(), TransactionError> {
        let central_state_pda =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id).0;
        let central_state_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(central_state_pda.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );

        let accounts = vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(admin_ata, false),
            AccountMeta::new(central_state_pda, false),
            AccountMeta::new(Pubkey::from(central_state_ata.to_bytes()), false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::associated_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new(solana_sdk_ids::system_program::ID, false),
        ];

        self.send_instruction("claim_admin_fees", accounts, (), &[admin])
    }

     pub fn create_program_data_mock(&mut self, upgrade_authority: &Pubkey) -> Pubkey {
        // Derive the ProgramData address for this program
        let (program_data_pda, _) = Pubkey::find_program_address(
            &[self.program_id.as_ref()],
            &solana_sdk_ids::bpf_loader_upgradeable::ID,
        );

        // Create mock ProgramData struct
        // Structure: [discriminator (4 bytes) | slot (8 bytes) | upgrade_authority_address (Option<Pubkey>)]
        let mut data = Vec::new();
        
        // Discriminator for ProgramData (3 in little-endian)
        data.extend_from_slice(&3u32.to_le_bytes());
        
        // Slot (can be 0 for testing)
        data.extend_from_slice(&0u64.to_le_bytes());
        
        // Option<Pubkey>: 1 byte for Some + 32 bytes for Pubkey
        data.push(1); // Some
        data.extend_from_slice(upgrade_authority.as_ref());

        // Set account on chain
        self.svm.set_account(
            program_data_pda,
            solana_sdk::account::Account {
                lamports: 1_000_000,
                data,
                owner: solana_sdk_ids::bpf_loader_upgradeable::ID,
                executable: false,
                rent_epoch: 0,
            },
        ).unwrap();

        program_data_pda
    }
    /// Initialize the CentralState PDA by calling the actual instruction
    pub fn initialize_central_state(
        &mut self,
        authority: &Keypair,
        admin: Pubkey,
        max_user_daily_burn_count: u16,
        max_creator_daily_burn_count: u16,
        user_burn_bp_x100: u32,
        creator_burn_bp_x100: u32,
        burn_reset_time_of_day_seconds: u32,
        creator_fee_basis_points: u16,
        buyback_fee_basis_points: u16,
        platform_fee_basis_points: u16,
    ) -> std::result::Result<Pubkey, TransactionError> {
        // Create a mock ProgramData account with the authority as the upgrade authority
        let program_data_pda = self.create_program_data_mock(&authority.pubkey());
        
        self.initialize_central_state_with_program_data(
            authority,
            admin,
            max_user_daily_burn_count,
            max_creator_daily_burn_count,
            user_burn_bp_x100,
            creator_burn_bp_x100,
            burn_reset_time_of_day_seconds,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
            program_data_pda,
        )
    }

    /// Initialize the CentralState PDA with a specific program_data account (for testing authorization)
    pub fn initialize_central_state_with_program_data(
        &mut self,
        authority: &Keypair,
        admin: Pubkey,
        max_user_daily_burn_count: u16,
        max_creator_daily_burn_count: u16,
        user_burn_bp_x100: u32,
        creator_burn_bp_x100: u32,
        burn_reset_time_of_day_seconds: u32,
        creator_fee_basis_points: u16,
        buyback_fee_basis_points: u16,
        platform_fee_basis_points: u16,
        program_data: Pubkey,
    ) -> std::result::Result<Pubkey, TransactionError> {
        // Derive the CentralState PDA
        let (central_state_pda, _) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

        // Build accounts vector
        let accounts = vec![
            AccountMeta::new(authority.pubkey(), true),              // authority (signer, pays rent)
            AccountMeta::new(central_state_pda, false),              // central_state (will be created)
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false), // system_program
            AccountMeta::new_readonly(program_data, false), // program_data
        ];

        // Build instruction arguments
        let args = crate::instructions::InitializeCentralStateArgs {
            admin: anchor_lang::prelude::Pubkey::from(admin.to_bytes()),
            max_user_daily_burn_count,
            max_creator_daily_burn_count,
            user_burn_bp_x100,
            creator_burn_bp_x100,
            burn_reset_time_of_day_seconds,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
        };

        // Call the instruction
        self.send_instruction("initialize_central_state", accounts, args, &[authority])?;

        Ok(central_state_pda)
    }



    
    /// Create a pool by calling the actual instruction
    pub fn create_pool(
        &mut self,
        payer: &Keypair,
        a_mint: Pubkey,
        a_virtual_reserve: u64,
    ) -> std::result::Result<Pubkey, TransactionError> {
        // Derive the pool PDA
        let (pool_pda, _) = Pubkey::find_program_address(
            &[
                cpmm_state::BCPMM_POOL_SEED,
                cpmm_state::BCPMM_POOL_INDEX_SEED.to_le_bytes().as_ref(),
                payer.pubkey().as_ref(),
            ],
            &self.program_id,
        );

        // Derive pool ATA
        let pool_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(pool_pda.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(a_mint.to_bytes()),
        );

        // Derive central state PDA
        let (central_state_pda, _) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

        // Derive central state ATA
        let central_state_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(central_state_pda.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(a_mint.to_bytes()),
        );

        // Build accounts vector
        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),                              // payer (signer)
            AccountMeta::new(a_mint, false),                                     // a_mint
            AccountMeta::new(pool_pda, false),                                   // pool (will be created)
            AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false),         // pool_ata
            AccountMeta::new(central_state_pda, false),                          // central_state
            AccountMeta::new(Pubkey::from(central_state_ata.to_bytes()), false), // central_state_ata
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ), // token_program
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::associated_token::ID.to_bytes()),
                false,
            ), // associated_token_program
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false), // system_program
        ];

        // Build instruction arguments
        let args = crate::instructions::CreatePoolArgs { a_virtual_reserve };

        // Call the instruction
        self.send_instruction("create_pool", accounts, args, &[payer])?;

        Ok(pool_pda)
    }

    pub fn close_user_burn_allowance(
        &mut self,
        payer: &Keypair,
        owner: Pubkey,
        is_pool_owner: bool,
    ) -> std::result::Result<(), TransactionError> {
        // Derive the UserBurnAllowance PDA
        let (user_burn_allowance_pda, _) = Pubkey::find_program_address(
            &[
                cpmm_state::USER_BURN_ALLOWANCE_SEED,
                owner.as_ref(),
                &[is_pool_owner as u8],
            ],
            &self.program_id,
        );

        // Get the UserBurnAllowance account to find the payer
        let uba_account = self
            .svm
            .get_account(&user_burn_allowance_pda)
            .expect("UserBurnAllowance account should exist");

        let uba = cpmm_state::UserBurnAllowance::try_deserialize(&mut uba_account.data.as_slice())
            .expect("Should deserialize UserBurnAllowance");

        // Derive the CentralState PDA
        let (central_state_pda, _) =
            Pubkey::find_program_address(&[cpmm_state::CENTRAL_STATE_SEED], &self.program_id);

        // Build accounts vector
        let accounts = vec![
            AccountMeta::new_readonly(owner, false),               // owner
            AccountMeta::new(user_burn_allowance_pda, false),      // user_burn_allowance
            AccountMeta::new(Pubkey::from(uba.payer.to_bytes()), false), // burn_allowance_open_payer
            AccountMeta::new_readonly(central_state_pda, false),   // central_state
        ];

        // Build instruction arguments
        let args = crate::instructions::CloseUserBurnAllowanceArgs {
            pool_owner: is_pool_owner,
        };

        // Call the instruction
        self.send_instruction("close_user_burn_allowance", accounts, args, &[payer])?;

        Ok(())
    }

    // ========================================
    // Whitepaper Test Helper Functions
    // ========================================

    /// Get pool state data
    pub fn get_pool_data(&self, pool: &Pubkey) -> cpmm_state::BcpmmPool {
        let pool_account = self.svm.get_account(pool)
            .expect("Pool account should exist");
        cpmm_state::BcpmmPool::try_deserialize(&mut pool_account.data.as_slice())
            .expect("Should deserialize BcpmmPool")
    }

    /// Get VTA state data
    pub fn get_vta_data(&self, vta: &Pubkey) -> cpmm_state::VirtualTokenAccount {
        let vta_account = self.svm.get_account(vta)
            .expect("VTA account should exist");
        cpmm_state::VirtualTokenAccount::try_deserialize(&mut vta_account.data.as_slice())
            .expect("Should deserialize VirtualTokenAccount")
    }

    /// Calculate expected buy output using the actual implementation formula
    /// 
    /// Whitepaper formula (Section 2.1): b = B₀ - k / (A₀ + ΔA + V)
    /// Where k = (A₀ + V) * B₀ (invariant)
    /// 
    /// Implementation formula (equivalent, more efficient): b = (B * ΔA) / (A + V + ΔA)
    /// 
    /// Mathematical equivalence:
    /// b = B₀ - (A₀ + V) * B₀ / (A₀ + ΔA + V)
    /// b = B₀ * (1 - (A₀ + V) / (A₀ + ΔA + V))
    /// b = B₀ * ((A₀ + ΔA + V) - (A₀ + V)) / (A₀ + ΔA + V)
    /// b = B₀ * ΔA / (A₀ + ΔA + V) ✓
    pub fn calculate_expected_buy_output(
        &self,
        a_reserve: u64,
        a_virtual_reserve: u64,
        b_reserve: u64,
        a_input_after_fees: u64,
    ) -> u64 {
        let numerator = b_reserve as u128 * a_input_after_fees as u128;
        let denominator = a_reserve as u128 + a_virtual_reserve as u128 + a_input_after_fees as u128;
        (numerator / denominator) as u64
    }

    /// Calculate expected virtual reserve after burn using formula: V₂ = V₁ * (B₁ - y) / B₁
    pub fn calculate_expected_virtual_reserve_after_burn(
        &self,
        v_before: u64,
        b_before: u64,
        burn_amount: u64,
    ) -> u64 {
        ((v_before as u128) * (b_before - burn_amount) as u128 / b_before as u128) as u64
    }

    /// Calculate price using formula: P = (A + V) / B
    pub fn calculate_price(&self, a_reserve: u64, a_virtual_reserve: u64, b_reserve: u64) -> f64 {
        (a_reserve as f64 + a_virtual_reserve as f64) / b_reserve as f64
    }

    /// Calculate invariant: k = (A + V) * B
    pub fn calculate_invariant(&self, a_reserve: u64, a_virtual_reserve: u64, b_reserve: u64) -> u128 {
        (a_reserve as u128 + a_virtual_reserve as u128) * b_reserve as u128
    }
}
