import { Account, Connection, PublicKey } from '@solana/web3.js';
import { Market, MARKETS, Orderbook } from '@project-serum/serum';
import * as fs from 'fs';
import BN from 'bn.js';

async function mm(
    connection: Connection, 
    ma: PublicKey, 
    pa : PublicKey, 
    mm_owner: Account,
    payer_base: PublicKey, 
    payer_quote : PublicKey
) {

    // Initial market spread 
    let min_bid = 1;
    let min_ask = 10000;
    let order_size = 100;

    // Checks if there are already orders in the market
    let market = await Market.load(connection, ma, {}, pa);
    let bids_last  = await market.loadBids(connection);
    let asks_last  = await market.loadAsks(connection); 


    // Initial best-bid & best-ask
    let start_best_bid = Number.MIN_SAFE_INTEGER;
    for( let order of bids_last) {
        start_best_bid = Math.max(start_best_bid, order.price);
    }
    let start_best_ask = Number.MAX_SAFE_INTEGER;
    for( let order of asks_last) {
        start_best_ask = Math.min(start_best_ask, order.price);
    }

    start_best_bid = (start_best_bid == Number.MIN_SAFE_INTEGER) ? min_bid : start_best_bid;
    start_best_ask = (start_best_ask == Number.MAX_SAFE_INTEGER) ? min_ask : start_best_ask;

    //place the orders
    let initial_bid = await market.placeOrder(
        connection,
        {
            owner: mm_owner,
            payer: payer_quote,
            side: 'buy',
            price: start_best_bid,
            size: order_size,
            orderType: 'limit',
            feeDiscountPubkey: null,
        }
    );

    let initial_ask = await market.placeOrder(
        connection,
        {
            owner: mm_owner,
            payer: payer_base,
            side: 'sell',
            price: start_best_ask,
            size: order_size,
            orderType: 'limit',
            feeDiscountPubkey: null,
        }
    );

    let mm_bid = start_best_bid;
    let mm_ask = start_best_ask;

    // poll market state
    while(true) {

        await delay(1000);

        let bids_state  = await market.loadBids(connection);
        let asks_state  = await market.loadAsks(connection); 


        let same = checkMarketState (bids_last, bids_state, asks_last, asks_state);
        if(same) continue; //if market hasn't changed

        console.log("market update");

        // check best bid & ask after market update

        let mm_orders = await market.loadOrdersForOwner(connection, mm_owner.publicKey);

        let best_bid = mm_bid;
        for(let bid of bids_state) {
            best_bid = Math.max(bid.price, best_bid)
        }
        let best_ask = mm_ask;
        for(let ask of asks_state) {
            best_ask = Math.min(ask.price, best_ask);
        }

        let updated_bids = false;
        let updated_asks = false;

        // there's a more favorable bid in the market
        if(best_bid > mm_bid) {
            //cancel all existing bids & re-place bid at better price
            for(let order of mm_orders) {
                if(order.side == 'buy') {
                    await market.cancelOrder(connection, mm_owner, order);
                }
            }

            //place new bid
            let up_bid = await market.placeOrder(
                connection,
                {
                    owner: mm_owner,
                    payer: payer_quote,
                    side: 'buy',
                    price: best_bid,
                    size: order_size,
                    orderType: 'limit',
                    feeDiscountPubkey: null,
                }
            );

            mm_bid = best_bid;
            updated_bids = true;

        }

        // there's a more favorable ask in the market
        if(best_ask < mm_ask) {
            //cancel all existing asks & re-place ask at better price
            for(let order of mm_orders) {
                if(order.side == 'sell') {
                    await market.cancelOrder(connection, mm_owner, order);
                }
            }

            //place new ask
            let up_ask = await market.placeOrder(
                connection,
                {
                    owner: mm_owner,
                    payer: payer_base,
                    side: 'sell',
                    price: best_ask,
                    size: order_size,
                    orderType: 'limit',
                    feeDiscountPubkey: null,
                }
            );

            mm_ask = best_ask;
            updated_asks = true;

        }


        // Check for fills & if we need to place partial orders
        let mm_ask_size = 0;
        let mm_bid_size = 0;
        let cur_bid = mm_bid;
        let cur_ask = mm_ask;

        for (let order of mm_orders) {
            if(order.side == 'buy') { mm_bid_size += order.size; cur_bid = order.price; }
            if(order.side == 'sell'){ mm_ask_size += order.size; cur_ask = order.price; }
        }

        console.log("bid : ", cur_bid, mm_bid_size);
        console.log("ask: ", cur_ask, mm_ask_size);

        // fill & we haven't cancelled all bids above
        if(mm_bid_size < order_size && !updated_bids) {
            let up_bid = await market.placeOrder(
                connection,
                {
                    owner: mm_owner,
                    payer: payer_quote,
                    side: 'buy',
                    price: cur_bid,
                    size: order_size - mm_bid_size,
                    orderType: 'limit',
                    feeDiscountPubkey: null,
                }
            );
            mm_bid = cur_bid;
        }

        // fill & we haven't cancelled all asks above
        if(mm_ask_size < order_size && !updated_asks) {
            let up_ask = await market.placeOrder(
                connection,
                {
                    owner: mm_owner,
                    payer: payer_base,
                    side: 'sell',
                    price: cur_ask,
                    size: order_size - mm_ask_size,
                    orderType: 'limit',
                    feeDiscountPubkey: null,
                }
            );
            mm_ask = cur_ask;
        }

        await settle(market, connection, mm_owner, payer_base, payer_quote);

        bids_last = bids_state;
        asks_last = asks_state;
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
}

function printside(side : Orderbook) {
    for (let order of side) {
        console.log(
          order.orderId,
          order.price,
          order.size,
          order.side, // 'buy' or 'sell'
        );
      }    
}

function checkMarketState( 
    bids_before : Orderbook, bids_after : Orderbook, 
    asks_before : Orderbook, asks_after : Orderbook,
) {

    if(bookSize(bids_before) != bookSize(bids_after)) {
        return false;
    }
    
    if(bookSize(asks_before) != bookSize(asks_after)) {
        return false;
    }

    for(let bid_before of bids_before) {
        let matched = false;
        for(let bid_after of bids_after) {
            if(bid_before.orderId.eq(bid_after.orderId) && 
               bid_before.price == bid_after.price      && 
               bid_before.size == bid_after.size        ) {
                matched = true;
                break;
            }
        }
        if(matched != true) return false;
    }

    for(let ask_before of asks_before) {
        let matched = false;
        for(let ask_after of asks_after) {
            if(ask_before.orderId.eq(ask_after.orderId) && 
               ask_before.price == ask_after.price      && 
               ask_before.size == ask_after.size        ) {
                matched = true;
                break;
            }
        }
        if(matched != true) return false;
    }

    return true;
}

function bookSize(book : Orderbook) {
    let sz = 0;
    for(let a of book) {
        sz++;
    }
    return sz;
}

function delay(ms: number) {
    return new Promise( resolve => setTimeout(resolve, ms) );
}

// Market setup && MM init
let text = fs.readFileSync('../mm_keys.txt','utf8');
let tbl = text.split("\n");


let connection = new Connection(tbl[0])
let marketAddress = new PublicKey(tbl[1])
let programAddress = new PublicKey(tbl[2])

let arr = tbl[3].replace("[", "").replace("]", "").split(", ");
let keypairarr = new Uint8Array(arr.length);
for(var i in keypairarr) {
    keypairarr[i] = +arr[i];
}
let participant = new Account(keypairarr)
let payer_quote = new PublicKey(tbl[4])
let payer_base = new PublicKey(tbl[5])

mm(connection, marketAddress, programAddress, participant, payer_base, payer_quote);
