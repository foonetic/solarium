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
    }

    /*
    #[test]
    fn serum_v1() {
        let sandbox = Sandbox::new().unwrap();
        let actor = Actor::new(&sandbox);
        actor.airdrop(10 * LAMPORTS_PER_SOL).unwrap();
        let base_mint = Mint::new(&sandbox, &actor, 0, None, None).unwrap();
        let quote_mint = Mint::new(&sandbox, &actor, 0, None, None).unwrap();
        let serum_program = actor
            .deploy(&std::path::Path::new(
                "/home/yfang/serum-dex/dex/target/deploy/serum_dex.so",
            ))
            .unwrap();

        Market::new(
            &sandbox,
            &actor,
            serum_program.pubkey(),
            &base_mint,
            &quote_mint,
            None,
            1,
            1,
            100,
            128,
            128,
            256,
        )
        .unwrap();
    }

    #[test]
    fn serum_v2() {
        let sandbox = Sandbox::new().unwrap();
        let actor = Actor::new(&sandbox);
        actor.airdrop(10 * LAMPORTS_PER_SOL).unwrap();
        let base_mint = Mint::new(&sandbox, &actor, 0, None, None).unwrap();
        let quote_mint = Mint::new(&sandbox, &actor, 0, None, None).unwrap();
        let serum_program = actor
            .deploy(&std::path::Path::new(
                "/home/yfang/serum-dex/dex/target/deploy/serum_dex.so",
            ))
            .unwrap();

        solarium::serum::Market::new(
            &sandbox,
            &actor,
            serum_program.pubkey(),
            &base_mint,
            &quote_mint,
            Some(actor.pubkey()),
            1,
            1,
            100,
            128,
            128,
            256,
        )
        .unwrap();
    }
    */
}
