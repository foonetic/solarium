mod tests {
    use solana_program::native_token::LAMPORTS_PER_SOL;
    use solarium::{actor::Actor, sandbox::Sandbox, token::Mint, token::TokenAccount};

    #[test]
    fn integration() {
        let sandbox = Sandbox::new().unwrap();
        let actor = Actor::new(&sandbox);
        actor.airdrop(10 * LAMPORTS_PER_SOL).unwrap();
        let mint = Mint::new(&sandbox, &actor, 0, None, None).unwrap();
        let token_account = TokenAccount::new(&sandbox, &actor, &mint, None).unwrap();

        let account_info = token_account.get_state().unwrap();
        assert_eq!(0, account_info.amount);
        mint.mint_to(&actor, &token_account, 123).unwrap();
        let account_info = token_account.get_state().unwrap();
        assert_eq!(123, account_info.amount);
        println!("amount after mint = {}", account_info.amount);
    }
}
