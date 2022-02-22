use serde::Serialize;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solarium::{actor::Actor, sandbox::Sandbox, serum::Participant, token::Mint};
use std::convert::TryInto;
use std::fs;

#[derive(Serialize, Debug)]
struct TestMarketParticipant {
    keypair: String,
    base: String,
    quote: String,
}

#[derive(Serialize, Debug)]
struct TestMarket {
    url: String,
    program_id: String,
    market: String,
    participants: [TestMarketParticipant; NUM_PARTICIPANTS],
}

const NUM_PARTICIPANTS: usize = 4;

fn main() {
    let sandbox = Sandbox::new().unwrap();
    let market_creator = Actor::new(&sandbox).unwrap();
    market_creator.airdrop(10000 * LAMPORTS_PER_SOL).unwrap();
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

    let mut participants = Vec::new();
    for _ in 0..NUM_PARTICIPANTS {
        let p = Participant::new(
            &sandbox,
            &market_creator,
            &market,
            10000 * LAMPORTS_PER_SOL,
            100000,
            100000,
        )
        .unwrap();

        participants.push(TestMarketParticipant {
            keypair: p.account().keypair().to_base58_string(),
            base: p.base().pubkey().to_string(),
            quote: p.quote().pubkey().to_string(),
        });
    }

    let data = TestMarket {
        url: sandbox.url(),
        program_id: serum_program.pubkey().to_string(),
        market: market.market().pubkey().to_string(),
        participants: participants.try_into().unwrap(),
    };

    serde_json::to_writer(&fs::File::create("market.json").unwrap(), &data).unwrap();

    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open("market.json.done")
        .unwrap();

    loop {}
}
