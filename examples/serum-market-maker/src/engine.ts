import { Orderbook } from "@project-serum/serum";
import { Connection, PublicKey, AccountInfo } from "@solana/web3.js";
import BN from "bn.js";
import * as fs from "fs";
import { MarketMaker, EVENT_QUEUE_HEADER } from "./mm_ws";
import { Mutex } from "async-mutex";
import { decodeEventsSince } from "@project-serum/serum/lib/queue";

const MIN_BID = 1;
const MIN_ASK = 10000;
const ORDER_SIZE = 100;

export class OrderState {
  clientID: BN;
  side: "buy" | "sell";
  price: number;
  size: number;

  constructor(orderID: BN, side: "buy" | "sell", price: number, size: number) {
    this.clientID = orderID;
    this.side = side;
    this.price = price;
    this.size = size;
  }
}

export class Engine {
  connection: Connection;
  market_bids: PublicKey;
  market_asks: PublicKey;
  best_bid: number;
  best_ask: number;
  market_eq: PublicKey;
  init_pos: boolean;
  mm: MarketMaker;
  bid_state: OrderState[];
  ask_state: OrderState[];
  mm_bids: OrderState[];
  mm_asks: OrderState[];
  eq_seq_num: number;

  constructor(mm: MarketMaker) {
    this.mm = mm;
    this.init_pos = false;
    this.eq_seq_num = 0;
    [this.connection, this.market_bids, this.market_asks, this.market_eq] =
      getEngineKeys();
  }

  async run() {
    if (this.init_pos == false) {
      [
        this.best_bid,
        this.best_ask,
        this.mm_bids,
        this.mm_asks,
        this.eq_seq_num,
      ] = await this.mm.initialize_positions();
      this.init_pos = true;
    }

    const mutex = new Mutex();

    this.connection.onAccountChange(
      this.market_bids,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.acquire();

        let bids = Orderbook.decode(this.mm.market, accountinfo.data);
        let top_bid = bids.items(true).next();

        this.best_bid = top_bid.value == null ? MIN_BID : top_bid.value.price;

        [this.best_bid, this.best_ask, this.mm_bids, this.mm_asks] =
          await this.mm.onBook(
            this.best_bid,
            this.best_ask,
            this.mm_bids,
            this.mm_asks
          );

        mutex.release();
      }
    );

    this.connection.onAccountChange(
      this.market_asks,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.acquire();

        let asks = Orderbook.decode(this.mm.market, accountinfo.data);
        let top_ask = asks.items().next();

        this.best_ask = top_ask.value == null ? MIN_ASK : top_ask.value.price;

        [this.best_bid, this.best_ask, this.mm_bids, this.mm_asks] =
          await this.mm.onBook(
            this.best_bid,
            this.best_ask,
            this.mm_bids,
            this.mm_asks
          );

        mutex.release();
      }
    );

    this.connection.onAccountChange(
      this.market_eq,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.acquire();
        let eq: any[] = decodeEventsSince(
          accountinfo.data,
          this.eq_seq_num
        ).filter((event) => event.eventFlags.fill);
        this.eq_seq_num = EVENT_QUEUE_HEADER.decode(accountinfo.data).seqNum;
        let [bid_refill, ask_refill] = this.refresh_mm(eq);

        if (eq.length != 0)
          [this.mm_bids, this.mm_asks] = await this.mm.onFill(
            bid_refill,
            ask_refill,
            this.mm_bids,
            this.mm_asks
          );

        mutex.release();
      }
    );
  }

  refresh_mm(events: any[]) {
    let bid_qty_refill = 0;
    let ask_qty_refill = 0;
    for (let event of events) {
      for (let i = 0; i < this.mm_bids.length; ) {
        if (this.mm_bids[i].clientID.eq(event.clientOrderId)) {
          bid_qty_refill += event.nativeQuantityReleased;
          this.mm_bids[i].size -= event.nativeQuantityReleased;
        }
        if (this.mm_bids[i].size == 0) {
          this.mm_bids.splice(i, 1);
          continue;
        }
        i++;
      }

      for (let i = 0; i < this.mm_asks.length; ) {
        if (this.mm_asks[i].clientID.eq(event.clientOrderId)) {
          ask_qty_refill += event.nativeQuantityReleased;
          this.mm_asks[i].size -= event.nativeQuantityReleased;
        }
        if (this.mm_asks[i].size == 0) {
          this.mm_asks.splice(i, 1);
          continue;
        }
        i++;
      }
    }
    return [bid_qty_refill, ask_qty_refill];
  }
}

function getEngineKeys(): [Connection, PublicKey, PublicKey, PublicKey] {
  let text = fs.readFileSync(__dirname + "/engine_keys.txt", "utf8");
  let tbl = text.split("\n");

  let connection = new Connection(tbl[0]);
  let market_bids = new PublicKey(tbl[1]);
  let market_asks = new PublicKey(tbl[2]);
  let market_eq = new PublicKey(tbl[3]);

  return [connection, market_bids, market_asks, market_eq];
}

export function throwIfNull<T>(
  value: T | null,
  message = "account not found"
): T {
  if (value === null) {
    throw new Error(message);
  }
  return value;
}

export function throwIfUndefined<T>(
  value: T | undefined,
  message = "account not found"
): T {
  if (value === undefined) {
    throw new Error(message);
  }
  return value;
}
