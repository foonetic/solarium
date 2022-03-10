//! Uses Solarium to create a local test market running in a local test
//! validator. All of the relevant keys related to the market are saved in
//! market.json.
//!
//! This program is meant to be used in conjunction with serum-market-maker and
//! serum-place-order examples. Please see the documentation under
//! serum-market-maker for more details.
//!
use clap::Parser;
use serde::Serialize;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solarium::{actor::Actor, sandbox::Sandbox, serum::Participant, token::Mint};
use std::convert::TryInto;
use std::fs;

// Represents a Serum market participant that was initialized by this binary.
#[derive(Serialize, Debug)]
struct TestMarketParticipant {
    // Participant's private key.
    keypair: String,

    // Public key of participant's base token account.
    base: String,

    // Public key of participant's quote token account.
    quote: String,

    // Public key of participant's open orders account.
    orders: String,
}

// Represents a Serum market that was initialized by this binary.
#[derive(Serialize, Debug)]
struct TestMarket {
    // Solana test validator RPC endpoint.
    url: String,

    // Serum program public key.
    program_id: String,

    // Public key of the market initialized by this binary.
    market: String,

    // Public key of the market's bids slab initialized by this binary.
    bids: String,

    // Public key of the market's asks slab initialized by this binary.
    asks: String,

    // Public key of the market's request queue initialized by this binary.
    request_queue: String,

    // Public key of the market's event queue initialized by this binary.
    event_queue: String,

    // Public key of the market's base vault initialized by this binary.
    base_vault: String,

    // Public key of the market's quote vault initialized by this binary.
    quote_vault: String,

    // Public key of the market's base mint initialized by this binary.
    base_mint: String,

    // Public key of the market's quote mint initialized by this binary.
    quote_mint: String,

    // Market participants initialized by this binary.
    participants: [TestMarketParticipant; NUM_PARTICIPANTS],
}

const NUM_PARTICIPANTS: usize = 4;

#[derive(Parser, Debug)]
struct CliArgs {
    #[clap(long, help = "serum dex's deploy url", default_value_t = String::from("https://github.com/foonetic/solarium-deps/raw/main/serum_dex.so"))]
    pub dex_url: String,
    #[clap(long, help = "base mint decimal", default_value_t = 0)]
    pub base_decimal: u8,
    #[clap(long, help = "quote mint decimal", default_value_t = 0)]
    pub quote_decimal: u8,
    #[clap(long, help = "base lot size", default_value_t = 1)]
    pub base_lot_size: u64,
    #[clap(long, help = "quote lot size", default_value_t = 1)]
    pub quote_lot_size: u64,
    #[clap(long, help="output_file_name", default_value_t = String::from("market.json"))]
    pub output_file_name: String,
}

fn main() {
    let args = CliArgs::parse();

    println!("Creating solana-test-validator sandbox environment");
    let sandbox = Sandbox::new().unwrap();
    let market_creator = Actor::new(&sandbox).unwrap();
    market_creator.airdrop(10000 * LAMPORTS_PER_SOL).unwrap();

    println!("Creating fake tokens for use in Serum market");
    let base_mint = Mint::new(&sandbox, &market_creator, args.base_decimal, None, None).unwrap();
    let quote_mint = Mint::new(&sandbox, &market_creator, args.quote_decimal, None, None).unwrap();

    println!("Deploying serum to the sandbox environment");
    let serum_program = market_creator
        .deploy_remote(&args.dex_url, "serum_dex.so")
        .unwrap();

    println!("Creating new Serum market for testing");
    let market = solarium::serum::Market::new(
        &sandbox,
        &market_creator,
        serum_program.pubkey(),
        &base_mint,
        &quote_mint,
        None,
        args.base_lot_size,
        args.quote_lot_size,
        100,
        128,
        128,
        256,
    )
    .unwrap();

    println!("Creating Serum market participants with large SOL and token balances for trading");
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
            orders: p.open_orders().pubkey().to_string(),
        });
    }

    println!("Writing market.json");
    let data = TestMarket {
        url: sandbox.url(),
        program_id: serum_program.pubkey().to_string(),
        market: market.market().pubkey().to_string(),
        bids: market.bids().pubkey().to_string(),
        asks: market.asks().pubkey().to_string(),
        request_queue: market.request_queue().pubkey().to_string(),
        event_queue: market.event_queue().pubkey().to_string(),
        base_vault: market.base_vault().account().pubkey().to_string(),
        quote_vault: market.quote_vault().account().pubkey().to_string(),
        base_mint: market.base_mint().actor().pubkey().to_string(),
        quote_mint: market.quote_mint().actor().pubkey().to_string(),
        participants: participants.try_into().unwrap(),
    };
    serde_json::to_writer(&fs::File::create(&args.output_file_name).unwrap(), &data).unwrap();
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(args.output_file_name + ".done")
        .unwrap();

    println!("Ready");
    loop {}
}
