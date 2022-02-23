import { SimpleMarketMaker } from "./market_maker";
import { ArgumentParser } from "argparse";
import { Keypair, Connection, PublicKey, Transaction } from "@solana/web3.js";

import * as bs58 from "bs58";
import * as fs from "fs";

const parser = new ArgumentParser();
parser.add_argument("--market-config", { default: "market.json" });
const args = parser.parse_args();
const marketConfig = JSON.parse(fs.readFileSync(args.market_config, "utf8"));
const connection = new Connection(marketConfig.url, "confirmed");
const programId = new PublicKey(marketConfig.program_id);
const marketAddress = new PublicKey(marketConfig.market);
const bids = new PublicKey(marketConfig.bids);
const asks = new PublicKey(marketConfig.asks);
const eventQueue = new PublicKey(marketConfig.event_queue);

const marketMaker = marketConfig.participants[0];
const marketMakerKey = Keypair.fromSecretKey(bs58.decode(marketMaker.keypair));
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
