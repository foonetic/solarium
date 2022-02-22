import { Orderbook } from "@project-serum/serum";
import { Connection, PublicKey, AccountInfo } from "@solana/web3.js";
import BN from "bn.js";
import * as fs from 'fs';
import { MarketMaker, EVENT_QUEUE_HEADER } from "./mm_ws";
import {Mutex} from 'async-mutex';
import { decodeEventsSince } from "@project-serum/serum/lib/queue";

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
    init_pos     : boolean;
    mm           : MarketMaker;
    bid_state    : OrderState[];
    ask_state    : OrderState[];
    mm_state     : OrderState[];
    eq_seq_num   : number;

    constructor(mm : MarketMaker) {
        this.mm = mm;
        this.init_pos = false;
        this.eq_seq_num = 0;
        [this.connection, this.market_bids, this.market_asks, this.market_eq] = getEngineKeys();
    }

    async run() {

        if(this.init_pos == false) {
            [[this.bid_state, this.ask_state, this.mm_state], this.eq_seq_num] = await this.mm.initialize_positions();
            this.init_pos = true;
        }

        const mutex = new Mutex();

        this.connection.onAccountChange(this.market_bids, async (accountinfo : AccountInfo<Buffer>) => {
            
            await mutex.acquire();

            let bids = Orderbook.decode(this.mm.market, accountinfo.data);
            let bid_callback = this.update_bids(bids);
      
            [this.bid_state, this.ask_state, this.mm_state] = await this.mm.onBook(bid_callback, this.ask_state, this.mm_state);

            mutex.release();

        });

        this.connection.onAccountChange(this.market_asks, async (accountinfo : AccountInfo<Buffer>) => {
 
            await mutex.acquire();

            let asks = Orderbook.decode(this.mm.market, accountinfo.data);
            let ask_callback = this.update_asks(asks);

            [this.bid_state, this.ask_state, this.mm_state] = await this.mm.onBook(this.bid_state, ask_callback, this.mm_state);

            mutex.release();

        });

        this.connection.onAccountChange(this.market_eq, async (accountinfo : AccountInfo<Buffer>) => {
           
            await mutex.acquire();
            let eq: any[] = decodeEventsSince(accountinfo.data, this.eq_seq_num).filter((event) => event.eventFlags.fill);
            this.eq_seq_num = EVENT_QUEUE_HEADER.decode(accountinfo.data).seqNum;
            this.refresh_mm(eq);

            if(eq.length != 0) [this.bid_state, this.ask_state, this.mm_state] = await this.mm.onFill(this.bid_state, this.ask_state, this.mm_state);

            mutex.release();

        });
    }

    refresh_mm(events : any[]) {
       for (let event of events) {
           for(let i = 0; i < this.mm_state.length;) {
               if(this.mm_state[i].clientID.eq(event.clientOrderId)) {
                   this.mm_state[i].size -= event.nativeQuantityReleased;
               }
               if(this.mm_state[i].size == 0) {
                   this.mm_state.splice(i, 1);
                   continue;
               }
               i++;
           }
       }
    }

    update_bids(book : Orderbook) : OrderState[] {
        let bid: OrderState[] = [];
        for(let order of book) {
            bid.push(new OrderState((order.clientId == undefined || order.clientId.isZero()) ? order.orderId : order.clientId, order.side, order.price, order.size));
        }
        return bid;
    }

    update_asks(book : Orderbook): OrderState [] {
        let ask: OrderState[] = [];
        for(let order of book) {
            ask.push(new OrderState((order.clientId == undefined || order.clientId.isZero()) ? order.orderId : order.clientId, order.side, order.price, order.size));
        }
        ask.reverse();
        return ask;
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

