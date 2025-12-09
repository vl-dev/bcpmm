use crate::errors::CbmmError;
use crate::helpers::{
    calculate_burn_amount, calculate_buy_output_amount, calculate_fees,
    calculate_new_virtual_reserve_after_burn, calculate_new_virtual_reserve_after_topup,
    calculate_optimal_real_quote_reserve, calculate_optimal_virtual_quote_reserve,
    calculate_sell_output_amount,
};
use crate::helpers::{BurnRateConfig, BurnRateLimiter, RateLimitResult};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

pub const PLATFORM_CONFIG_SEED: &[u8] = b"platform_config";
pub const CBMM_POOL_SEED: &[u8] = b"cbmm_pool";
pub const CBMM_POOL_INDEX_SEED: u32 = 0; // this is introduced for extensibility - if we ever need more that one pool per creator, we can use this to differentiate them
pub const VIRTUAL_TOKEN_ACCOUNT_SEED: &[u8] = b"virtual_token_account";
pub const USER_BURN_ALLOWANCE_SEED: &[u8] = b"user_burn_allowance";

pub const DEFAULT_BASE_MINT_DECIMALS: u8 = 6;
pub const DEFAULT_BASE_MINT_RESERVE: u64 =
    1_000_000_000 * 10u64.pow(DEFAULT_BASE_MINT_DECIMALS as u32);
pub const MIN_VIRTUAL_RESERVE: u64 = 1_000_000;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace, PartialEq)]
pub enum BurnRole {
    Anyone,                 // Permissionless - anyone can burn at this tier
    PoolOwner,              // Only the pool owner (creator) can burn at this tier
    SpecificPubkey(Pubkey), // Only a specific whitelisted pubkey can burn
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace, PartialEq)]
pub struct BurnTier {
    pub burn_bp_x100: u32,    // Burn percentage in basis points * 100
    pub role: BurnRole,       // Who can use this tier
    pub max_daily_burns: u16, // Max burns per day (0 = unlimited)
}

#[account]
#[derive(Default, InitSpace)]
pub struct PlatformConfig {
    pub bump: u8,

    pub admin: Pubkey,
    pub creator: Pubkey,
    pub quote_mint: Pubkey,

    pub pool_creator_fee_bp: u16,
    pub pool_topup_fee_bp: u16,
    pub platform_fee_bp: u16,

    pub burn_rate_config: BurnRateConfig,

    pub burn_tiers_updated_at: i64, // used as a seed for the burn allowance accounts - update makes all old allowances invalid
    #[max_len(5)]
    pub burn_tiers: Vec<BurnTier>,
}

impl PlatformConfig {
    /// Maximum total fees allowed (20%)
    pub const MAX_TOTAL_FEES_BP: u16 = 2_000;
    /// Minimum topup fee required (1%)
    pub const MIN_TOPUP_FEE_BP: u16 = 100;
    /// Maximum platform fee allowed (10%)
    pub const MAX_PLATFORM_FEE_BP: u16 = 1_000;
    /// Time window for reaching theoretical burn limit (15 minutes in seconds)
    pub const BURN_LIMIT_TIME_WINDOW_SECONDS: i64 = 900;
    // 10 bp (1000 bp_x100) hard limit for unrestricted role
    pub const MAX_DAILY_BURN_BP_X100_ANYONE: u64 = 1_000;

