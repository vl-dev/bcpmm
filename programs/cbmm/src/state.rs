use crate::errors::BcpmmError;
use crate::helpers::{
    calculate_buy_output_amount, calculate_fees, calculate_sell_output_amount, Fees,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

pub const PLATFORM_CONFIG_SEED: &[u8] = b"platform_config";
pub const BCPMM_POOL_SEED: &[u8] = b"bcpmm_pool";
pub const BCPMM_POOL_INDEX_SEED: u32 = 0; // this is introduced for extensibility - if we ever need more that one pool per user, we can use this to differentiate them
pub const VIRTUAL_TOKEN_ACCOUNT_SEED: &[u8] = b"virtual_token_account";
pub const USER_BURN_ALLOWANCE_SEED: &[u8] = b"user_burn_allowance";

pub const DEFAULT_B_MINT_DECIMALS: u8 = 6;
pub const DEFAULT_B_MINT_RESERVE: u64 = 1_000_000_000 * 10u64.pow(DEFAULT_B_MINT_DECIMALS as u32);
pub const DEFAULT_BURN_TIERS_UPDATE_COOLDOWN_SECONDS: i64 = 86400; // 24 hours

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub enum BurnRole {
    Anyone,                 // Permissionless - anyone can burn at this tier
    PoolCreator,            // Only the pool creator can burn at this tier
    SpecificPubkey(Pubkey), // Only a specific whitelisted pubkey can burn
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
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

    // todo - nicer to do platform creation timestamp instead of time of day
    pub burn_reset_time_of_day_seconds: u32, // Seconds from midnight

    pub pool_creator_fee_basis_points: u16,
    pub pool_topup_fee_basis_points: u16,
    pub platform_fee_basis_points: u16,

    pub burn_tiers_updated_at: i64, // needed for cooling off period
    #[max_len(5)]
    pub burn_tiers: Vec<BurnTier>,
}

/// Check if given time is after today's burn reset timestamp (for testing with mock time).
pub fn is_after_burn_reset_with_time(
    time_to_check: i64,
    current_time: i64,
    reset_time_of_day_seconds: u32,
) -> bool {
    let todays_midnight = current_time - current_time.rem_euclid(86400);
    let todays_reset_ts = todays_midnight + reset_time_of_day_seconds as i64;
    time_to_check >= todays_reset_ts
}

impl PlatformConfig {
    pub fn try_new(
        bump: u8,
        admin: Pubkey,
        creator: Pubkey,
        quote_mint: Pubkey,
        burn_tiers: Vec<BurnTier>,
        burn_reset_time_of_day_seconds: u32,
        pool_creator_fee_basis_points: u16,
        pool_topup_fee_basis_points: u16,
        platform_fee_basis_points: u16,
    ) -> Result<Self> {
        require!(burn_tiers.len() <= 5, BcpmmError::InvalidBurnTiers);
        let total_fees =
            pool_creator_fee_basis_points + pool_topup_fee_basis_points + platform_fee_basis_points;
        // 1% fee minimum to be able to do reasonable burns
        require!(
            total_fees <= 10_000 && total_fees >= 100,
            BcpmmError::InvalidFeeBasisPoints
        );

        // check that every burn tier has the amount at most 1/10 of the total fees - keeps the burning reasonably safe
        // todo doublecheck that 1/10 is the right number
        require!(
            !&burn_tiers
                .iter()
                .any(|tier| tier.burn_bp_x100 / 100 > total_fees as u32 / 10),
            BcpmmError::InvalidBurnTiers
        );

        Ok(Self {
            bump,
            admin,
            creator,
            quote_mint,
            burn_tiers,
            burn_tiers_updated_at: Clock::get()?.unix_timestamp,
            burn_reset_time_of_day_seconds,
            pool_creator_fee_basis_points,
            pool_topup_fee_basis_points,
            platform_fee_basis_points,
        })
    }

    /// Check if given time is after today's burn reset timestamp.
    pub fn is_after_burn_reset(&self, time_to_check: i64) -> Result<bool> {
        let now = Clock::get()?.unix_timestamp;
        Ok(is_after_burn_reset_with_time(
            time_to_check,
            now,
            self.burn_reset_time_of_day_seconds,
        ))
    }
}

// A is the real SPL token
// B is the virtual token
#[account]
#[derive(Default, InitSpace)]
pub struct BcpmmPool {
    /// Bump seed
    pub bump: u8,
    /// Pool creator address
    pub creator: Pubkey,
    /// Pool index per creator
    pub pool_index: u32,
    /// Platform config used by this pool
    pub platform_config: Pubkey,

    /// A mint address
    pub a_mint: Pubkey,
    /// A reserve including decimals
    pub a_reserve: u64,
    /// A virtual reserve including decimals
    pub a_virtual_reserve: u64,
    // A remaining topup to compensate for the virtual reserve reduction happening on burn
    pub a_outstanding_topup: u64,

    /// B mint decimals
    pub b_mint_decimals: u8,
    /// B reserve including decimals
    pub b_reserve: u64,

    /// Creator fees balance denominated in Mint A including decimals
    pub creator_fees_balance: u64,
    /// Total buyback fees accumulated in Mint A including decimals
    pub buyback_fees_balance: u64,

    /// Creator fee basis points
    pub creator_fee_basis_points: u16,
    /// Buyback fee basis points
    pub buyback_fee_basis_points: u16,
    /// Platform fee basis points
    pub platform_fee_basis_points: u16,

    /// Burn allowance for the pool
    pub burns_today: u16,
    pub last_burn_timestamp: i64,
    // TODO: burn amounts here?
}

impl BcpmmPool {
    pub fn try_new(
        bump: u8,
        creator: Pubkey,
        pool_index: u32,
        platform_config: Pubkey,
        a_mint: Pubkey,
        a_virtual_reserve: u64,
        creator_fee_basis_points: u16,
        buyback_fee_basis_points: u16,
        platform_fee_basis_points: u16,
    ) -> Result<Self> {
        require!(a_virtual_reserve > 0, BcpmmError::InvalidVirtualReserve);
        require!(
            buyback_fee_basis_points > 0,
            BcpmmError::InvalidBuybackFeeBasisPoints
        );

        Ok(Self {
            bump,
            creator,
            pool_index,
            platform_config,
            a_mint,
            a_reserve: 0,
            a_virtual_reserve,
            a_outstanding_topup: 0,
            b_mint_decimals: DEFAULT_B_MINT_DECIMALS,
            b_reserve: DEFAULT_B_MINT_RESERVE,
            creator_fees_balance: 0,
            buyback_fees_balance: 0,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            platform_fee_basis_points,
            burns_today: 0,
            last_burn_timestamp: 0,
        })
    }

    pub fn calculate_fees(&self, a_amount: u64) -> anchor_lang::prelude::Result<Fees> {
        calculate_fees(
            a_amount,
            self.creator_fee_basis_points,
            self.buyback_fee_basis_points,
            self.platform_fee_basis_points,
        )
    }

    pub fn calculate_sell_output_amount(&self, b_amount: u64) -> u64 {
        calculate_sell_output_amount(
            b_amount,
            self.b_reserve,
            self.a_reserve,
            self.a_virtual_reserve,
        )
    }

    pub fn calculate_buy_output_amount(&self, a_amount: u64) -> u64 {
        calculate_buy_output_amount(
            a_amount,
            self.a_reserve,
            self.b_reserve,
            self.a_virtual_reserve,
        )
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
            BCPMM_POOL_SEED,
            pool_index_bytes.as_slice(),
            self.creator.as_ref(),
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
    /// All fees paid when buying and selling tokens to this account. Denominated in Mint A including decimals
    pub fees_paid: u64,
}

impl VirtualTokenAccount {
    pub fn try_new(bump: u8, pool: Pubkey, owner: Pubkey) -> Self {
        Self {
            bump,
            pool,
            owner,
            balance: 0,
            fees_paid: 0,
        }
    }

    pub fn sub(&mut self, b_amount: u64, fees: &Fees) -> Result<()> {
        require_gte!(
            self.balance,
            b_amount,
            BcpmmError::InsufficientVirtualTokenBalance
        );
        self.balance -= b_amount;
        self.fees_paid += fees.total_fees_amount();
        Ok(())
    }

    pub fn add(&mut self, b_amount: u64, fees: &Fees) -> Result<()> {
        self.balance += b_amount;
        self.fees_paid += fees.total_fees_amount();
        Ok(())
    }
}

#[account]
#[derive(Default, InitSpace)]
pub struct UserBurnAllowance {
    pub bump: u8,
    pub user: Pubkey,
    pub payer: Pubkey, // Wallet that receives funds when this account is closed
    pub burns_today: u16,

    pub last_burn_timestamp: i64,
}

impl UserBurnAllowance {
    pub fn new(bump: u8, user: Pubkey, payer: Pubkey) -> Self {
        Self {
            bump,
            user,
            payer,
            burns_today: 0,
            last_burn_timestamp: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_after_burn_reset_with_time_before_reset() {
        let midnight = 1761177600;
        let current_time = midnight + 1;
        let time_before_reset = 1761177660; // Just after midnight
        assert!(!is_after_burn_reset_with_time(
            time_before_reset,
            current_time,
            43200
        ));
    }

    #[test]
    fn test_is_after_burn_reset_with_time_yesterday() {
        let midnight = 1761177600;
        let current_time = midnight + 1;
        let yesterday_night = 1761166800;
        assert!(!is_after_burn_reset_with_time(
            yesterday_night,
            current_time,
            43200
        ));
    }

    #[test]
    fn test_is_after_burn_reset_with_time_same_day() {
        let midnight = 1761177600;
        let current_time = midnight + 1;
        let time_after_reset_same_day = 1761224400;
        assert!(is_after_burn_reset_with_time(
            time_after_reset_same_day,
            current_time,
            43200
        ));
    }

    #[test]
    fn test_is_after_burn_reset_with_time_next_day() {
        let midnight = 1761177600;
        let current_time = midnight + 1;
        let next_day = 1761264000;
        assert!(is_after_burn_reset_with_time(next_day, current_time, 43200));
    }
}
