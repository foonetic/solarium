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

    // #[test]
    // fn integration() {
    //     let sandbox = Sandbox::new().unwrap();
    //     let actor = Actor::new(&sandbox);
    //     actor.airdrop(10 * LAMPORTS_PER_SOL).unwrap();
    //     let mint = Mint::new(&sandbox, &actor, 0, None, None).unwrap();
    //     let token_account = TokenAccount::new(&sandbox, &actor, &mint, None).unwrap();

    //     let account_info = token_account.get_state().unwrap();
    //     assert_eq!(0, account_info.amount);
    //     mint.mint_to(&actor, &token_account, 123).unwrap();
    //     let account_info = token_account.get_state().unwrap();
    //     assert_eq!(123, account_info.amount);
    // }

    #[test]
    fn order_crank_market() {
        let sandbox = Sandbox::new().unwrap();
        println!("sandbox url: {}", sandbox.url());
        let market_creator = Actor::new(&sandbox);
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

        crank_market(&market, &market_creator);
        settle_funds(&market, &market_creator, &maker);
        settle_funds(&market, &market_creator, &taker);

        let taker_after_balance: String = get_balance(&taker, &sandbox);
        let maker_after_balance: String = get_balance(&maker, &sandbox);

        assert_eq!(taker_after_balance, "1000");
        assert_eq!(maker_after_balance, "500");
    }

    fn crank_market(market: &Market, market_creator: &Actor) -> () {
        let consume_events = crank::Command::ConsumeEvents {
            dex_program_id: *market.serum(),
            payer: market_creator.keyfile().to_str().unwrap().to_string(),
            market: *market.market().pubkey(),
            coin_wallet: *market.base_vault().account().pubkey(),
            pc_wallet: *market.quote_vault().account().pubkey(),
            num_workers: 1,
            events_per_worker: 1,
            num_accounts: None,
            log_directory: "./crank_log.txt".to_string(),
            max_q_length: None,
            max_wait_for_events_delay: None,
        };

        let crank_opts = crank::Opts {
            cluster: serum_common::client::Cluster::Custom(market_creator.sandbox().url()),
            command: consume_events,
        };

        thread::spawn(|| {
            crank::start(crank_opts);
        });

        println!("Waiting for crank");
        //crank for 6 seconds
        sleep(Duration::from_millis(6000));
        println!("Cranked");
    }

    fn settle_funds(market: &Market, market_creator: &Actor, side: &Participant) -> () {
        let settle = crank::Command::SettleFunds {
            payer: market_creator.keyfile().to_str().unwrap().to_string(),
            dex_program_id: *market.serum(),
            market: *market.market().pubkey(),
            orders: *side.open_orders().pubkey(),
            coin_wallet: *market.base_vault().account().pubkey(),
            pc_wallet: *market.quote_vault().account().pubkey(),
            signer: None,
        };

        let settle_opts = crank::Opts {
            cluster: serum_common::client::Cluster::Custom(market_creator.sandbox().url()),
            command: settle,
        };

        crank::start(settle_opts);
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

    /*
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

        let market = solarium::serum::Market::new(
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

        let _market_maker =
            Participant::new(&sandbox, &actor, &market, 10 * LAMPORTS_PER_SOL, 1000, 2000).unwrap();
    }
    */
}
