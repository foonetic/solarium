mod tests {
    use borsh::{BorshDeserialize, BorshSerialize};
    use solana_program::native_token::LAMPORTS_PER_SOL;
    use std::borrow::Borrow;
    use std::num::NonZeroU64;

    use solarium::{
        actor::Actor,
        // pyth::PriceAccount,
        sandbox::Sandbox,
        serum::Participant,
        token::{BaseOrQuote, Mint},
    };

    use serum_dex::{
        instruction::SelfTradeBehavior,
        matching::{OrderType, Side},
    };

    use pyth_client::{load_price, PriceConf};

    use std::thread::sleep;

    use std::time::Duration;

    #[derive(BorshSerialize, BorshDeserialize, Debug, Copy, Clone)]
    pub struct OpenOrders {
        pub serum_head_padding: [u8; 5],

        pub account_flags: u64, // Initialized, OpenOrders
        pub market: [u64; 4],
        pub owner: [u64; 4],

        pub native_coin_free: u64,
        pub native_coin_total: u64,

        pub native_pc_free: u64,
        pub native_pc_total: u64,

        pub free_slot_bits: u128,
        pub is_bid_bits: u128,
        pub orders: [u128; 128],
        // Using Option<NonZeroU64> in a pod type requires nightly
        pub client_order_ids: [u64; 128],
        pub referrer_rebates_accrued: u64,

        pub serum_tail_padding: [u8; 7],
    }

    // #[test]
    // fn pyth() {
    //     let sandbox = Sandbox::new().unwrap();
    //     println!("sandbox url: {}", sandbox.url());

    //     let pyth_owner = Actor::new(&sandbox).unwrap();
    //     pyth_owner.airdrop(10 * LAMPORTS_PER_SOL).unwrap();

    //     let pyth_sim = pyth_owner
    //         .deploy_remote(
    //             "https://github.com/foonetic/solarium-deps/raw/main/pyth_sim.so",
    //             "pyth_sim.so",
    //         )
    //         .unwrap();

    //     let priceAccount =
    //         solarium::pyth::PriceAccount::new(&sandbox, pyth_sim.pubkey(), &pyth_owner).unwrap();

    //     let price_data = sandbox
    //         .client()
    //         .get_account_data(priceAccount.account().pubkey())
    //         .unwrap();

    //     let val = load_price(price_data.as_slice().try_into().unwrap()).unwrap();

    //     assert_eq!(val.get_current_price().unwrap().price, 0);

    //     priceAccount
    //         .publish_price(pyth_sim.pubkey(), &pyth_owner, 15, 2)
    //         .unwrap();

    //     let price_data = sandbox
    //         .client()
    //         .get_account_data(priceAccount.account().pubkey())
    //         .unwrap();

    //     let val = load_price(price_data.as_slice().try_into().unwrap()).unwrap();

    //     assert_eq!(val.get_current_price().unwrap().price, 15);
    //     assert_eq!(val.get_current_price().unwrap().expo, 2);
    // }

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
        let _taker_order = market
            .new_order(
                &taker.quote(),
                &taker,
                Side::Bid,
                NonZeroU64::new(20).unwrap(),
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

        let _maker_order = market
            .new_order(
                &maker.base(),
                &maker,
                Side::Ask,
                NonZeroU64::new(20).unwrap(),
                OrderType::Limit,
                NonZeroU64::new(400).unwrap(),
                1,
                SelfTradeBehavior::DecrementTake,
                1,
                NonZeroU64::new(500).unwrap(),
                None,
            )
            .unwrap();

        println!("Placed ask order.");

        let maker_oo_info = sandbox
            .client()
            .get_account(maker.open_orders().pubkey())
            .unwrap();
        let maker_oo_data: OpenOrders =
            OpenOrders::try_from_slice(&maker_oo_info.data.borrow()).unwrap();
        let maker_order_id: u128 = maker_oo_data.orders[0];

        market.cancel_order(&market_creator, &maker, Side::Ask, maker_order_id);

        market.consume_events(
            &market_creator,
            vec![maker.open_orders().pubkey(), taker.open_orders().pubkey()],
            10,
        );

        market.settle_funds(&market_creator, &taker);
        market.settle_funds(&market_creator, &maker);

        let end_maker_b = get_pubkey_balance(maker.base().pubkey(), &sandbox);
        let end_taker_b = get_pubkey_balance(taker.base().pubkey(), &sandbox);

        let end_maker_q = get_pubkey_balance(maker.quote().pubkey(), &sandbox);
        let end_taker_q = get_pubkey_balance(taker.quote().pubkey(), &sandbox);

        assert_eq!(end_maker_b, "985");
        assert_eq!(end_taker_b, "1015");
        assert_eq!(end_maker_q, "2299");
        assert_eq!(end_taker_q, "1700");

        while(true) {}
    }

    fn do_vecs_match<T: PartialEq>(a: &Vec<T>, b: &Vec<T>) -> bool {
        let matching = a.iter().zip(b.iter()).filter(|&(a, b)| a == b).count();
        matching == a.len() && matching == b.len()
    }

    fn get_pubkey_balance(pubkey: &solana_program::pubkey::Pubkey, sandbox: &Sandbox) -> String {
        sandbox
            .client()
            .get_token_account_balance(pubkey)
            .unwrap()
            .amount
    }

    fn get_balance(participant: &Participant, sandbox: &Sandbox) -> String {
        get_pubkey_balance(participant.base().pubkey(), sandbox)
    }
}
