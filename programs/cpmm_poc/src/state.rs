use crate::errors::BcpmmError;
use crate::helpers::{calculate_fees, calculate_sell_output_amount, Fees};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

pub const CENTRAL_STATE_SEED: &[u8] = b"central_state";
pub const BCPMM_POOL_SEED: &[u8] = b"bcpmm_pool";
pub const VIRTUAL_TOKEN_ACCOUNT_SEED: &[u8] = b"virtual_token_account";
pub const USER_BURN_ALLOWANCE_SEED: &[u8] = b"user_burn_allowance";

pub const DEFAULT_B_MINT_DECIMALS: u8 = 6;
pub const DEFAULT_B_MINT_RESERVE: u64 = 1_000_000_000 * 10u64.pow(DEFAULT_B_MINT_DECIMALS as u32);

#[account]
#[derive(Default, InitSpace)]
pub struct CentralState {
    pub bump: u8,
    pub admin: Pubkey,
    pub b_mint_index: u64,
    pub daily_burn_allowance: u16,
    pub creator_daily_burn_allowance: u16,
    pub user_burn_bp_x100: u32, 
    pub creator_burn_bp_x100: u32,
    pub burn_reset_time_of_day_seconds: u32, // Seconds from midnight
}

/// Check if given time is after today's burn reset timestamp (for testing with mock time).
pub fn is_after_burn_reset_with_time( time_to_check: i64, current_time: i64, reset_time_of_day_seconds: u32) -> bool {
    let todays_midnight = current_time - current_time.rem_euclid(86400);
    let todays_reset_ts = todays_midnight + reset_time_of_day_seconds as i64;
    time_to_check >= todays_reset_ts
}

impl CentralState {
    pub fn new(
        bump: u8,
        admin: Pubkey,
        daily_burn_allowance: u16,
        creator_daily_burn_allowance: u16,
        user_burn_bp_x100: u32,
        creator_burn_bp_x100: u32,
        burn_reset_time_of_day_seconds: u32,
    ) -> Self {
        Self {
            bump,
            admin,
            b_mint_index: 0,
            daily_burn_allowance,
            creator_daily_burn_allowance,
            user_burn_bp_x100,
            creator_burn_bp_x100,
            burn_reset_time_of_day_seconds,
        }
    }

    /// Check if given time is after today's burn reset timestamp.
    pub fn is_after_burn_reset(&self, time_to_check: i64) -> Result<bool> {
        let now = Clock::get()?.unix_timestamp;
        Ok(is_after_burn_reset_with_time(time_to_check, now, self.burn_reset_time_of_day_seconds))
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

    /// A mint address
    pub a_mint: Pubkey,
    /// A reserve including decimals
    pub a_reserve: u64,
    /// A virtual reserve including decimals
    pub a_virtual_reserve: u64,
    // A remaining topup to compensate for the virtual reserve reduction happening on burn
    pub a_remaining_topup: u64,

    /// B mint is virtual and denoted by index
    pub b_mint_index: u64,
    /// B mint decimals
    pub b_mint_decimals: u8,
    /// B reserve including decimals
    pub b_reserve: u64,

    /// Creator fees balance denominated in Mint A including decimals
    pub creator_fees_balance: u64,
    /// Buyback fees balance denominated in Mint A including decimals
    pub buyback_fees_balance: u64,

    /// Creator fee basis points
    pub creator_fee_basis_points: u16,
    /// Buyback fee basis points
    pub buyback_fee_basis_points: u16,

    /// Burn allowance for the pool
    pub burns_today: u16,
    pub last_burn_timestamp: i64,
}

impl BcpmmPool {
    pub fn try_new(
        bump: u8,
        creator: Pubkey,
        a_mint: Pubkey,
        a_virtual_reserve: u64,
        b_mint_index: u64,
        creator_fee_basis_points: u16,
        buyback_fee_basis_points: u16,
    ) -> Result<Self> {
        require!(a_virtual_reserve > 0, BcpmmError::InvalidVirtualReserve);
        require!(
            buyback_fee_basis_points > 0,
            BcpmmError::InvalidBuybackFeeBasisPoints
        );

        Ok(Self {
            bump,
            creator,
            a_mint,
            a_reserve: 0,
            a_virtual_reserve,
            a_remaining_topup: 0,
            b_mint_index,
            b_mint_decimals: DEFAULT_B_MINT_DECIMALS,
            b_reserve: DEFAULT_B_MINT_RESERVE,
            creator_fees_balance: 0,
            buyback_fees_balance: 0,
            creator_fee_basis_points,
            buyback_fee_basis_points,
            burns_today: 0,
            last_burn_timestamp: 0,
        })
    }

