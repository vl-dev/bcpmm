use anchor_lang::prelude::*;

#[error_code]
pub enum CbmmError {
    #[msg("Insufficient virtual token balance")]
    InsufficientVirtualTokenBalance,
    #[msg("Amount too small")]
    AmountTooSmall,
    #[msg("Invalid owner")]
    InvalidOwner,
    #[msg("Nonzero balance")]
    NonzeroBalance,
    #[msg("Invalid fee basis points")]
    InvalidFeeBasisPoints,
    #[msg("Amount too big")]
    AmountTooBig,
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    #[msg("Insufficient burn allowance")]
    InsufficientBurnAllowance,
    #[msg("Cannot close active burn allowance")]
    CannotCloseActiveBurnAllowance,
    #[msg("Invalid burn account payer")]
    InvalidBurnAccountPayer,
    #[msg("Invalid virtual reserve")]
    InvalidVirtualReserve,
    #[msg("Invalid buyback fee basis points")]
    InvalidBuybackFeeBasisPoints,
    #[msg("Underflow")]
    Underflow,
    #[msg("Invalid pool owner")]
    InvalidPoolOwner,
    #[msg("Invalid mint")]
    InvalidMint,
    #[msg("Invalid platform admin")]
    InvalidPlatformAdmin,
    #[msg("Invalid burn tiers")]
    InvalidBurnTiers,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Invalid burn tier index")]
    InvalidBurnTierIndex,
    #[msg("Pool creator burn tier requires pool")]
    PoolCreatorBurnTierRequiresPool,
    #[msg("Invalid pool creator")]
    InvalidPoolCreator,
    #[msg("Invalid platform config")]
    InvalidPlatformConfig,
    #[msg("Burn limit reached")]
    BurnLimitReached,
    #[msg("Invalid burn tiers length")]
    InvalidBurnTiersLength,
    #[msg("Invalid burn rate")]
    InvalidBurnRate,
}
