import { Account, Keypair, Connection, PublicKey } from "@solana/web3.js";
import { ArgumentParser } from "argparse";
import { Market, MARKETS } from "@project-serum/serum";
import * as fs from "fs";
import BN from "bn.js";
import * as bs58 from "bs58";

const parser = new ArgumentParser();
parser.add_argument("--market-config", { default: "market.json" });
parser.add_argument("--participant", { default: 1, type: "int" });
parser.add_argument("--price", { default: 150.0, type: "float" });
parser.add_argument("--size", { default: 1, type: "int" });
parser.add_argument("--side", { default: "settle", type: "str" });
const args = parser.parse_args();
const marketConfig = JSON.parse(fs.readFileSync(args.market_config, "utf8"));
const participant = marketConfig.participants[args.participant];
const connection = new Connection(marketConfig.url);
const programId = new PublicKey(marketConfig.program_id);
const marketAddress = new PublicKey(marketConfig.market);

const bids = new PublicKey(marketConfig.bids);
const asks = new PublicKey(marketConfig.asks);
const eventQueue = new PublicKey(marketConfig.event_queue);

const keypair = Keypair.fromSecretKey(bs58.decode(participant.keypair));
const openOrders = new PublicKey(participant.orders);
const base = new PublicKey(participant.base);
const quote = new PublicKey(participant.quote);

execute(
  connection,
  marketAddress,
  programId,
  keypair,
  base,
  quote,
  args.price,
  args.size,
  args.side
);

async function execute(
  connection: Connection,
  marketAddress: PublicKey,
  programId: PublicKey,
  participant: Keypair,
  base: PublicKey,
  quote: PublicKey,
  price: number,
  size: number,
  side: String
) {
  let market = await Market.load(connection, marketAddress, {}, programId);

  if (side == "settle") {
    await settle(
      market,
      connection,
      new Account(participant.secretKey),
      base,
      quote
    );
    return;
  }

  if (side != "buy" && side != "sell") throw new TypeError("Invalid Side");

  if (side == "buy") {
    await market.placeOrder(connection, {
      owner: new Account(participant.secretKey),
      payer: quote,
      side: "buy",
      price: price,
      size: size,
      orderType: "limit",
      feeDiscountPubkey: null,
    });
  }

  if (side == "sell") {
    await market.placeOrder(connection, {
      owner: new Account(participant.secretKey),
      payer: base,
      side: "sell",
      price: price,
      size: size,
      orderType: "limit",
      feeDiscountPubkey: null,
    });
  }
}

async function settle(
  market: Market,
  conn: Connection,
  owner: Account,
  base: PublicKey,
  quote: PublicKey
) {
  for (let openOrders of await market.findOpenOrdersAccountsForOwner(
    conn,
    owner.publicKey
  )) {
    if (
      openOrders.baseTokenFree > new BN(0) ||
      openOrders.quoteTokenFree > new BN(0)
    ) {
      // spl-token accounts to which to send the proceeds from trades
      await market.settleFunds(conn, owner, openOrders, base, quote);
    }
  }
  console.log("Settled funds");
}
