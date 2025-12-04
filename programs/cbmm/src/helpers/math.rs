use crate::errors::CbmmError;
use anchor_lang::prelude::*;

pub const X10K_100_PERCENT_BP: u64 = 100_000_000;
pub const X100_100_PERCENT_BP: u64 = 1_000_000;
pub const SCALING_FACTOR: u64 = X10K_100_PERCENT_BP / X100_100_PERCENT_BP;

#[derive(Debug)]
pub struct Fees {
    pub creator_fees_amount: u64,
    pub buyback_fees_amount: u64,
    pub platform_fees_amount: u64,
}

impl Fees {
    pub fn total_fees_amount(&self) -> u64 {
        self.creator_fees_amount + self.buyback_fees_amount + self.platform_fees_amount
    }
}

pub fn calculate_fees(
    quote_amount: u64,
    creator_fee_basis_points: u16,
    buyback_fee_basis_points: u16,
    platform_fee_basis_points: u16,
) -> Result<Fees> {
    if platform_fee_basis_points > 10000
        || creator_fee_basis_points > 10000
        || buyback_fee_basis_points > 10000
    {
        return Err(CbmmError::InvalidFeeBasisPoints.into());
    }
    if u64::MAX / (platform_fee_basis_points as u64) < quote_amount
        || u64::MAX / (creator_fee_basis_points as u64) < quote_amount
        || u64::MAX / (buyback_fee_basis_points as u64) < quote_amount
    {
        return Err(CbmmError::AmountTooBig.into());
    }
    // Use ceiling division for fees to avoid rounding down: ceil(x / d) = (x + d - 1) / d
    let creator_fees_amount =
        ((quote_amount as u128 * creator_fee_basis_points as u128 + 9999) / 10000) as u64;
    let buyback_fees_amount =
        ((quote_amount as u128 * buyback_fee_basis_points as u128 + 9999) / 10000) as u64;
    let platform_fees_amount =
        ((quote_amount as u128 * platform_fee_basis_points as u128 + 9999) / 10000) as u64;
    Ok(Fees {
        creator_fees_amount,
        buyback_fees_amount,
        platform_fees_amount,
    })
}

/// Calculates the amount of Mint B received when spending Mint A.
pub fn calculate_buy_output_amount(
    quote_amount: u64,
    quote_reserve: u64,
    base_reserve: u64,
    quote_virtual_reserve: u64,
) -> u64 {
    let numerator = base_reserve as u128 * quote_amount as u128;
    let denominator = quote_reserve as u128 + quote_virtual_reserve as u128 + quote_amount as u128;
    (numerator / denominator) as u64
}

// todo overflow and underflow checks
/// Calculates the amount of Mint A received when selling Mint B.
pub fn calculate_sell_output_amount(
    base_amount: u64,
    base_reserve: u64,
    quote_reserve: u64,
    quote_virtual_reserve: u64,
) -> u64 {
    let numerator = base_amount as u128 * (quote_reserve as u128 + quote_virtual_reserve as u128);
    let denominator = base_reserve as u128 + base_amount as u128;
    (numerator / denominator) as u64
}

pub fn calculate_burn_amount(base_amount_bp_x100: u64, base_reserve: u64) -> u64 {
    (base_reserve as u128 * base_amount_bp_x100 as u128 / X100_100_PERCENT_BP as u128) as u64
}

pub fn calculate_new_virtual_reserve_after_burn(
    quote_virtual_reserve: u64,
    base_reserve: u64,
    base_burn_amount: u64,
) -> u64 {
    // Rounding down to be sure that we stay solvent
    (quote_virtual_reserve as u128 * (base_reserve as u128 - base_burn_amount as u128)
        / base_reserve as u128) as u64
}

pub fn calculate_optimal_virtual_quote_reserve(
    quote_starting_virtual_reserve: u64,
    base_starting_total_supply: u64,
    base_total_supply: u64,
) -> u64 {
    let numerator = quote_starting_virtual_reserve as u128 * base_total_supply as u128;
    let denominator = base_starting_total_supply as u128;
    // Rounding up to be sure that we stay solvent
    ((numerator + denominator - 1) / denominator) as u64
}

pub fn calculate_optimal_real_quote_reserve(
    base_total_supply: u64,
    quote_optimal_virtual_reserve: u64,
    base_reserve: u64,
) -> u64 {
    let numerator =
        quote_optimal_virtual_reserve as u128 * (base_total_supply as u128 - base_reserve as u128);
    let denominator = base_reserve as u128;
    // Rounding up to be sure that the worst-case exit price is always at least the original price
    ((numerator + denominator - 1) / denominator) as u64
}

pub fn calculate_new_virtual_reserve_after_topup(
    quote_real_reserve: u64,
    base_reserve: u64,
    base_total_supply: u64,
) -> u64 {
    (quote_real_reserve as u128 * (base_reserve as u128)
        / (base_total_supply - base_reserve) as u128) as u64
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_calculate_fees() {
        let fees = calculate_fees(1_000_000_000, 1000, 2000, 3000).unwrap();
        println!("fees: {:?}", fees);
        assert_eq!(
            fees.creator_fees_amount, 100_000_000,
            "creator fees amount is not correct"
        );
        assert_eq!(
            fees.buyback_fees_amount, 200_000_000,
            "buyback fees amount is not correct"
        );
        assert_eq!(
            fees.platform_fees_amount, 300_000_000,
            "platform fees amount is not correct"
        );
    }

    #[test]
    fn test_calculate_amount_too_big() {
        let result = calculate_fees(u64::MAX, 10000, 10000, 10000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CbmmError::AmountTooBig.into());
    }

    #[test]
    fn test_calculate_fees_creator_fee_basis_points_overflow() {
        let result = calculate_fees(1_000_000_000, 10000, 10001, 10000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CbmmError::InvalidFeeBasisPoints.into());
    }

    #[test]
    fn test_calculate_fees_buyback_fee_basis_points_overflow() {
        let result = calculate_fees(1_000_000_000, 10001, 10000, 10000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CbmmError::InvalidFeeBasisPoints.into());
    }

    #[test]
    fn test_calculate_fees_platform_fee_basis_points_overflow() {
        let result = calculate_fees(1_000_000_000, 10000, 10000, 10001);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CbmmError::InvalidFeeBasisPoints.into());
    }
}
