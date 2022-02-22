import { Account, Connection, PublicKey, Transaction } from '@solana/web3.js';
import { Market } from '@project-serum/serum';
import * as fs from 'fs';
import BN from 'bn.js';
import { OrderState, throwIfNull, throwIfUndefined } from './engine';
import { blob, struct, u32 } from 'buffer-layout';
import {
    accountFlagsLayout,
    zeros,
  } from '@project-serum/serum/lib/layout';
import { DexInstructions } from "@project-serum/serum/lib/instructions"

const MIN_BID = 1;
const MIN_ASK = 10000;
const ORDER_SIZE = 100;

export const EVENT_QUEUE_HEADER = struct([
    blob(5),
  
    accountFlagsLayout('accountFlags'),
    u32('head'),
    zeros(4),
    u32('count'),
    zeros(4),
    u32('seqNum'),
    zeros(4),
  ]);

export interface MarketMaker {
    market : Market;
    initialize_positions(): Promise<[number, number, OrderState[], OrderState[], number]> ;
    onBook(est_bid : number, best_ask : number, mm_bids : OrderState[], mm_asks : OrderState[]): Promise<[number, number, OrderState[], OrderState[]]>;
    onFill(bid_qty_refill : number, ask_qty_refill : number, mm_bids : OrderState[], mm_asks : OrderState[]): Promise<[OrderState[], OrderState[]]>;
}

export class SimpleMarketMaker implements MarketMaker {
    connection   : Connection;
    market_addr  : PublicKey;
    program_addr : PublicKey;
    mm_owner     : Account;
    mm_base      : PublicKey;
    mm_quote     : PublicKey;
    open_orders  : PublicKey;
    market_bids  : PublicKey;
    market_asks  : PublicKey;
    event_q      : PublicKey;
    bid_pos      : number;
    ask_pos      : number;
    seq_num      : number;
    public market : Market;

    constructor (market_config : string) {

        [this.connection, this.market_addr, this.program_addr, this.open_orders, this.market_bids, this.market_asks, this.event_q, this.mm_owner, this.mm_quote, this.mm_base] = getMarketKeys(market_config)
        this.bid_pos = 0;
        this.ask_pos = 0;
        this.seq_num = 10;
    }