    pub fn validate_fees_and_burn_config(
        pool_creator_fee_bp: u16,
        pool_topup_fee_bp: u16,
        platform_fee_bp: u16,
        burn_tiers: &[BurnTier],
        burn_limit_bp_x100: u64,
        burn_decay_rate_per_sec_bp_x100: u64,
    ) -> Result<()> {
        // 1. Validate fee constraints
        let total_fees = pool_creator_fee_bp
            .checked_add(pool_topup_fee_bp)
            .and_then(|sum| sum.checked_add(platform_fee_bp))
            .ok_or(CbmmError::MathOverflow)?;

        require!(
            total_fees <= Self::MAX_TOTAL_FEES_BP,
            CbmmError::InvalidFeeBasisPoints
        );
        require!(
            pool_topup_fee_bp >= Self::MIN_TOPUP_FEE_BP,
            CbmmError::InvalidFeeBasisPoints
        );
        require!(
            platform_fee_bp <= Self::MAX_PLATFORM_FEE_BP,
            CbmmError::InvalidFeeBasisPoints
        );

        // 2. Validate burn tiers
        let total_fees_bp_x100 = (total_fees as u64) * 100;

        // Safe max is set to 3/4 of total fees percentage
        let safe_max_bp_x100 = (total_fees_bp_x100 * 3) / 4;

        for tier in burn_tiers {
            match &tier.role {
                BurnRole::Anyone => {
                    require!(
                        tier.burn_bp_x100 as u64 <= Self::MAX_DAILY_BURN_BP_X100_ANYONE,
                        CbmmError::InvalidBurnTiers
                    );
                }
                BurnRole::PoolOwner | BurnRole::SpecificPubkey(_) => {
                    require!(
                        tier.burn_bp_x100 as u64 <= safe_max_bp_x100,
                        CbmmError::InvalidBurnTiers
                    );
                }
            }
        }

        // 3. Validate burn rate config
        require!(
            burn_limit_bp_x100 < total_fees_bp_x100,
            CbmmError::InvalidBurnTiers
        );

        // Decay should throttle to at least 15 minute full recovery
        let max_decay = burn_limit_bp_x100 / Self::BURN_LIMIT_TIME_WINDOW_SECONDS as u64;

        // Require at least some decay rate (Allow 99% tolerance downward (slower decay is safer)
        let min_decay = max_decay / 100;

        require_gte!(
            burn_decay_rate_per_sec_bp_x100,
            min_decay,
            CbmmError::InvalidBurnRate
        );
        require_gte!(
            max_decay,
            burn_decay_rate_per_sec_bp_x100,
            CbmmError::InvalidBurnRate
        );

        Ok(())
    }

    pub fn try_new(
        bump: u8,
        admin: Pubkey,
        creator: Pubkey,
        quote_mint: Pubkey,
        burn_tiers: Vec<BurnTier>,
        pool_creator_fee_bp: u16,
        pool_topup_fee_bp: u16,
        platform_fee_bp: u16,
        burn_limit_bp_x100: u64,
        burn_min_bp_x100: u64,
        burn_decay_rate_per_sec_bp_x100: u64,
    ) -> Result<Self> {
        require!(burn_tiers.len() <= 5, CbmmError::InvalidBurnTiers);

        Self::validate_fees_and_burn_config(
            pool_creator_fee_bp,
            pool_topup_fee_bp,
            platform_fee_bp,
            &burn_tiers,
            burn_limit_bp_x100,
            burn_decay_rate_per_sec_bp_x100,
        )?;

        let burn_config = BurnRateConfig::new(
            burn_limit_bp_x100,
            burn_min_bp_x100,
            burn_decay_rate_per_sec_bp_x100,
        );

        Ok(Self {
            bump,
            admin,
            creator,
            quote_mint,
            burn_tiers,
            burn_tiers_updated_at: Clock::get()?.unix_timestamp,
            burn_rate_config: burn_config,
            pool_creator_fee_bp,
            pool_topup_fee_bp,
            platform_fee_bp,
        })
    }
}

// A is the real SPL token
// B is the virtual token
#[account]
#[derive(Default, InitSpace)]
pub struct CbmmPool {
    /// Bump seed
    pub bump: u8,
    /// Pool creator address
    pub creator: Pubkey,
    // todo maybe delete
    /// Pool index per creator
    pub pool_index: u32,
    /// Platform config used by this pool
    pub platform_config: Pubkey,

    /// A mint address
    pub quote_mint: Pubkey,
    /// A reserve including decimals
    pub quote_reserve: u64,
    /// A virtual reserve including decimals
    pub quote_virtual_reserve: u64,
    /// A optimal virtual reserve that keeps the worst-case exit price at the original value
    pub quote_optimal_virtual_reserve: u64,
    /// A starting virtual reserve that is used to calculate the optimal virtual reserve
    pub quote_starting_virtual_reserve: u64,

    /// B mint decimals
    pub base_mint_decimals: u8,
    /// B reserve including decimals
    pub base_reserve: u64,
    /// B starting total supply including decimals
    pub base_starting_total_supply: u64,
    /// B total supply including decimals
    pub base_total_supply: u64,

    /// Creator fees balance denominated in Mint A including decimals
    pub creator_fees_balance: u64,
    /// Total buyback fees accumulated in Mint A including decimals
    pub buyback_fees_balance: u64,
    /// Total platform fees accumulated in Mint A including decimals
    pub platform_fees_balance: u64,

