import { decodeEventQueue, Orderbook } from "@project-serum/serum";
import { Order } from "@project-serum/serum/lib/market";
import { Connection, PublicKey, AccountInfo } from "@solana/web3.js";
import BN from "bn.js";
import * as fs from 'fs';
import { MarketMaker } from "./mm_ws";
import { _OPEN_ORDERS_LAYOUT_V2 } from "@project-serum/serum/lib/market";
import {Mutex} from 'async-mutex';

export class OrderState {
    clientID : BN;
    side    : 'buy' | 'sell';
    price   : number;
    size    : number;

    constructor(orderID: BN, side: 'buy' | 'sell', price: number, size: number) {
        this.clientID = orderID;
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
    market_rq    : PublicKey;
    mm_oo        : PublicKey;
    init_pos     : boolean;
    tx_inflight  : boolean;
    mm           : MarketMaker;
    bid_state    : OrderState[];
    ask_state    : OrderState[];
    mm_state     : OrderState[];
    num_fills    : number;

    constructor(mm : MarketMaker) {

        this.mm = mm;
        this.init_pos = false;
        this.num_fills = 0;
        [this.connection, this.market_bids, this.market_asks, this.market_eq, this.market_rq, this.mm_oo] = getEngineKeys();
    }

    async run() {

        if(this.init_pos == false) {
            [this.bid_state, this.ask_state, this.mm_state] = await this.mm.initialize_positions();
            this.init_pos = true;
            this.tx_inflight = true;
        }

        const mutex = new Mutex();

        this.connection.onAccountChange(this.market_bids, async (accountinfo : AccountInfo<Buffer>) => {
            let bids = Orderbook.decode(this.mm.market, accountinfo.data);
            this.update_bids(bids);
            this.refresh_mm(this.bid_state, this.ask_state);

            await mutex.acquire();
            if(this.diff_orderbook()) {
                this.tx_inflight = false;
            }
            if(this.tx_inflight == false) {
                this.tx_inflight = true;
                this.mm_state = await this.mm.onBook(this.bid_state, this.ask_state, this.mm_state);
            }

            mutex.release();

        });

        this.connection.onAccountChange(this.market_asks, async (accountinfo : AccountInfo<Buffer>) => {
            let asks = Orderbook.decode(this.mm.market, accountinfo.data);
            this.update_asks(asks);
            this.refresh_mm(this.bid_state, this.ask_state);


            await mutex.acquire();
            if(this.diff_orderbook()) {
                this.tx_inflight = false;
            }

            if(this.tx_inflight == false) {
                this.tx_inflight = true;
                this.mm_state = await this.mm.onBook(this.bid_state, this.ask_state, this.mm_state);
            }

            mutex.release();

        });
    }

    refresh_mm(bids : OrderState[], asks : OrderState[]) {
        for (let i in this.mm_state) {
            for(let bid of bids) {
                if(this.mm_state[i].clientID.eq(bid.clientID)) { this.mm_state[i].price = bid.price; this.mm_state[i].size = bid.size; }
            }
            for(let bid of asks) {
                if(this.mm_state[i].clientID.eq(bid.clientID)) { this.mm_state[i].price = bid.price; this.mm_state[i].size = bid.size; }
            }
        }
    }

    update_bids(book : Orderbook) {
        this.bid_state = [];
        for(let order of book) {
            this.bid_state.push(new OrderState((order.clientId == undefined || order.clientId.isZero()) ? order.orderId : order.clientId, order.side, order.price, order.size));
        }
    }

    update_asks(book : Orderbook) {
        this.ask_state = [];
        for(let order of book) {
            this.ask_state.push(new OrderState((order.clientId == undefined || order.clientId.isZero()) ? order.orderId : order.clientId, order.side, order.price, order.size));
        }
    }

    diff_orderbook() {
        for(let order of this.mm_state) {
            let found = false;
            if(order.side == 'buy') {
                for(let bid of this.bid_state){
                    if(order.clientID.eq(bid.clientID) && order.price == bid.price && order.size == bid.size) {
                        found = true;
                        break;
                    }
                }
                if(found) continue;
            }
            if(order.side == 'sell') {
                for(let ask of this.ask_state){
                    if(order.clientID.eq(ask.clientID) && order.price == ask.price && order.size == ask.size) {
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
}

function getEngineKeys(): [Connection, PublicKey, PublicKey, PublicKey, PublicKey, PublicKey] {
    let text = fs.readFileSync(__dirname + "/../engine_keys.txt",'utf8');
    let tbl = text.split("\n");

    let connection = new Connection(tbl[0]);
    let market_bids = new PublicKey(tbl[1]);
    let market_asks = new PublicKey(tbl[2]);
    let market_eq = new PublicKey(tbl[3]);
    let market_rq = new PublicKey(tbl[4]);
    let mm_oo = new PublicKey(tbl[5]);


    return [connection, market_bids, market_asks, market_eq, market_rq, mm_oo];
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