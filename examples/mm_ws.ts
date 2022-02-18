import { Account, Connection, PublicKey, Transaction } from '@solana/web3.js';
import { Market, Orderbook } from '@project-serum/serum';
import * as fs from 'fs';
import BN from 'bn.js';
import { Order } from '@project-serum/serum/lib/market';
import { OrderState, throwIfNull, throwIfUndefined } from './engine';
import { REQUEST_QUEUE_LAYOUT } from '@project-serum/serum';
import { blob, seq, struct, u8, u32 } from 'buffer-layout';
import {
    accountFlagsLayout,
    publicKeyLayout,
    u128,
    u64,
    zeros,
  } from '@project-serum/serum/lib/layout';
import { DexInstructions } from "@project-serum/serum/lib/instructions"

const MIN_BID = 1;
const MIN_ASK = 10000;
const ORDER_SIZE = 100;

const REQUEST_QUEUE_HEADER = struct([
    blob(5),
    accountFlagsLayout('accountFlags'),
    u32('head'),
    zeros(4),
    u32('count'),
    zeros(4),
    u32('nextSeqNum'),
    zeros(4),
  ]);

export interface MarketMaker {
    market : Market;
    initialize_positions(): Promise<[OrderState[], OrderState[], OrderState[]]> ;
    onBook(bids : OrderState[], asks : OrderState[], mm_state : OrderState[]): Promise<OrderState[]>;
    onFill(): void;
    update_seq_num(sqbuf : Buffer);
}

export class SimpleMarketMaker implements MarketMaker {
    connection   : Connection;
    market_addr  : PublicKey;
    program_addr : PublicKey;
    mm_owner     : Account;
    mm_base      : PublicKey;
    mm_quote     : PublicKey;
    request_q    : PublicKey;
    open_orders  : PublicKey;
    market_bids  : PublicKey;
    market_asks  : PublicKey;
    event_q      : PublicKey;
    bid_pos      : number;
    ask_pos      : number;
    seq_num      : number;
    public market : Market;
    init_ct      : number; // fixes strange interaction

    constructor (market_config : string) {

        [this.connection, this.market_addr, this.program_addr, this.request_q, this.open_orders, this.market_bids, this.market_asks, this.event_q, this.mm_owner, this.mm_quote, this.mm_base] = getMarketKeys(market_config)
        this.bid_pos = 0;
        this.ask_pos = 0;
        this.init_ct = 0;
    }

    async placeOrder(clientID: BN, side : string, price : number, qty : number) {
        if(side != "buy" && side != "sell") throw new TypeError("Invalid Side");

        let market = this.market;
        
        if(side == 'buy') {
            return await market.placeOrder(
                this.connection,
                {
                    owner: this.mm_owner,
                    payer: this.mm_quote,
                    clientId: clientID,
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
                    clientId: clientID,
                    side: 'sell',
                    price: price,
                    size: qty,
                    orderType: 'limit',
                    feeDiscountPubkey: null,
                }
            );
        }  
    }

    async cancelOrder(orderId : BN, side: 'buy' | 'sell') {
        let instr = DexInstructions.cancelOrderByClientIdV2({
            market: this.market_addr,
            bids: this.market_bids,
            asks: this.market_asks,
            eventQueue: this.event_q,
            openOrders: this.open_orders,
            owner: this.mm_owner.publicKey,
            clientId: orderId,
            programId: this.market.programId,
        });
        const transaction = new Transaction();
        transaction.add(instr);
        this.sendTxn(transaction, [this.mm_owner]);
    }

    async sendTxn(txn : Transaction, signers: Array<Account>) {
        const signature = await this.connection.sendTransaction(txn, signers, {
            skipPreflight: true,
          });
          const { value } = await this.connection.confirmTransaction(
            signature,
          );
          if (value?.err) {
            throw new Error(JSON.stringify(value.err));
          }
          return signature;
    }

    async initialize_positions(): Promise<[OrderState[], OrderState[], OrderState[]]> {
        console.log("init_positions");
       
        let market = await Market.load(this.connection, this.market_addr, {}, this.program_addr);
        this.market = market;
        let bids = await market.loadBids(this.connection);
        let asks = await market.loadAsks(this.connection);

        let req_q_data = throwIfNull( await this.connection.getAccountInfo(this.request_q));
        this.seq_num = REQUEST_QUEUE_HEADER.decode(req_q_data.data).nextSeqNum;

        let mm_orders: OrderState[] = [];
        let bid_state: OrderState[] = [];
        let ask_state: OrderState[] = [];

        let cur_mm_bb = Number.MIN_SAFE_INTEGER;
        let cur_mm_ba = Number.MAX_SAFE_INTEGER;

        let book_bb = Number.MIN_SAFE_INTEGER;
        let book_ba = Number.MAX_SAFE_INTEGER;

        for(let bid of bids) {
            if(bid.openOrdersAddress.equals(this.open_orders)) {
                cur_mm_bb = Math.max(cur_mm_bb, bid.price);
                mm_orders.push(new OrderState(throwIfUndefined (bid.clientId), bid.side, bid.price, bid.size));
            }
            bid_state.push(new OrderState(bid.orderId, bid.side, bid.price, bid.size));
            book_bb = Math.max(book_bb, bid.price)
        }
        for(let ask of asks) {
            if(ask.openOrdersAddress.equals(this.open_orders)) { 
                cur_mm_ba = Math.min(cur_mm_ba, ask.price);
                mm_orders.push(new OrderState(throwIfUndefined (ask.clientId), ask.side, ask.price, ask.size));
            }
            ask_state.push(new OrderState(ask.orderId, ask.side, ask.price, ask.size));
            book_ba = Math.min(book_ba, ask.price);
        }

        book_bb = (book_bb == Number.MIN_SAFE_INTEGER) ? MIN_BID : book_bb;
        book_ba = (book_ba == Number.MAX_SAFE_INTEGER) ? MIN_ASK : book_ba;

        if(cur_mm_bb == Number.MIN_SAFE_INTEGER) {
            let clientID = this.gen_client_id(book_bb, 'buy');
            let bid_placed = await this.placeOrder(clientID, "buy", book_bb, 100);
            mm_orders.push(new OrderState(clientID, 'buy', book_bb, 100));
            this.seq_num++;
        }
        this.bid_pos = book_bb;

        if(cur_mm_ba == Number.MAX_SAFE_INTEGER) {
            let clientID = this.gen_client_id(book_ba, 'sell');
            let ask_placed = await this.placeOrder(clientID, "sell", book_ba, 100);
            mm_orders.push(new OrderState(clientID, 'sell', book_ba, 100));
            this.seq_num++;
        }
        this.ask_pos = book_ba;
       
        return [bid_state, ask_state, mm_orders];
    }

