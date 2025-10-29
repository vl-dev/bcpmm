use crate::errors::BcpmmError;
use anchor_lang::prelude::*;

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
    a_amount: u64,
    platform_fee_basis_points: u16,
    creator_fee_basis_points: u16,
    buyback_fee_basis_points: u16,
) -> Result<Fees> {
    if platform_fee_basis_points > 10000
        || creator_fee_basis_points > 10000
        || buyback_fee_basis_points > 10000
    {
        return Err(BcpmmError::InvalidFeeBasisPoints.into());
    }
    if u64::MAX / (platform_fee_basis_points as u64) < a_amount
        || u64::MAX / (creator_fee_basis_points as u64) < a_amount
        || u64::MAX / (buyback_fee_basis_points as u64) < a_amount
    {
        return Err(BcpmmError::AmountTooBig.into());
    }
    // Use ceiling division for fees to avoid rounding down: ceil(x / d) = (x + d - 1) / d
    let creator_fees_amount =
        ((a_amount as u128 * creator_fee_basis_points as u128 + 9999) / 10000) as u64;
    let buyback_fees_amount =
        ((a_amount as u128 * buyback_fee_basis_points as u128 + 9999) / 10000) as u64;
    let platform_fees_amount =
        ((a_amount as u128 * platform_fee_basis_points as u128 + 9999) / 10000) as u64;
    Ok(Fees {
        creator_fees_amount,
        buyback_fees_amount,
        platform_fees_amount,
    })
}

/// Calculates the amount of Mint B received when spending Mint A.
pub fn calculate_buy_output_amount(
    a_amount: u64,
    a_reserve: u64,
    b_reserve: u64,
    a_virtual_reserve: u64,
) -> u64 {
    let numerator = b_reserve as u128 * a_amount as u128;
    let denominator = a_reserve as u128 + a_virtual_reserve as u128 + a_amount as u128;
    (numerator / denominator) as u64
}

// todo overflow and underflow checks
/// Calculates the amount of Mint A received when selling Mint B.
pub fn calculate_sell_output_amount(
    b_amount: u64,
    b_reserve: u64,
    a_reserve: u64,
    a_virtual_reserve: u64,
) -> u64 {
    let numerator = b_amount as u128 * (a_reserve as u128 + a_virtual_reserve as u128);
    let denominator = b_reserve as u128 + b_amount as u128;
    (numerator / denominator) as u64
}

pub fn calculate_burn_amount(b_amount_bp_x100: u32, b_reserve: u64) -> u64 {
    (b_reserve as u128 * b_amount_bp_x100 as u128 / 1_000_000 as u128) as u64
}

pub fn calculate_new_virtual_reserve(
    a_virtual_reserve: u64,
    b_reserve: u64,
    b_burn_amount: u64,
) -> u64 {
    (a_virtual_reserve as u128 * (b_reserve as u128 - b_burn_amount as u128) / b_reserve as u128)
        as u64
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_calculate_fees() {
        let fees = calculate_fees(1_000_000_000, 1000, 2000, 3000).unwrap();
        assert_eq!(fees.creator_fees_amount, 100_000_000);
        assert_eq!(fees.buyback_fees_amount, 200_000_000);
        assert_eq!(fees.platform_fees_amount, 300_000_000);
    }

    #[test]
    fn test_calculate_amount_too_big() {
        let result = calculate_fees(u64::MAX, 10000, 10000, 10000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), BcpmmError::AmountTooBig.into());
    }

    #[test]
    fn test_calculate_fees_creator_fee_basis_points_overflow() {
        let result = calculate_fees(1_000_000_000, 10000, 10001, 10000);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            BcpmmError::InvalidFeeBasisPoints.into()
        );
    }

    #[test]
    fn test_calculate_fees_buyback_fee_basis_points_overflow() {
        let result = calculate_fees(1_000_000_000, 10001, 10000, 10000);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            BcpmmError::InvalidFeeBasisPoints.into()
        );
    }

    #[test]
    fn test_calculate_fees_platform_fee_basis_points_overflow() {
        let result = calculate_fees(1_000_000_000, 10000, 10000, 10001);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            BcpmmError::InvalidFeeBasisPoints.into()
        );
    }
}