    /// Creator fee basis points
    pub creator_fee_bp: u16,
    /// Buyback fee basis points
    pub buyback_fee_bp: u16,
    /// Platform fee basis points
    pub platform_fee_bp: u16,

    /// Burn rate limiter
    pub burn_limiter: BurnRateLimiter,
}

pub struct BurnResult {
    pub rate_limit_result: RateLimitResult,
    pub burn_amount: u64,
}

pub struct SwapResult {
    pub quote_amount: u64,
    pub base_amount: u64,
}

impl CbmmPool {
    pub fn try_new(
        bump: u8,
        creator: Pubkey,
        pool_index: u32,
        platform_config: Pubkey,
        quote_mint: Pubkey,
        quote_virtual_reserve: u64,
        creator_fee_bp: u16,
        buyback_fee_bp: u16,
        platform_fee_bp: u16,
    ) -> Result<Self> {
        require!(quote_virtual_reserve > 0, CbmmError::InvalidVirtualReserve);
        require!(buyback_fee_bp > 0, CbmmError::InvalidBuybackFeeBasisPoints);

        // Initial stress is 3/4 of total fees - to ensure the pool is not exploitable after creation
        let total_fees_bp_x100 = (creator_fee_bp + buyback_fee_bp + platform_fee_bp) as u64 * 100;
        let initial_stress_bp_x10k = total_fees_bp_x100 * 3 / 4;
        let burn_limiter =
            BurnRateLimiter::new(Clock::get()?.unix_timestamp, initial_stress_bp_x10k);

        Ok(Self {
            bump,
            creator,
            pool_index,
            platform_config,
            quote_mint,
            quote_reserve: 0,
            quote_virtual_reserve,
            quote_optimal_virtual_reserve: quote_virtual_reserve,
            quote_starting_virtual_reserve: quote_virtual_reserve,
            base_mint_decimals: DEFAULT_BASE_MINT_DECIMALS,
            base_reserve: DEFAULT_BASE_MINT_RESERVE,
            base_starting_total_supply: DEFAULT_BASE_MINT_RESERVE,
            base_total_supply: DEFAULT_BASE_MINT_RESERVE,
            creator_fees_balance: 0,
            buyback_fees_balance: 0,
            platform_fees_balance: 0,
            creator_fee_bp,
            buyback_fee_bp,
            platform_fee_bp,
            burn_limiter,
        })
    }

    pub fn collect_fees(&mut self, quote_amount: u64) -> anchor_lang::prelude::Result<u64> {
        let fees = calculate_fees(
            quote_amount,
            self.creator_fee_bp,
            self.buyback_fee_bp,
            self.platform_fee_bp,
        )?;
        self.creator_fees_balance += fees.creator_fees_amount;
        self.buyback_fees_balance += fees.buyback_fees_amount;
        self.platform_fees_balance += fees.platform_fees_amount;
        Ok(quote_amount - fees.total_fees_amount())
    }

    pub fn quote_to_base(&mut self, quote_amount: u64) -> anchor_lang::prelude::Result<SwapResult> {
        let base_amount = self.calculate_base_output_amount(quote_amount);
        self.base_reserve = self
            .base_reserve
            .checked_sub(base_amount)
            .ok_or(CbmmError::Underflow)?;
        self.quote_reserve = self
            .quote_reserve
            .checked_add(quote_amount)
            .ok_or(CbmmError::MathOverflow)?;
        Ok(SwapResult {
            quote_amount,
            base_amount,
        })
    }

    pub fn base_to_quote(&mut self, base_amount: u64) -> anchor_lang::prelude::Result<SwapResult> {
        let quote_amount = self.calculate_quote_output_amount(base_amount);
        self.quote_reserve = self
            .quote_reserve
            .checked_sub(quote_amount)
            .ok_or(CbmmError::Underflow)?;
        self.base_reserve = self
            .base_reserve
            .checked_add(base_amount)
            .ok_or(CbmmError::MathOverflow)?;
        Ok(SwapResult {
            quote_amount,
            base_amount,
        })
    }

    fn calculate_quote_output_amount(&self, base_amount: u64) -> u64 {
        calculate_sell_output_amount(
            base_amount,
            self.base_reserve,
            self.quote_reserve,
            self.quote_virtual_reserve,
        )
    }

