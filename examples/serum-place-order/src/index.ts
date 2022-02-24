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

async function main() {
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
  const market = await Market.load(connection, marketAddress, {}, programId);

  if (args.side == "settle") {
    for (const openOrders of await market.findOpenOrdersAccountsForOwner(
      connection,
      keypair.publicKey
    )) {
      if (
        openOrders.baseTokenFree > new BN(0) ||
        openOrders.quoteTokenFree > new BN(0)
      ) {
        await market.settleFunds(
          connection,
          new Account(keypair.secretKey),
          openOrders,
          base,
          quote
        );
      }
    }
  } else if (args.side == "buy" || args.side == "sell") {
    const payer = args.side == "buy" ? quote : base;
    await market.placeOrder(connection, {
      owner: new Account(keypair.secretKey),
      payer: payer,
      side: args.side as "buy" | "sell",
      price: args.price,
      size: args.size,
      orderType: "limit",
      feeDiscountPubkey: null,
    });
  }
}

main();
