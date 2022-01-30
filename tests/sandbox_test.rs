mod tests {
    use solana_program::account_info::AccountInfo;
    use solana_program::native_token::LAMPORTS_PER_SOL;
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;

    use solarium::{
        actor::Actor,
        sandbox::Sandbox,
        serum::{Market, Participant},
        token::Mint,
        token::TokenAccount,
    };

    use crank::{start, Command, Opts};

    use serum_dex::instruction::{consume_events, MarketInstruction};

    use serum_dex::state::strip_header;

    use serum_dex::{
        instruction::SelfTradeBehavior,
        matching::{OrderType, Side},
        state as serum_state,
    };

    use serum_common::client::Cluster;

    use std::num::NonZeroU64;

    #[test]
    fn integration() {
        let sandbox = Sandbox::new().unwrap();
        println!("sandbox url: {}", sandbox.url());
        let market_creator = Actor::new(&sandbox).unwrap();
        market_creator.airdrop(10 * LAMPORTS_PER_SOL).unwrap();
        let base_mint = Mint::new(&sandbox, &market_creator, 0, None, None).unwrap();
        let quote_mint = Mint::new(&sandbox, &market_creator, 0, None, None).unwrap();
        let serum_program = market_creator
            .deploy_remote(
                "https://github.com/foonetic/solarium-deps/raw/main/serum_dex.so",
                "serum_dex.so",
            )
            .unwrap();

        let market = solarium::serum::Market::new(
            &sandbox,
            &market_creator,
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
        println!("Made market.");

        let maker = Participant::new(
            &sandbox,
            &market_creator,
            &market,
            10 * LAMPORTS_PER_SOL,
            1000,
            2000,
        )
        .unwrap();
        let taker = Participant::new(
            &sandbox,
            &market_creator,
            &market,
            10 * LAMPORTS_PER_SOL,
            1000,
            2000,
        )
        .unwrap();

        // Place ask order
        let taker_order = market
            .new_order(
                &taker.quote(),
                &taker,
                Side::Bid,
                NonZeroU64::new(200).unwrap(),
                OrderType::Limit,
                NonZeroU64::new(500).unwrap(),
                1,
                SelfTradeBehavior::DecrementTake,
                1,
                NonZeroU64::new(300).unwrap(),
                None,
            )
            .unwrap();
        println!("Placed bid order.");

        let maker_order = market
            .new_order(
                &maker.base(),
                &maker,
                Side::Ask,
                NonZeroU64::new(200).unwrap(),
                OrderType::Limit,
                NonZeroU64::new(500).unwrap(),
                1,
                SelfTradeBehavior::DecrementTake,
                1,
                NonZeroU64::new(300).unwrap(),
                None,
            )
            .unwrap();

        println!("Placed ask order.");

        market
            .consume_events_loop(&market_creator, 1, 1, String::from("./crank_log.txt"), 6000)
            .unwrap();

        market.settle_funds(&market_creator, &maker).unwrap();
        market.settle_funds(&market_creator, &taker).unwrap();

        let taker_after_balance: String = get_balance(&taker, &sandbox);
        let maker_after_balance: String = get_balance(&maker, &sandbox);

        assert_eq!(taker_after_balance, "1000");
        assert_eq!(maker_after_balance, "500");
    }

    fn do_vecs_match<T: PartialEq>(a: &Vec<T>, b: &Vec<T>) -> bool {
        let matching = a.iter().zip(b.iter()).filter(|&(a, b)| a == b).count();
        matching == a.len() && matching == b.len()
    }

    fn get_balance(participant: &Participant, sandbox: &Sandbox) -> String {
        sandbox
            .client()
            .get_token_account_balance(participant.base().pubkey())
            .unwrap()
            .amount
    }
}
