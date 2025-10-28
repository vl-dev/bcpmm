use anchor_lang::prelude::*;

#[error_code]
pub enum BcpmmError {
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
    #[msg("Invalid admin")]
    InvalidAdmin,
    #[msg("Invalid mint")]
    InvalidMint,
}