    pub fn calculate_fees(&self, a_amount: u64) -> anchor_lang::prelude::Result<Fees> {
        calculate_fees(
            a_amount,
            self.creator_fee_basis_points,
            self.buyback_fee_basis_points,
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

    pub fn add(
        &mut self,
        output_amount: u64,
        b_amount: u64,
        creator_fees_amount: u64,
        buyback_fees_amount: u64,
    ) {
        self.a_reserve -= output_amount;
        self.b_reserve += b_amount;
        self.creator_fees_balance += creator_fees_amount;

        if self.a_remaining_topup > 0 {
            let remaining_topup_amount = self.a_remaining_topup;
            let real_topup_amount = if remaining_topup_amount > buyback_fees_amount {
                buyback_fees_amount
            } else {
                remaining_topup_amount
            };
            self.a_remaining_topup = self.a_remaining_topup - real_topup_amount;
            self.a_reserve += real_topup_amount;
        } else {
            self.buyback_fees_balance += buyback_fees_amount;
            // Record to some central state instead so we can claim for all pools at once?
        }
    }

    pub fn transfer_out<'info>(
        &mut self,
        amount: u64,
        pool_account_info: AccountInfo<'info>,
        mint: &InterfaceAccount<'info, Mint>,
        pool_ata: &InterfaceAccount<'info, TokenAccount>,
        to: &InterfaceAccount<'info, TokenAccount>,
        token_program: &Interface<'info, TokenInterface>,
    ) -> Result<()> {
        let cpi_accounts = TransferChecked {
            mint: mint.to_account_info(),
            from: pool_ata.to_account_info(),
            to: to.to_account_info(),
            authority: pool_account_info,
        };
        let bump_seed = self.bump;
        let b_mint_index = &self.b_mint_index;
        let b_mint_index_bytes = b_mint_index.to_le_bytes().to_vec();
        let signer_seeds: &[&[&[u8]]] =
            &[&[BCPMM_POOL_SEED, b_mint_index_bytes.as_slice(), &[bump_seed]]];
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

    pub fn sub(
        &mut self,
        b_amount: u64,
        creator_fees_amount: u64,
        buyback_fees_amount: u64,
    ) -> Result<()> {
        require_gte!(
            self.balance,
            b_amount,
            BcpmmError::InsufficientVirtualTokenBalance
        );
        self.balance -= b_amount;
        self.fees_paid += creator_fees_amount + buyback_fees_amount;
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
    pub fn new(
        bump: u8,
        user: Pubkey,
        payer: Pubkey,
    ) -> Self {
        Self { bump, user, payer, burns_today: 0, last_burn_timestamp: 0 }
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
        assert!(!is_after_burn_reset_with_time(time_before_reset, current_time, 43200));
    }

    #[test]
    fn test_is_after_burn_reset_with_time_yesterday() {
        let midnight = 1761177600;
        let current_time = midnight + 1;
        let yesterday_night = 1761166800;
        assert!(!is_after_burn_reset_with_time(yesterday_night, current_time, 43200));
    }

    #[test]
    fn test_is_after_burn_reset_with_time_same_day() {
        let midnight = 1761177600;
        let current_time = midnight + 1;
        let time_after_reset_same_day = 1761224400;
        assert!(is_after_burn_reset_with_time(time_after_reset_same_day, current_time, 43200));
    }

    #[test]
    fn test_is_after_burn_reset_with_time_next_day() {
        let midnight = 1761177600;
        let current_time = midnight + 1;
        let next_day = 1761264000;
        assert!(is_after_burn_reset_with_time(next_day, current_time, 43200));
    }
}