    async onBook(bids : OrderState[], asks : OrderState[], mm_orders : OrderState[]) : Promise<OrderState[]> {
        console.log("BookUpdate!");

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

        let new_order_state: OrderState[] = [];

        // there's a more favorable bid in the market
        if(best_bid > this.bid_pos) {
            //cancel all existing bids & re-place bid at better price
            for(let order of mm_orders) {
                if(order.side == 'buy') {
                    await this.cancelOrder(order.clientID, order.side);
                } else if (best_ask >= this.ask_pos) new_order_state.push(order);
            }

            //place the new bid & reflect that in the intended market state
            let clientID = this.gen_client_id(best_bid, 'buy');
            await this.placeOrder(clientID, 'buy', best_bid, 100);
            
            new_order_state.push(new OrderState(clientID, 'buy', best_bid, 100));
            this.seq_num++;
            this.bid_pos = best_bid;
            updated_bids = true;
        }
        // there's a more favorable ask in the market
        if(best_ask < this.ask_pos) {
            //cancel all existing asks & re-place ask at better price
            for(let order of mm_orders) {
                if(order.side == 'sell') {
                    await this.cancelOrder(order.clientID, order.side);
                } else if (best_bid <= this.bid_pos) new_order_state.push(order);
            }
            let clientID = this.gen_client_id(best_ask, 'sell');
            await this.placeOrder(clientID, 'sell', best_ask, 100);
            new_order_state.push(new OrderState(clientID, 'sell', best_ask, 100));
            this.seq_num++;
            this.ask_pos = best_ask;
            updated_asks = true;
        }

        if(updated_asks && updated_bids) return new_order_state;

        // Check for fills & if we need to place partial orders
        let mm_ask_size = 0;
        let mm_bid_size = 0;
        let cur_bid = this.bid_pos;
        let cur_ask = this.ask_pos;

        for (let order of mm_orders) {
            if(order.side == 'buy') { 
                mm_bid_size += order.size; cur_bid = order.price;
            }
            if(order.side == 'sell'){ 
                mm_ask_size += order.size; cur_ask = order.price;
            }
        }

        let fill_flag = false;

        // fill & we haven't cancelled all bids above
        if(mm_bid_size < ORDER_SIZE && !updated_bids) {
            let clientID = this.gen_client_id(cur_bid, 'buy');
            let up_bid = await(this.placeOrder(clientID, 'buy', cur_bid, ORDER_SIZE - mm_bid_size));
            new_order_state.push(new OrderState(clientID, 'buy', cur_bid, ORDER_SIZE - mm_bid_size));
            this.bid_pos = cur_bid;
            fill_flag = true;
        }

        // fill & we haven't cancelled all asks above
        if(mm_ask_size < ORDER_SIZE && !updated_asks) {
            let clientID = this.gen_client_id(cur_ask, 'sell');
            let up_ask = await this.placeOrder(clientID, 'sell', cur_ask, ORDER_SIZE - mm_ask_size);
            new_order_state.push(new OrderState(clientID, 'sell', cur_ask, ORDER_SIZE - mm_ask_size));
            this.ask_pos = cur_ask;
            fill_flag = true;
        }

        if(fill_flag) {
            for(let order of mm_orders) {
                new_order_state.push(order)
            }
        }

        if(!(mm_bid_size < ORDER_SIZE && !updated_bids) && !(mm_ask_size < ORDER_SIZE && !updated_asks) && !(updated_bids || updated_asks)) {
            return mm_orders;
        }

        return new_order_state;



    //     await this.settle();
    //     this.init_ct ++;
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

    async update_seq_num(sqbuf : Buffer) {
        this.seq_num = REQUEST_QUEUE_HEADER.decode(sqbuf).nextSeqNum;
    }

    gen_client_id(price : number, side: 'buy' | 'sell') {
        let upper = new BN(price);
        upper.iushln(32);
        let lower = new BN(this.seq_num);
        if(side == 'buy') lower.inotn(32);
        this.seq_num++;
        return upper.uor(lower);
    }
}

function getMarketKeys(market_config : string) : [Connection, PublicKey, PublicKey, PublicKey, PublicKey, PublicKey, PublicKey, PublicKey, Account, PublicKey, PublicKey] {
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
    
    let request_queue = new PublicKey(tbl[9])
    let open_orders = new PublicKey(tbl[10]);

    let bids = new PublicKey(tbl[11]);
    let asks = new PublicKey(tbl[12]);
    let event_q = new PublicKey(tbl[13]);


    return [connection, marketAddress, programAddress, request_queue, open_orders, bids, asks, event_q, participant, payer_quote, payer_base];
}
