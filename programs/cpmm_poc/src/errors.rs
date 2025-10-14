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
}
