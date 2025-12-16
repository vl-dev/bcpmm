mod burn_virtual_token;
mod buy_virtual_token;
mod claim_creator_fees;
mod claim_platform_fees;
mod close_user_burn_allowance;
mod close_virtual_token_account;
mod create_pool;
mod initialize_platform_config;
mod initialize_user_burn_allowance;
mod initialize_virtual_token_account;
mod sell_virtual_token;
mod update_platform_config;

pub use burn_virtual_token::*;
pub use buy_virtual_token::*;
pub use claim_creator_fees::*;
pub use claim_platform_fees::*;
pub use close_user_burn_allowance::*;
pub use close_virtual_token_account::*;
pub use create_pool::*;
pub use initialize_platform_config::*;
pub use initialize_user_burn_allowance::*;
pub use initialize_virtual_token_account::*;
pub use sell_virtual_token::*;
pub use update_platform_config::*;

// Setup metrics collection for all tests.
#[cfg(test)]
mod tests {
    use crate::test_utils::{init_metrics, print_metrics_report};

    #[ctor::ctor]
    fn init() {
        init_metrics();
    }

    #[ctor::dtor]
    fn cleanup() {
        print_metrics_report();
    }
}
