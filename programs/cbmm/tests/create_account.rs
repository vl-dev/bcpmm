use litesvm::LiteSVM;
use solana_address::Address;
use solana_sdk::signature::{Keypair, Signer};

#[test]
fn create_account() {
    let mut svm = LiteSVM::new();
    let user = Keypair::new();
    let user_addr: Address = Address::from(user.pubkey());
    svm.airdrop(&user_addr, 1_000_000_000).unwrap();
    let balance = svm.get_balance(&user_addr).unwrap();
    assert_eq!(balance, 1_000_000_000);
    println!("Account funded with {} SOL", balance as f64 / 1e9);
}
