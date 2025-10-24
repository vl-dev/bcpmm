mod burn_virtual_token;
mod buy_virtual_token;
mod close_user_burn_allowance;
mod close_virtual_token_account;
mod create_pool;
mod initialize_central_state;
mod initialize_treasury;
mod initialize_user_burn_allowance;
mod initialize_virtual_token_account;
mod sell_virtual_token;
mod update_treasury_authority;
mod claim_creator_fees;
mod claim_admin_fees;

pub use burn_virtual_token::*;
pub use buy_virtual_token::*;
pub use close_user_burn_allowance::*;
pub use close_virtual_token_account::*;
pub use create_pool::*;
pub use initialize_central_state::*;
pub use initialize_treasury::*;
pub use initialize_user_burn_allowance::*;
pub use initialize_virtual_token_account::*;
pub use sell_virtual_token::*;
pub use update_treasury_authority::*;
pub use claim_creator_fees::*;
pub use claim_admin_fees::*;

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