    fn calculate_base_output_amount(&self, quote_amount: u64) -> u64 {
        calculate_buy_output_amount(
            quote_amount,
            self.quote_reserve,
            self.base_reserve,
            self.quote_virtual_reserve,
        )
    }

    pub fn burn(&mut self, config: &BurnRateConfig, requested_bp_x100: u32) -> Result<BurnResult> {
        let allowed_burn = self.burn_limiter.calculate_required_bp_x100(
            requested_bp_x100,
            &config,
            Clock::get()?.unix_timestamp,
        )?;

        let allowed_burn_bp_x100;
        match allowed_burn {
            RateLimitResult::ExecuteFull(bp_x100) => allowed_burn_bp_x100 = bp_x100,
            RateLimitResult::ExecutePartial(bp_x100) => allowed_burn_bp_x100 = bp_x100,
            RateLimitResult::Queued => {
                return Ok(BurnResult {
                    rate_limit_result: RateLimitResult::Queued,
                    burn_amount: 0,
                })
            }
        }

        let burn_amount = calculate_burn_amount(allowed_burn_bp_x100, self.base_reserve);

        self.quote_virtual_reserve = calculate_new_virtual_reserve_after_burn(
            self.quote_virtual_reserve,
            self.base_reserve,
            burn_amount,
        );
        self.quote_optimal_virtual_reserve = calculate_new_virtual_reserve_after_burn(
            self.quote_virtual_reserve,
            self.base_total_supply,
            burn_amount,
        );
        self.base_reserve -= burn_amount;
        self.base_total_supply -= burn_amount;
        Ok(BurnResult {
            rate_limit_result: allowed_burn,
            burn_amount,
        })
    }

    pub fn topup(&mut self) -> Result<u64> {
        let quote_optimal_virtual_reserve = calculate_optimal_virtual_quote_reserve(
            self.quote_starting_virtual_reserve,
            self.base_starting_total_supply,
            self.base_total_supply,
        );

        let quote_optimal_real_reserve = calculate_optimal_real_quote_reserve(
            self.base_total_supply,
            quote_optimal_virtual_reserve,
            self.base_reserve,
        );

        let needed_topup_amount = quote_optimal_real_reserve
            .checked_sub(self.quote_reserve)
            .ok_or(CbmmError::MathOverflow)?;
        if needed_topup_amount == 0 {
            return Ok(0);
        }

        let real_topup_amount = needed_topup_amount.min(self.buyback_fees_balance);
        self.buyback_fees_balance -= real_topup_amount;
        self.quote_reserve += real_topup_amount;
        self.quote_virtual_reserve = if real_topup_amount < needed_topup_amount {
            calculate_new_virtual_reserve_after_topup(
                self.quote_reserve,
                self.base_reserve,
                self.base_total_supply,
            )
        } else {
            quote_optimal_virtual_reserve
        };
        Ok(real_topup_amount)
    }

    pub fn transfer_out<'info>(
        &mut self,
        amount: u64,
        pool_account_info: &AccountInfo<'info>,
        mint: &InterfaceAccount<'info, Mint>,
        pool_ata: &InterfaceAccount<'info, TokenAccount>,
        to: &InterfaceAccount<'info, TokenAccount>,
        token_program: &Interface<'info, TokenInterface>,
    ) -> Result<()> {
        let cpi_accounts = TransferChecked {
            mint: mint.to_account_info(),
            from: pool_ata.to_account_info(),
            to: to.to_account_info(),
            authority: pool_account_info.clone(),
        };
        let bump_seed = self.bump;
        let pool_index = &self.pool_index;
        let pool_index_bytes = pool_index.to_le_bytes().to_vec();
        let signer_seeds: &[&[&[u8]]] = &[&[
            CBMM_POOL_SEED,
            pool_index_bytes.as_slice(),
            self.creator.as_ref(),
            self.platform_config.as_ref(),
            &[bump_seed],
        ]];
        let cpi_context = CpiContext::new(token_program.to_account_info(), cpi_accounts)
            .with_signer(signer_seeds);
        let decimals = mint.decimals;
        transfer_checked(cpi_context, amount, decimals)?;
        Ok(())
    }
}

