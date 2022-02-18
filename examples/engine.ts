import { Orderbook } from "@project-serum/serum";
import { Order } from "@project-serum/serum/lib/market";
import { Connection, PublicKey, AccountInfo } from "@solana/web3.js";
import BN from "bn.js";
import * as fs from 'fs';
import { MarketMaker } from "./mm_ws";

export class OrderState {
    orderID : BN;
    side    : 'buy' | 'sell';
    price   : number;
    size    : number;

    constructor(orderID: BN, side: 'buy' | 'sell', price: number, size: number) {
        this.orderID = orderID;
        this.side = side;
        this.price = price;
        this.size = size;
    }

}

export class Engine {
    connection   : Connection;
    market_bids  : PublicKey;
    market_asks  : PublicKey;
    market_eq    : PublicKey;
    init_pos     : boolean;
    tx_inflight  : boolean;
    mm           : MarketMaker;
    bid_state    : OrderState[];
    ask_state    : OrderState[];
    mm_state     : OrderState[];

    constructor(mm : MarketMaker) {

        this.mm = mm;
        this.init_pos = false;
        [this.connection, this.market_bids, this.market_asks, this.market_eq] = getEngineKeys();
    }

    async run() {

        if(this.init_pos == false) {
            [this.bid_state, this.ask_state, this.mm_state] = await this.mm.initialize_positions();
            this.init_pos = true;
            this.tx_inflight = true;
        }

        this.connection.onAccountChange(this.market_bids, async (accountinfo : AccountInfo<Buffer>) => {
            console.log("\n")
            console.log("callback");
            let bids = Orderbook.decode(this.mm.market, accountinfo.data);
            let asks = Orderbook.decode(this.mm.market, throwIfNull( await this.connection.getAccountInfo(this.market_asks) ).data);
            this.update_bids(bids);
            this.update_asks(asks);

            if(this.diff_orderbook()) {
                this.tx_inflight = false;
            }

            if(this.tx_inflight == false) {
                console.log("bookupdate happens");
                this.mm_state = await this.mm.onBook(this.bid_state, this.ask_state, this.mm_state);
                this.tx_inflight = true;
            }


            

            // console.log("\n");
            

            // console.log(this.diff_orderbook());

            
        });

        // let asks_before = throwIfNull( await this.connection.getAccountInfo(this.random) );
   
        // this.connection.onAccountChange(this.random, async (accountinfo : AccountInfo<Buffer>) => {
        //     printbuf(asks_before.data);
        //     console.log("\n");
        //     printbuf(accountinfo.data);
        //     console.log(Buffer.compare(asks_before.data, accountinfo.data));

        //     console.log("lbefore: ", asks_before.lamports);
        //     console.log("lafter: ", accountinfo.lamports);
        // });
        // this.connection.onAccountChange(this.market_eq, async (accountinfo : AccountInfo<Buffer>) => {await this.mm.onFill()});
    }

    update_bids(book : Orderbook) {
        this.bid_state = [];
        for(let order of book) {
            this.bid_state.push(new OrderState(order.orderId, order.side, order.price, order.size));
        }
    }

    update_asks(book : Orderbook) {
        this.ask_state = [];
        for(let order of book) {
            this.ask_state.push(new OrderState(order.orderId, order.side, order.price, order.size));
        }
    }

    diff_orderbook() {
        console.log("difforderbook")
        this.printstates();

        for(let order of this.mm_state) {
            let found = false;
            if(order.side == 'buy') {
                for(let bid of this.bid_state){
                    if(order.orderID.eq(bid.orderID) && order.price == bid.price && order.size == bid.size) {
                        found = true;
                        break;
                    }
                }
                if(found) continue;
            }
            if(order.side == 'sell') {
                for(let ask of this.ask_state){
                    if(order.orderID.eq(ask.orderID) && order.price == ask.price && order.size == ask.size) {
                        found = true;
                        break;
                    }
                }
                if(found) continue;
            }
            return false;
        }
        return true;
    }

    printstates() {
        console.log("bids: ")
        for(let bid of this.bid_state) {
            console.log(bid.orderID, bid.price, bid.side, bid.size);
        }
        console.log("asks: ")

        for(let bid of this.ask_state) {
            console.log(bid.orderID, bid.price, bid.side, bid.size);
        }

        console.log("mm: ")

        for(let bid of this.mm_state) {
            console.log(bid.orderID, bid.price, bid.side, bid.size);
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
          order.openOrdersSlot,
        );
      }    
}



function getEngineKeys(): [Connection, PublicKey, PublicKey, PublicKey] {
    let text = fs.readFileSync(__dirname + "/../engine_keys.txt",'utf8');
    let tbl = text.split("\n");

    let connection = new Connection(tbl[0]);
    let market_bids = new PublicKey(tbl[1]);
    let market_asks = new PublicKey(tbl[2]);
    let market_eq = new PublicKey(tbl[3]);


    return [connection, market_bids, market_asks, market_eq];
}

export function throwIfNull<T>(value: T | null, message = 'account not found'): T {
    if (value === null) {
      throw new Error(message);
    }
    return value;
  }

export function throwIfUndefined<T>(value: T | undefined, message = 'account not found'): T {
    if (value === undefined) {
      throw new Error(message);
    }
    return value;
  }

function printbuf(buff : Buffer) {
    var str = '';
    for (var ii = 0; ii < buff.length; ii++) {
        str += buff[ii].toString(16) + ' ' ;
    };
    console.log(str);
}