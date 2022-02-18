import { Account, Connection, PublicKey } from '@solana/web3.js';
import { Market, MARKETS } from '@project-serum/serum';
import * as fs from 'fs';

let text = fs.readFileSync('../mm_keys.txt','utf8');
let tbl = text.split("\n");

let connection = new Connection(tbl[0])
let marketAddress = new PublicKey(tbl[1])
let programAddress = new PublicKey(tbl[2])

market_state(connection, marketAddress, programAddress);

async function market_state(
    connection: Connection, 
    ma: PublicKey, 
    pa : PublicKey, 
) {

    let market = await Market.load(connection, ma, {}, pa);
    let bids = await market.loadBids(connection);
    let asks = await market.loadAsks(connection);

    console.log("Bids: ");
    for (let order of bids) {
        console.log(order.size + "@" + order.price + "\t",    
        "(OrderID: " , order.orderId,
        "Size: ", order.size,
        "Price: " ,order.price + ")");
      }

      console.log("\nAsks: ");
      for (let order of asks) {
        console.log(order.size + "@" + order.price + "\t",    
        "(OrderID: " , order.orderId,
        "Size: ", order.size,
        "Price: " ,order.price + ")");
      }
}
