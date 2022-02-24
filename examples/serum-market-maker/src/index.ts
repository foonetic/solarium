import { SimpleMarketMaker } from "./market_maker";
import { ArgumentParser } from "argparse";
import { Keypair, Connection, PublicKey } from "@solana/web3.js";

import * as bs58 from "bs58";
import * as fs from "fs";

async function main() {
  const parser = new ArgumentParser();
  parser.add_argument("--market-config", { default: "market.json" });
  parser.add_argument("--participant", { default: 0, type: "int" });
  const args = parser.parse_args();

  // This configuration is generated by the create_serum_market binary.
  const marketConfig = JSON.parse(fs.readFileSync(args.market_config, "utf8"));

  // See the following link for details on commitment levels:
  // https://docs.solana.com/developing/clients/jsonrpc-api#configuring-state-commitment
  const connection = new Connection(marketConfig.url, "confirmed");

  // This is the Serum program ID. In a production setting you would use a fixed
  // ID. In this test, we deploy a local copy of Serum to the test validator so
  // the ID is an input to all downstream Serum clients.
  const programId = new PublicKey(marketConfig.program_id);

  // Market address, bids, asks, and event queue are accounts used by a specific
  // Serum market. In production, these are fixed addresses for specific token
  // pairs. In this test, we use a dummy market created by create_serum_market.
  const marketAddress = new PublicKey(marketConfig.market);
  const bids = new PublicKey(marketConfig.bids);
  const asks = new PublicKey(marketConfig.asks);
  const eventQueue = new PublicKey(marketConfig.event_queue);

  const marketMaker = marketConfig.participants[args.participant];

  // The market maker signs for its transactions using its own keypair. The
  // market maker must have an initialized open orders account (see
  // create_serum_market for details on how this can be initialized) as well as
  // token accounts corresponding to the base and quote token of the market it
  // will trade on.
  //
  // In production, the base and quote token accounts would be for actual tokens
  // being traded (say BTC and USDC if trading on the BTC-USDC Serum market). In
  // this test, the token accounts correspond to dummy tokens created by
  // create_serum_market.
  const marketMakerKey = Keypair.fromSecretKey(
    bs58.decode(marketMaker.keypair)
  );
  const openOrders = new PublicKey(marketMaker.orders);
  const base = new PublicKey(marketMaker.base);
  const quote = new PublicKey(marketMaker.quote);

  let mm = new SimpleMarketMaker(
    connection,
    programId,
    marketAddress,
    bids,
    asks,
    eventQueue,
    marketMakerKey,
    openOrders,
    quote,
    base
  );
  mm.run();
}

main();
