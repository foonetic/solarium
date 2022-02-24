/**
 * Demo program for buying, selling, and settling serum orders.
 *
 * This demo is meant to be used in conjunction with the create_serum_market
 * rust binary and the serum-market-maker example.
 * 
 * See the comments in serum-market-maker.
 */
import { Account, Keypair, Connection, PublicKey } from "@solana/web3.js";
import { ArgumentParser } from "argparse";
import { Market } from "@project-serum/serum";
import * as fs from "fs";
import BN from "bn.js";
import * as bs58 from "bs58";

const parser = new ArgumentParser();
parser.add_argument("--market-config", { default: "market.json" });
parser.add_argument("--participant", { default: 1, type: "int" });
parser.add_argument("--price", { default: 150.0, type: "float" });
parser.add_argument("--size", { default: 1, type: "int" });
parser.add_argument("--side", {
  default: "settle",
  type: "str",
  choices: ["buy", "sell", "settle"],
});
const args = parser.parse_args();

const marketConfig = JSON.parse(fs.readFileSync(args.market_config, "utf8"));
const participant = marketConfig.participants[args.participant];
const connection = new Connection(marketConfig.url, "confirmed");
const programId = new PublicKey(marketConfig.program_id);
const marketAddress = new PublicKey(marketConfig.market);
const keypair = Keypair.fromSecretKey(bs58.decode(participant.keypair));
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
    for (let openOrders of await market.findOpenOrdersAccountsForOwner(
      connection,
      participant.publicKey
    )) {
      if (
        openOrders.baseTokenFree > new BN(0) ||
        openOrders.quoteTokenFree > new BN(0)
      ) {
        await market.settleFunds(
          connection,
          new Account(participant.secretKey),
          openOrders,
          base,
          quote
        );
      }
    }
    return;
  } else if (side == "buy" || side == "sell") {
    let payer = side == "buy" ? quote : base;
    await market.placeOrder(connection, {
      owner: new Account(participant.secretKey),
      payer: payer,
      side: side as "buy" | "sell",
      price: price,
      size: size,
      orderType: "limit",
      feeDiscountPubkey: null,
    });
  }
}
