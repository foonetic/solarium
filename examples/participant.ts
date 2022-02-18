import { Account, Connection, PublicKey } from '@solana/web3.js';
import { Market, MARKETS } from '@project-serum/serum';
import * as fs from 'fs';
import BN from 'bn.js';

let text = fs.readFileSync(__dirname + "/../mm_keys.txt",'utf8');
let tbl = text.split("\n");
let connection = new Connection(tbl[0])
let marketAddress = new PublicKey(tbl[1])
let programAddress = new PublicKey(tbl[2])

let arr = tbl[6].replace("[", "").replace("]", "").split(", ");
let keypairarr = new Uint8Array(arr.length);
for(var i in keypairarr) {
    keypairarr[i] = +arr[i];
}
let participant = new Account(keypairarr)
let payer_quote = new PublicKey(tbl[7])
let payer_base = new PublicKey(tbl[8])

let side : String = process.argv[2];
let order = (process.argv[3] != null) ?process.argv[3].split("@") : "";

execute(connection, marketAddress, programAddress, participant, payer_base, payer_quote, +order[1], +order[0], side);

async function execute(
    connection: Connection, 
    ma: PublicKey, 
    pa : PublicKey, 
    part: Account,
    payer_base: PublicKey, 
    payer_quote : PublicKey,
    price : number,
    size : number,
    side : String,
) {

    let market = await Market.load(connection, ma, {}, pa);

    if(side == 'settle') {
        await settle(market, connection, part, payer_base, payer_quote);
        return;
    } 

    if(side != 'buy' && side != 'sell') throw new TypeError("Invalid Side");

    if(side == 'buy') {
        await market.placeOrder(
              connection,
              {
                  owner: part,
                  payer: payer_quote,
                  side: 'buy',
                  price: price,
                  size: size,
                  orderType: 'limit',
                  feeDiscountPubkey: null,
              }
          );
    }

    if(side == 'sell') {
        await market.placeOrder(
              connection,
              {
                  owner: part,
                  payer: payer_base,
                  side: 'sell',
                  price: price,
                  size: size,
                  orderType: 'limit',
                  feeDiscountPubkey: null,
              }
          );
    }
}

async function settle(market : Market, conn : Connection, owner : Account, base : PublicKey, quote : PublicKey) {
    for (let openOrders of await market.findOpenOrdersAccountsForOwner(
        conn,
        owner.publicKey,
      )) {
        if (openOrders.baseTokenFree > new BN(0) || openOrders.quoteTokenFree > new BN(0)) {
          // spl-token accounts to which to send the proceeds from trades      
          await market.settleFunds(
            conn,
            owner,
            openOrders,
            base,
            quote,
          );
        }
    }
    console.log("Settled funds");
}
