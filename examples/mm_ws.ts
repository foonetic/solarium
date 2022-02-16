import { Account, Connection, PublicKey } from '@solana/web3.js';
import { Market, Orderbook } from '@project-serum/serum';
import * as fs from 'fs';
import BN from 'bn.js';

const MIN_BID = 1;
const MIN_ASK = 10000;
const ORDER_SIZE = 100;

export interface MarketMaker {
    market : Market;
    initialize_positions(): void;
    onBook(bids : Orderbook, asks : Orderbook): void;
    onFill(): void;
}

export class SimpleMarketMaker implements MarketMaker {
    connection   : Connection;
    market_addr  : PublicKey;
    program_addr : PublicKey;
    mm_owner     : Account;
    mm_base      : PublicKey;
    mm_quote     : PublicKey;
    bid_pos      : number;
    ask_pos      : number;
    public market : Market;
    init_ct      : number; // fixes strange interaction

    constructor (market_config : string) {

        [this.connection, this.market_addr, this.program_addr, this.mm_owner, this.mm_quote, this.mm_base]   = getMarketKeys(market_config)
        this.bid_pos = 0;
        this.ask_pos = 0;
        this.init_ct = 0;
    }

    async placeOrder(side : string, price : number, qty : number) {
        if(side != "buy" && side != "sell") throw new TypeError("Invalid Side");

        let market = this.market;
        
        if(side == 'buy') {
            return await market.placeOrder(
                this.connection,
                {
                    owner: this.mm_owner,
                    payer: this.mm_quote,
                    side: 'buy',
                    price: price,
                    size: qty,
                    orderType: 'limit',
                    feeDiscountPubkey: null,
                }
            );
        } else {
            return await market.placeOrder(
                this.connection,
                {
                    owner: this.mm_owner,
                    payer: this.mm_base,
                    side: 'sell',
                    price: price,
                    size: qty,
                    orderType: 'limit',
                    feeDiscountPubkey: null,
                }
            );
        }  
    }

    async initialize_positions() {
       
        let market = await Market.load(this.connection, this.market_addr, {}, this.program_addr);
        this.market = market;
        let mm_orders = await market.loadOrdersForOwner(this.connection, this.mm_owner.publicKey);

        let cur_mm_bb = Number.MIN_SAFE_INTEGER;
        let cur_mm_ba = Number.MAX_SAFE_INTEGER;

        for(let order of mm_orders) {
            if(order.side == 'buy') cur_mm_bb = Math.max(cur_mm_bb, order.price);
            else cur_mm_ba = Math.min(cur_mm_ba, order.price);
        }

        // If the MM has no bids out
        if(cur_mm_bb == Number.MIN_SAFE_INTEGER) {
            let bids  = await market.loadBids(this.connection);
            let start_best_bid = Number.MIN_SAFE_INTEGER;
            for( let order of bids) {
                start_best_bid = Math.max(start_best_bid, order.price);
            }

            start_best_bid = (start_best_bid == Number.MIN_SAFE_INTEGER) ? MIN_BID : start_best_bid;
            let bid_placed = await this.placeOrder("buy", start_best_bid, 100);
            this.bid_pos = start_best_bid;
        } else {
            // Init values to existing bid
            this.bid_pos = cur_mm_bb;
        }

        // If the MM has no asks out
        if(cur_mm_ba == Number.MAX_SAFE_INTEGER) {
            let asks  = await market.loadAsks(this.connection); 
            let start_best_ask = Number.MAX_SAFE_INTEGER;
            for( let order of asks) {
                start_best_ask = Math.min(start_best_ask, order.price);
            }
    
            start_best_ask = (start_best_ask == Number.MAX_SAFE_INTEGER) ? MIN_ASK : start_best_ask;
            let ask_placed = await this.placeOrder("sell", start_best_ask, 100);
            this.ask_pos = start_best_ask;
        } else {
            // Init values to existing ask
            this.ask_pos = cur_mm_ba;
        }
    }

    async onBook(bids : Orderbook, asks : Orderbook) {
        console.log("BookUpdate!");
        let market = this.market;
        let mm_orders = await market.loadOrdersForOwner(this.connection, this.mm_owner.publicKey);
        
        console.log("bids: ");
        printside(bids);

        console.log("asks: ");
        printside(asks);
        

        let best_bid = this.bid_pos;
        for(let bid of bids) {
            best_bid = Math.max(bid.price, best_bid)
        }
        let best_ask = this.ask_pos;
        for(let ask of asks) {
            best_ask = Math.min(ask.price, best_ask);
        }

        let updated_bids = false;
        let updated_asks = false;

        console.log("bb check: ", best_bid);
        console.log("ba check: ", best_ask);

        console.log(best_bid > this.bid_pos);
        console.log(best_ask < this.ask_pos);

        // there's a more favorable bid in the market
        if(best_bid > this.bid_pos) {
            //cancel all existing bids & re-place bid at better price
            for(let order of mm_orders) {
                if(order.side == 'buy') {
                    await market.cancelOrder(this.connection, this.mm_owner, order);
                }
            }
            updated_bids = true;
        }

        // there's a more favorable ask in the market
        if(best_ask < this.ask_pos) {
            //cancel all existing asks & re-place ask at better price
            for(let order of mm_orders) {
                if(order.side == 'sell') {
                    await market.cancelOrder(this.connection, this.mm_owner, order);
                }
            }
            updated_asks = true;
        }

        // Check for fills & if we need to place partial orders
        let mm_ask_size = 0;
        let mm_bid_size = 0;
        let cur_bid = this.bid_pos;
        let cur_ask = this.ask_pos;

        for (let order of mm_orders) {
            if(order.side == 'buy') { mm_bid_size += order.size; cur_bid = order.price; }
            if(order.side == 'sell'){ mm_ask_size += order.size; cur_ask = order.price; }
        }

        // fill & we haven't cancelled all bids above
        cur_bid = (updated_bids) ? best_bid : cur_bid;
        if(mm_bid_size < ORDER_SIZE && this.init_ct >= 2) {
            let up_bid = await(this.placeOrder('buy', cur_bid, ORDER_SIZE - mm_bid_size));
            this.bid_pos = cur_bid;
        }

        // fill & we haven't cancelled all asks above
        cur_ask = (updated_asks) ? best_ask : cur_ask;
        if(mm_ask_size < ORDER_SIZE && this.init_ct >= 2) {
            let up_ask = await this.placeOrder('sell', cur_ask, ORDER_SIZE - mm_ask_size);
            this.ask_pos = cur_ask;
        }

        await this.settle();
        this.init_ct ++;
    }

    async onFill() {
        // onFill not implemented in this example
    }

    async settle() {
        let market = this.market;

        for (let openOrders of await market.findOpenOrdersAccountsForOwner(
            this.connection,
            this.mm_owner.publicKey,
          )) {
            if (openOrders.baseTokenFree > new BN(0) || openOrders.quoteTokenFree > new BN(0)) {
              // spl-token accounts to which to send the proceeds from trades      
              await market.settleFunds(
                this.connection,
                this.mm_owner,
                openOrders,
                this.mm_base,
                this.mm_quote,
              );
            }
        }
    }

}

function getMarketKeys(market_config : string) : [Connection, PublicKey, PublicKey, Account, PublicKey, PublicKey] {
    // Parses a file for market keys
    let text = fs.readFileSync(market_config,'utf8');
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

    return [connection, marketAddress, programAddress, participant, payer_quote, payer_base];
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
