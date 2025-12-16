use super::compute_metrics::send_and_record;
use crate::helpers::BurnRateLimiter;
use crate::instructions::BuyVirtualTokenArgs;
use crate::state::{self as cpmm_state, CBMM_POOL_INDEX_SEED};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use litesvm::LiteSVM;
use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
use solana_sdk::clock::Clock;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

const POOL_INDEX: u32 = CBMM_POOL_INDEX_SEED;

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

    pub fn create_mint(&mut self, payer: &Keypair, quote_mint_decimals: u8) -> Pubkey {
        let quote_mint = CreateMint::new(&mut self.svm, &payer)
            .authority(&payer.pubkey())
            .decimals(quote_mint_decimals)
            .send()
            .unwrap();
        return quote_mint;
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

    pub fn put_account_on_chain<T>(&mut self, account_address: &Pubkey, account_data: T) -> Pubkey
    where
        T: anchor_lang::AnchorSerialize + anchor_lang::Discriminator,
    {
        let mut serialized_data = Vec::new();
        // Add the 8-byte discriminator first (required by Anchor)
        serialized_data.extend_from_slice(&T::DISCRIMINATOR);
        // Then serialize the account data using AnchorSerialize
        anchor_lang::AnchorSerialize::serialize(&account_data, &mut serialized_data).unwrap();

        // Calculate rent-exempt lamports based on account size
        let rent = self.svm.get_sysvar::<solana_sdk::rent::Rent>();
        let lamports = rent.minimum_balance(serialized_data.len());

        self.svm
            .set_account(
                *account_address,
                solana_sdk::account::Account {
                    lamports,
                    data: serialized_data,
                    owner: self.program_id,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
        *account_address
    }

    pub fn create_platform_config_mock(
        &mut self,
        creator: &Keypair,
        quote_mint: Pubkey,
        daily_burn_allowance: u16,
        creator_daily_burn_allowance: u16,
        user_burn_bp_x100: u32,
        creator_burn_bp_x100: u32,
        creator_fee_bp: u16,
        buyback_fee_bp: u16,
        platform_fee_bp: u16,
        burn_authority: Option<anchor_lang::prelude::Pubkey>,
    ) -> Pubkey {
        let (platform_config_pda, platform_config_bump) = Pubkey::find_program_address(
            &[cpmm_state::PLATFORM_CONFIG_SEED, creator.pubkey().as_ref()],
            &self.program_id,
        );
        let burn_tiers = vec![
            cpmm_state::BurnTier {
                burn_bp_x100: user_burn_bp_x100,
                role: cpmm_state::BurnRole::Anyone,
                max_daily_burns: daily_burn_allowance,
            },
            cpmm_state::BurnTier {
                burn_bp_x100: creator_burn_bp_x100,
                role: cpmm_state::BurnRole::PoolOwner,
                max_daily_burns: creator_daily_burn_allowance,
            },
        ];

        use crate::helpers::BurnRateConfig;

        // Burn rate config values that pass validation and allow normal operations:
        // For 10% total fees (1000 bp), total_fees_bp_x100 = 100,000
        // Initial pool stress is 75,000 (3/4 of total fees)
        // burn_limit must be > initial stress and < total_fees_bp_x100
        let burn_config = BurnRateConfig::new(
            90_000, // burn_limit_bp_x100 (90% of total fees)
            10,     // burn_min_bp_x100
            50,     // burn_decay_rate_per_sec_bp_x100 (max_decay = 90000/900 = 100)
        );

        // Directly create the struct to avoid Clock::get() call in try_new
        let platform_config = cpmm_state::PlatformConfig {
            bump: platform_config_bump,
            admin: anchor_lang::prelude::Pubkey::new_from_array(creator.pubkey().to_bytes()),
            creator: anchor_lang::prelude::Pubkey::new_from_array(creator.pubkey().to_bytes()),
            quote_mint: anchor_lang::prelude::Pubkey::new_from_array(quote_mint.to_bytes()),
            burn_authority,
            pool_creator_fee_bp: creator_fee_bp,
            pool_topup_fee_bp: buyback_fee_bp,
            platform_fee_bp,
            burn_rate_config: burn_config,
            burn_tiers_updated_at: 0,
            burn_tiers,
        };

        self.put_account_on_chain(&platform_config_pda, platform_config)
    }

    pub fn create_user_burn_allowance_mock(
        &mut self,
        user: Pubkey,
        payer: Pubkey,
        platform_config: Pubkey,
        burns_today: u16,
        last_burn_timestamp: i64,
        is_pool_owner: bool,
        created_at: i64,
    ) -> Pubkey {
        // Get platform config to read burn_tiers_updated_at
        let platform_config_account = self.svm.get_account(&platform_config).unwrap();
        let platform_config_data = cpmm_state::PlatformConfig::try_deserialize(
            &mut platform_config_account.data.as_slice(),
        )
        .unwrap();

        let burn_tier_index = if is_pool_owner { 1u8 } else { 0u8 };

        let (user_burn_allowance_pda, bump) = Pubkey::find_program_address(
            &[
                cpmm_state::USER_BURN_ALLOWANCE_SEED,
                user.as_ref(),
                platform_config.as_ref(),
                &[burn_tier_index],
                platform_config_data
                    .burn_tiers_updated_at
                    .to_le_bytes()
                    .as_ref(),
            ],
            &self.program_id,
        );
        let user_burn_allowance = cpmm_state::UserBurnAllowance {
            bump,
            platform_config: anchor_lang::prelude::Pubkey::new_from_array(
                platform_config.to_bytes(),
            ),
            user: anchor_lang::prelude::Pubkey::new_from_array(user.to_bytes()),
            payer: anchor_lang::prelude::Pubkey::new_from_array(payer.to_bytes()),
            burns_today,
            last_burn_timestamp,
            created_at,
            burn_tier_index,
            burn_tier_update_timestamp: platform_config_data.burn_tiers_updated_at,
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
        platform_config_pda: Pubkey,
        quote_mint: Pubkey,
        quote_reserve: u64,
        quote_virtual_reserve: u64,
        base_reserve: u64,
        base_total_supply: u64,
        base_mint_decimals: u8,
        creator_fee_bp: u16,
        buyback_fee_bp: u16,
        platform_fee_bp: u16,
        creator_fees_balance: u64,
        buyback_fees_balance: u64,
        _quote_outstanding_topup: u64,
    ) -> TestPool {
        // Setup PDAs consistent with on-chain seeds
        let (pool_pda, pool_bump) = Pubkey::find_program_address(
            &[
                cpmm_state::CBMM_POOL_SEED,
                POOL_INDEX.to_le_bytes().as_ref(),
                payer.pubkey().as_ref(),
                platform_config_pda.as_ref(),
            ],
            &self.program_id,
        );

        let total_fees_bp_x100 = (creator_fee_bp + buyback_fee_bp + platform_fee_bp) as u64 * 100;

        // Get current clock time for burn_limiter initialization
        let current_timestamp = self
            .svm
            .get_sysvar::<solana_sdk::clock::Clock>()
            .unix_timestamp;

        // Create pool PDA account with CbmmPool structure
        let pool_data = cpmm_state::CbmmPool {
            bump: pool_bump,
            creator: anchor_lang::prelude::Pubkey::new_from_array(payer.pubkey().to_bytes()),
            pool_index: POOL_INDEX,
            platform_config: anchor_lang::prelude::Pubkey::new_from_array(
                platform_config_pda.to_bytes(),
            ),
            quote_mint: anchor_lang::prelude::Pubkey::new_from_array(quote_mint.to_bytes()),
            quote_reserve: quote_reserve,
            quote_virtual_reserve: quote_virtual_reserve,
            // quote_outstanding_topup removed from state
            base_mint_decimals: base_mint_decimals,
            base_reserve: base_reserve,
            base_total_supply,
            creator_fees_balance,
            buyback_fees_balance,
            creator_fee_bp,
            buyback_fee_bp,
            platform_fee_bp,
            burn_limiter: BurnRateLimiter::new(current_timestamp, total_fees_bp_x100 * 3 / 4),
            quote_optimal_virtual_reserve: quote_virtual_reserve, // defaulting
            quote_starting_virtual_reserve: quote_virtual_reserve, // defaulting
            base_starting_total_supply: base_reserve,             // defaulting
            platform_fees_balance: 0,
        };

        self.put_account_on_chain(&pool_pda, pool_data);

        TestPool { pool: pool_pda }
    }

    pub fn create_virtual_token_account_mock(
        &mut self,
        owner: Pubkey,
        pool: Pubkey,
        balance: u64,
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
                pool: anchor_lang::prelude::Pubkey::new_from_array(pool.to_bytes()),
                owner: anchor_lang::prelude::Pubkey::new_from_array(owner.to_bytes()),
                balance,
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
        quote_amount: u64,
        base_amount_min: u64,
    ) -> std::result::Result<(), TransactionError> {
        let pool_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );

        // Get platform_config from pool account
        let pool_account = self.svm.get_account(&pool).unwrap();
        let pool_data =
            cpmm_state::CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config_pda = pool_data.platform_config;

        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(payer_ata, false),
            AccountMeta::new(virtual_token_account, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false),
            AccountMeta::new(Pubkey::from(platform_config_pda.to_bytes()), false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new(solana_sdk_ids::system_program::ID, false),
        ];

        let args = BuyVirtualTokenArgs {
            quote_amount,
            base_amount_min,
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
        base_amount: u64,
        min_quote_amount: u64,
    ) -> std::result::Result<(), TransactionError> {
        let pool_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );

        // Get platform_config from pool account
        let pool_account = self.svm.get_account(&pool).unwrap();
        let pool_data =
            cpmm_state::CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config_pda = pool_data.platform_config;

        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(payer_ata, false),
            AccountMeta::new(virtual_token_account, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false),
            AccountMeta::new(Pubkey::from(platform_config_pda.to_bytes()), false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new(solana_sdk_ids::system_program::ID, false),
        ];

        let args = crate::instructions::SellVirtualTokenArgs {
            base_amount,
            min_quote_amount,
        };

        self.send_instruction("sell_virtual_token", accounts, args, &[payer])
    }

    pub fn initialize_user_burn_allowance(
        &mut self,
        payer: &Keypair,
        owner: Pubkey,
        platform_config: Pubkey,
        is_pool_owner: bool,
    ) -> std::result::Result<Pubkey, TransactionError> {
        use crate::instructions::InitializeUserBurnAllowanceArgs;

        // Get platform config to read burn_tiers_updated_at
        let platform_config_account = self.svm.get_account(&platform_config).unwrap();
        let platform_config_data = cpmm_state::PlatformConfig::try_deserialize(
            &mut platform_config_account.data.as_slice(),
        )
        .unwrap();

        let burn_tier_index = if is_pool_owner { 1u8 } else { 0u8 };

        // Derive the UserBurnAllowance PDA with correct seeds
        let (user_burn_allowance_pda, _bump) = Pubkey::find_program_address(
            &[
                cpmm_state::USER_BURN_ALLOWANCE_SEED,
                owner.as_ref(),
                platform_config.as_ref(),
                &[burn_tier_index],
                platform_config_data
                    .burn_tiers_updated_at
                    .to_le_bytes()
                    .as_ref(),
            ],
            &self.program_id,
        );

        // Find the pool if needed
        let pool_pda = if is_pool_owner {
            let (pool, _) = Pubkey::find_program_address(
                &[
                    cpmm_state::CBMM_POOL_SEED,
                    cpmm_state::CBMM_POOL_INDEX_SEED.to_le_bytes().as_ref(),
                    owner.as_ref(),
                    platform_config.as_ref(),
                ],
                &self.program_id,
            );
            pool
        } else {
            self.program_id // Use program_id as dummy when pool is not needed
        };

        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new(user_burn_allowance_pda, false),
            AccountMeta::new_readonly(platform_config, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
            AccountMeta::new_readonly(pool_pda, false),
        ];

        let args = InitializeUserBurnAllowanceArgs { burn_tier_index };

        self.send_instruction("initialize_user_burn_allowance", accounts, args, &[payer])?;

        Ok(user_burn_allowance_pda)
    }

    pub fn burn_virtual_token(
        &mut self,
        payer: &Keypair,
        pool: Pubkey,
        user_burn_allowance: Pubkey,
        burn_authority: Option<&Keypair>,
    ) -> std::result::Result<(), TransactionError> {
        // Get platform_config from pool account
        let pool_account = self.svm.get_account(&pool).unwrap();
        let pool_data =
            cpmm_state::CbmmPool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
        let platform_config_pda = pool_data.platform_config;

        let mut accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pool, false),
            AccountMeta::new(user_burn_allowance, false),
            AccountMeta::new(Pubkey::from(platform_config_pda.to_bytes()), false),
        ];

        let mut signers: Vec<&Keypair> = vec![payer];

        // Always include the burn_authority account (Anchor's Option<Signer> still requires the account to be present)
        if let Some(auth) = burn_authority {
            accounts.push(AccountMeta::new(auth.pubkey(), true));
            signers.push(auth);
        } else {
            accounts.push(AccountMeta::new_readonly(self.program_id, false));
        }

        self.send_instruction("burn_virtual_token", accounts, (), &signers)
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

        MintTo::new(&mut self.svm, &authority, &mint, &recipient_ata_sdk, amount)
            .owner(authority)
            .send()
            .unwrap();
    }

    pub fn claim_creator_fees(
        &mut self,
        owner: &Keypair,
        owner_ata: Pubkey,
        mint: Pubkey,
        pool: Pubkey,
    ) -> std::result::Result<(), TransactionError> {
        let pool_ata = anchor_spl::associated_token::get_associated_token_address(
            &anchor_lang::prelude::Pubkey::from(pool.to_bytes()),
            &anchor_lang::prelude::Pubkey::from(mint.to_bytes()),
        );

        let accounts = vec![
            AccountMeta::new(owner.pubkey(), true),
            AccountMeta::new(owner_ata, false),
            AccountMeta::new(pool, false),
            AccountMeta::new(Pubkey::from(pool_ata.to_bytes()), false),
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(
                Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
                false,
            ),
            AccountMeta::new(solana_sdk_ids::system_program::ID, false),
        ];

        self.send_instruction("claim_creator_fees", accounts, (), &[owner])
    }

    // pub fn claim_platform_fees(
    //     &mut self,
    //     admin: &Keypair,
    //     creator: Pubkey,
    //     admin_ata: Pubkey,
    //     mint: Pubkey,
    // ) -> std::result::Result<(), TransactionError> {
    //     let platform_config_pda = Pubkey::find_program_address(
    //         &[cpmm_state::PLATFORM_CONFIG_SEED, creator.as_ref()],
    //         &self.program_id,
    //     )
    //     .0;

    //     let accounts = vec![
    //         AccountMeta::new(admin.pubkey(), true),
    //         AccountMeta::new_readonly(creator, false),
    //         AccountMeta::new(admin_ata, false),
    //         AccountMeta::new(platform_config_pda, false),
    //         AccountMeta::new(mint, false),
    //         AccountMeta::new_readonly(
    //             Pubkey::from(anchor_spl::token::spl_token::ID.to_bytes()),
    //             false,
    //         ),
    //         AccountMeta::new_readonly(
    //             Pubkey::from(anchor_spl::associated_token::ID.to_bytes()),
    //             false,
    //         ),
    //         AccountMeta::new(solana_sdk_ids::system_program::ID, false),
    //     ];

    //     self.send_instruction("claim_platform_fees", accounts, (), &[admin])
    // }
}