#[account]
#[derive(Default, InitSpace)]
pub struct VirtualTokenAccount {
    /// Bump seed
    pub bump: u8,
    /// Pool address
    pub pool: Pubkey,
    /// Owner address
    pub owner: Pubkey,
    /// Balance of Mint B including decimals
    pub balance: u64,
}

impl VirtualTokenAccount {
    pub fn try_new(bump: u8, pool: Pubkey, owner: Pubkey) -> Self {
        Self {
            bump,
            pool,
            owner,
            balance: 0,
        }
    }

    pub fn sub(&mut self, base_amount: u64) -> Result<()> {
        self.balance = self
            .balance
            .checked_sub(base_amount)
            .ok_or(CbmmError::InsufficientVirtualTokenBalance)?;
        Ok(())
    }

    pub fn add(&mut self, base_amount: u64) -> Result<()> {
        self.balance = self
            .balance
            .checked_add(base_amount)
            .ok_or(CbmmError::MathOverflow)?;
        Ok(())
    }
}

#[account]
#[derive(Default, InitSpace)]
pub struct UserBurnAllowance {
    pub bump: u8,
    // seeds
    pub user: Pubkey,
    pub burn_tier_index: u8,
    pub burn_tier_update_timestamp: i64,
    pub platform_config: Pubkey,

    pub payer: Pubkey, // Wallet that receives funds when this account is closed
    pub burns_today: u16,

    pub last_burn_timestamp: i64,

    pub created_at: i64,
}

impl UserBurnAllowance {
    const RESET_INTERVAL_SECONDS: i64 = 86400;
    pub fn new(
        bump: u8,
        user: Pubkey,
        platform_config: Pubkey,
        payer: Pubkey,
        burn_tier_index: u8,
        burn_tier_update_timestamp: i64,
        now: i64,
    ) -> Self {
        Self {
            bump,
            user,
            platform_config,
            payer,
            burns_today: 0,
            last_burn_timestamp: 0,
            created_at: now,
            burn_tier_index,
            burn_tier_update_timestamp,
        }
    }

    pub fn pop(&mut self) -> Result<u16> {
        let now = Clock::get()?.unix_timestamp;
        if self.should_reset(now) {
            self.burns_today = 0;
        }
        self.burns_today += 1;
        self.last_burn_timestamp = now;
        Ok(self.burns_today)
    }

    pub fn is_closable(&self, platform_burn_tiers_updated_at: i64, now: i64) -> bool {
        self.burns_today == 0
            || platform_burn_tiers_updated_at > self.burn_tier_update_timestamp
            || now - self.last_burn_timestamp >= Self::RESET_INTERVAL_SECONDS
    }

    fn should_reset(&self, now: i64) -> bool {
        let reset_offset = self.created_at % Self::RESET_INTERVAL_SECONDS;
        let day_last =
            (self.last_burn_timestamp.saturating_sub(reset_offset)) / Self::RESET_INTERVAL_SECONDS;
        let day_now = (now.saturating_sub(reset_offset)) / Self::RESET_INTERVAL_SECONDS;
        day_last < day_now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    const CREATED_AT: i64 = 1761177600;
    const DAY: i64 = 86400;

    #[test_case(CREATED_AT, 0, CREATED_AT, true; "reset_on_creation")]
    #[test_case(CREATED_AT, CREATED_AT, CREATED_AT + 1, false; "reset_on_creation_and_immediately_after_creation")]
    #[test_case(CREATED_AT, 0, CREATED_AT + 1, true; "reset_at_immediately_after_creation")]
    #[test_case(CREATED_AT, CREATED_AT + DAY - 2, CREATED_AT + DAY - 1, false; "reset_today")]
    #[test_case(CREATED_AT, CREATED_AT + DAY - 1, CREATED_AT + DAY, true; "reset_yesteray")]
    #[test_case(CREATED_AT, CREATED_AT + DAY, CREATED_AT + DAY + 1, false; "reset_bound")]
    #[test_case(CREATED_AT, CREATED_AT + DAY, CREATED_AT + 20*DAY - 1, true; "reset_after_20_days")]
    fn test_should_reset(created_at: i64, last_burn_timestamp: i64, now: i64, should_reset: bool) {
        let mut user_burn_allowance = UserBurnAllowance::new(
            0,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            0,
            0,
            created_at,
        );
        user_burn_allowance.last_burn_timestamp = last_burn_timestamp;
        assert_eq!(user_burn_allowance.should_reset(now), should_reset);
    }
}