    async placeOrder(clientID: BN, side : 'buy' | 'sell', price : number, qty : number) {

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

    async cancelOrder(orderId : BN) {
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
        try{
            const signature = await this.connection.sendTransaction(txn, signers, {
                skipPreflight: true,
            });
       
            const { value } = await this.connection.confirmTransaction(
                signature,
            );
            return signature;
          } catch {} 
    }

    async initialize_positions(): Promise<[number, number, OrderState[],  OrderState[], number]> {
        console.log("init_positions");
       
        let market = await Market.load(this.connection, this.market_addr, {}, this.program_addr);
        this.market = market;
        let bids = await market.loadBids(this.connection);
        let asks = await market.loadAsks(this.connection);
        let eq = await this.connection.getAccountInfo(this.event_q);
        let eqsn = EVENT_QUEUE_HEADER.decode(throwIfNull( eq ).data).seqNum;


        let mm_bids: OrderState[] = [];
        let mm_asks: OrderState[] = [];

        let cur_mm_bb = Number.MIN_SAFE_INTEGER;
        let cur_mm_ba = Number.MAX_SAFE_INTEGER;

        let book_bb = Number.MIN_SAFE_INTEGER;
        let book_ba = Number.MAX_SAFE_INTEGER;

        for(let bid of bids) {
            if(bid.openOrdersAddress.equals(this.open_orders)) {
                cur_mm_bb = Math.max(cur_mm_bb, bid.price);
                mm_bids.push(new OrderState(throwIfUndefined (bid.clientId), bid.side, bid.price, bid.size));
            }
            book_bb = Math.max(book_bb, bid.price)
        }
        for(let ask of asks) {
            if(ask.openOrdersAddress.equals(this.open_orders)) { 
                cur_mm_ba = Math.min(cur_mm_ba, ask.price);
                mm_asks.push(new OrderState(throwIfUndefined (ask.clientId), ask.side, ask.price, ask.size));
            }
            book_ba = Math.min(book_ba, ask.price);
        }

        book_bb = (book_bb == Number.MIN_SAFE_INTEGER) ? MIN_BID : book_bb;
        book_ba = (book_ba == Number.MAX_SAFE_INTEGER) ? MIN_ASK : book_ba;

        if(cur_mm_bb == Number.MIN_SAFE_INTEGER) {
            let clientID = this.gen_client_id(book_bb, 'buy');
            let bid_placed = await this.placeOrder(clientID, "buy", book_bb, 100);
            mm_bids.push(new OrderState(clientID, 'buy', book_bb, 100));
        }
        this.bid_pos = book_bb;

        if(cur_mm_ba == Number.MAX_SAFE_INTEGER) {
            let clientID2 = this.gen_client_id(book_ba, 'sell');
            let ask_placed = await this.placeOrder(clientID2, "sell", book_ba, 100);
            mm_asks.push(new OrderState(clientID2, 'sell', book_ba, 100));
        }
        this.ask_pos = book_ba;

        console.log("Positions Set");
       
        return [this.bid_pos, this.ask_pos, mm_bids, mm_asks, eqsn];
    }

    async onBook(best_bid : number, best_ask : number, mm_bids : OrderState[], mm_asks : OrderState[]) : Promise<[number, number, OrderState[], OrderState[]]> {
        console.log("onBook!");

        for(let ord of mm_bids) {
            console.log("bid: ", ord.clientID, ord.price, ord.size);
        }
       
        //bbo changed:
        if(best_bid > this.bid_pos) {
            for (let i = 0; i < mm_bids.length; i++) {
                let order = mm_bids[i];
                console.log("Cancelling " + order.size + "@" + order.price + " bid");
                await this.cancelOrder(order.clientID);
            }

            mm_bids = [];

            let clientID = this.gen_client_id(best_bid, 'buy');
            console.log("Placing " + ORDER_SIZE + "@" + best_bid + " bid");
            await this.placeOrder(clientID, 'buy', best_bid, ORDER_SIZE);
            mm_bids.push(new OrderState(clientID, 'buy', best_bid, ORDER_SIZE));
            this.bid_pos = best_bid;
        }

        if(best_ask < this.ask_pos) {
            for (let i = 0; i < mm_asks.length; i++) {
                let order = mm_asks[i];
                console.log("Cancelling " + order.size + "@" + order.price + " ask");
                await this.cancelOrder(order.clientID);
            }

            mm_asks = [];

            let clientID = this.gen_client_id(best_ask, 'sell');
            console.log("Placing " + ORDER_SIZE + "@" + best_ask + " ask");
            await this.placeOrder(clientID, 'sell', best_ask, ORDER_SIZE);
            mm_asks.push(new OrderState(clientID, 'sell', best_ask, ORDER_SIZE));
            this.ask_pos = best_ask;
        }
        
        return [this.bid_pos, this.ask_pos, mm_bids, mm_asks];
    }

    async onFill(bid_qty_refill : number, ask_qty_refill : number, mm_bids : OrderState[], mm_asks : OrderState[]): Promise<[OrderState[], OrderState[]]> {
        console.log("onFill!");
        
        if(bid_qty_refill >= ORDER_SIZE || ask_qty_refill >= ORDER_SIZE) return [mm_bids, mm_asks]; // Let onBook manage this (Full Fill case)
        if(bid_qty_refill == 0 && ask_qty_refill == 0) return [mm_bids, mm_asks]; // No action taken

        if(bid_qty_refill > 0) {
            let clientID = this.gen_client_id(this.bid_pos, 'buy');
            console.log("Placing " + bid_qty_refill + "@" + this.bid_pos + " bid");
            await this.placeOrder(clientID, 'buy', this.bid_pos, bid_qty_refill);
            mm_bids.push(new OrderState(clientID, 'buy', this.bid_pos, bid_qty_refill));
        }

        if(ask_qty_refill > 0) {
            let clientID2 = this.gen_client_id(this.ask_pos, 'sell');
            console.log("Placing " + ask_qty_refill + "@" + this.ask_pos + " ask");
            await this.placeOrder(clientID2, 'sell', this.ask_pos, ask_qty_refill);
            mm_asks.push(new OrderState(clientID2, 'sell', this.ask_pos, ask_qty_refill));
        }

        return [mm_bids, mm_asks];
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

    gen_client_id(price : number, side: 'buy' | 'sell') {
        let upper = new BN(price);
        upper.iushln(32);
        let lower = new BN(this.seq_num);
        if(side == 'buy') lower.inotn(32);
        this.seq_num++;
        return upper.uor(lower);
    }
}

function getMarketKeys(market_config : string) : [Connection, PublicKey, PublicKey, PublicKey, PublicKey, PublicKey, PublicKey, Account, PublicKey, PublicKey] {
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
    
    let open_orders = new PublicKey(tbl[10]);

    let bids = new PublicKey(tbl[11]);
    let asks = new PublicKey(tbl[12]);
    let event_q = new PublicKey(tbl[13]);


    return [connection, marketAddress, programAddress, open_orders, bids, asks, event_q, participant, payer_quote, payer_base];
}
