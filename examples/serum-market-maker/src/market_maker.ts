import {
  Account,
  AccountInfo,
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { Market } from "@project-serum/serum";
import { Mutex } from "async-mutex";
import BN from "bn.js";
import { Orderbook } from "@project-serum/serum";
import { DexInstructions } from "@project-serum/serum/lib/instructions";
import {
  decodeEventsSince,
  EVENT_QUEUE_LAYOUT,
} from "@project-serum/serum/lib/queue";

// Safety parameters: when top of book is missing (this should be an
// exceptionally rare corner case), place a bid/ask at these prices.
// Alternatively, we could just skip the order until there is a level to join.
const MIN_BID_PRICE = 1;
const MIN_ASK_PRICE = 10000;

// Quantity to keep on each side.
const ORDER_SIZE = 100;

// The EVENT_QUEUE_LAYOUT.HEADER data can be deserialized to a number of fields.
// We are interested in the sequence number only to keep track of which events
// we have already seen.
export interface EventQueueHeader {
  seqNum: number;
}

// Represents an order that we placed.
export interface PlacedOrder {
  isBuy: boolean;
  price: number;
  size: number;
}

export class SimpleMarketMaker {
  connection: Connection;
  programId: PublicKey;
  marketAddress: PublicKey;
  bids: PublicKey;
  asks: PublicKey;
  eventQueue: PublicKey;
  eventQueueSequenceNumber: number;

  marketMaker: Keypair;
  marketMakerOpenOrders: PublicKey;
  marketMakerBaseVault: PublicKey;
  marketMakerQuoteVault: PublicKey;
  seqNum: number;
  market: Market;

  // Order state
  bidOrders: Map<string, PlacedOrder>;
  bidPrice: number | null;
  askOrders: Map<string, PlacedOrder>;
  askPrice: number | null;

  // Market state
  bestBid: number | null;
  bestAsk: number | null;

  constructor(
    connection: Connection,
    programId: PublicKey,
    marketAddress: PublicKey,
    bids: PublicKey,
    asks: PublicKey,
    eventQueue: PublicKey,

    marketMaker: Keypair,
    marketMakerOpenOrders: PublicKey,
    marketMakerQuoteVault: PublicKey,
    marketMakerBaseVault: PublicKey
  ) {
    this.connection = connection;
    this.programId = programId;
    this.marketAddress = marketAddress;
    this.bids = bids;
    this.asks = asks;
    this.eventQueue = eventQueue;

    this.marketMaker = marketMaker;
    this.marketMakerOpenOrders = marketMakerOpenOrders;
    this.marketMakerQuoteVault = marketMakerQuoteVault;
    this.marketMakerBaseVault = marketMakerBaseVault;

    this.bidPrice = null;
    this.askPrice = null;
    this.seqNum = 10;

    this.bidOrders = new Map();
    this.askOrders = new Map();
  }

  // Generates a unique ID for a new order. The ID will be used later on to cancel outstanding orders.
  generateClientId(price: number, side: "buy" | "sell"): BN {
    const upper = new BN(price);
    upper.iushln(32);
    const lower = new BN(this.seqNum);
    if (side == "buy") lower.inotn(32);
    this.seqNum++;
    return upper.uor(lower);
  }

  // Places a buy or sell limit order. Tracks the placed order in internal state.
  async placeOrder(isBuy: boolean, price: number, qty: number) {
    const side: "buy" | "sell" = isBuy ? "buy" : "sell";
    const payer = isBuy
      ? this.marketMakerQuoteVault
      : this.marketMakerBaseVault;
    const clientID = this.generateClientId(price, side);
    console.log("[placeOrder]", clientID.toString(), side, qty, "@", price);
    await this.market.placeOrder(this.connection, {
      owner: new Account(this.marketMaker.secretKey),
      payer: payer,
      clientId: clientID,
      side: side,
      price: price,
      size: qty,
      orderType: "limit",
      feeDiscountPubkey: null,
    });

    const orders = isBuy ? this.bidOrders : this.askOrders;
    orders.set(clientID.toString(), {
      isBuy,
      price,
      size: qty,
    });
  }

  // Cancels all orders in the given map.
  async cancelAll(orders: Map<string, PlacedOrder>) {
    if (orders.size == 0) return;
    const transaction = new Transaction();
    for (const [orderID, order] of orders) {
      const instr = DexInstructions.cancelOrderByClientIdV2({
        market: this.marketAddress,
        bids: this.bids,
        asks: this.asks,
        eventQueue: this.eventQueue,
        openOrders: this.marketMakerOpenOrders,
        owner: this.marketMaker.publicKey,
        clientId: new BN(orderID),
        programId: this.market.programId,
      });
      console.log("[cancelAll]", orderID, order.size, "@", order.price);
      transaction.add(instr);
    }
    await sendAndConfirmTransaction(this.connection, transaction, [
      this.marketMaker,
    ]);
  }

  // Initializes the market maker state.
  async initialize() {
    this.market = await Market.load(
      this.connection,
      this.marketAddress,
      {},
      this.programId
    );
    const bids = await this.market.loadBids(this.connection);
    const asks = await this.market.loadAsks(this.connection);
    const eq = await this.connection.getAccountInfo(this.eventQueue);
    const decodedEventQueue = EVENT_QUEUE_LAYOUT.HEADER.decode(
      throwIfNull(eq).data
    ) as EventQueueHeader;
    this.eventQueueSequenceNumber = decodedEventQueue.seqNum;

    this.bestAsk = null;
    this.bestBid = null;

    for (const bid of bids) {
      if (bid.openOrdersAddress.equals(this.marketMakerOpenOrders)) {
        const id = throwIfUndefined(bid.clientId);
        this.bidOrders.set(id.toString(), {
          isBuy: true,
          price: bid.price,
          size: bid.size,
        });
      }
      if (this.bestBid == null) {
        this.bestBid = bid.price;
      } else {
        this.bestBid = Math.max(this.bestBid, bid.price);
      }
    }
    for (let ask of asks) {
      if (ask.openOrdersAddress.equals(this.marketMakerOpenOrders)) {
        const id = throwIfUndefined(ask.clientId);
        this.askOrders.set(id.toString(), {
          isBuy: false,
          price: ask.price,
          size: ask.size,
        });
      }
      if (this.bestAsk == null) {
        this.bestAsk = ask.price;
      } else {
        this.bestAsk = Math.max(this.bestAsk, ask.price);
      }
    }

    await this.cancelAll(this.askOrders);
    this.askOrders.clear();
    await this.cancelAll(this.bidOrders);
    this.bidOrders.clear();

    await this.onBid();
    await this.onAsk();
  }

  // Reacts to bid changes.
  async onBid() {
    if (this.bestBid != this.bidPrice && this.bestBid != null) {
      await this.cancelAll(this.bidOrders);
      this.bidOrders.clear();
      await this.placeOrder(true, this.bestBid, ORDER_SIZE);
      this.bidPrice = this.bestBid;
    }
  }

  // Reacts to ask changes.
  async onAsk() {
    if (this.bestAsk != this.askPrice && this.bestAsk != null) {
      await this.cancelAll(this.askOrders);
      this.askOrders.clear();
      await this.placeOrder(false, this.bestAsk, ORDER_SIZE);
      this.askPrice = this.bestAsk;
    }
  }

  // Reacts to a fill event.
  async onFill(bidFillQuantity: number, askFillQuantity: number) {
    console.log(
      "[onFill] bidFillQuantity",
      bidFillQuantity,
      "askFillQuantity",
      askFillQuantity
    );
    if (bidFillQuantity > 0 && this.bidPrice != null) {
      await this.placeOrder(true, this.bidPrice, bidFillQuantity);
    }
    if (askFillQuantity > 0 && this.askPrice != null) {
      await this.placeOrder(false, this.askPrice, askFillQuantity);
    }

    if (bidFillQuantity || askFillQuantity) {
      await this.settle();
    }
  }

  // Settles funds. This only needs to be called occasionally if the market
  // maker's token accounts have sufficient inventory. In this example, we
  // settle as soon as there is a fill.
  async settle() {
    for (const openOrders of await this.market.findOpenOrdersAccountsForOwner(
      this.connection,
      this.marketMaker.publicKey
    )) {
      if (
        openOrders.baseTokenFree > new BN(0) ||
        openOrders.quoteTokenFree > new BN(0)
      ) {
        await this.market.settleFunds(
          this.connection,
          new Account(this.marketMaker.secretKey),
          openOrders,
          this.marketMakerBaseVault,
          this.marketMakerQuoteVault
        );
      }
    }
  }

  // Runs the market maker. Initializes and sets up the event listeners.
  async run() {
    await this.initialize();
    const mutex = new Mutex();

    // Listens to bid account changes.
    this.connection.onAccountChange(
      this.bids,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.runExclusive(async () => {
          let bids = Orderbook.decode(this.market, accountinfo.data);
          let topOfBook = bids.items(true).next();
          this.bestBid =
            topOfBook.value == null ? MIN_BID_PRICE : topOfBook.value.price;
          await this.onBid();
        });
      }
    );

    // Listens to ask account changes.
    this.connection.onAccountChange(
      this.asks,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.runExclusive(async () => {
          let asks = Orderbook.decode(this.market, accountinfo.data);
          let topOfBook = asks.items().next();
          this.bestAsk =
            topOfBook.value == null ? MIN_ASK_PRICE : topOfBook.value.price;
          await this.onAsk();
        });
      }
    );

    // Listens to fills.
    this.connection.onAccountChange(
      this.eventQueue,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.runExclusive(async () => {
          let eq: any[] = decodeEventsSince(
            accountinfo.data,
            this.eventQueueSequenceNumber
          ).filter((event) => event.eventFlags.fill);
          this.eventQueueSequenceNumber = (
            EVENT_QUEUE_LAYOUT.HEADER.decode(
              accountinfo.data
            ) as EventQueueHeader
          ).seqNum;
          let [bidOrdersFilled, askOrdersFilled] =
            this.parseFillsFromEventQueue(eq);

          if (eq.length != 0)
            await this.onFill(bidOrdersFilled, askOrdersFilled);
        });
      }
    );
  }

  // Updates internal order state with fill information and returns aggregate
  // filled bids and asks.
  parseFillsFromEventQueue(events: any[]): [number, number] {
    let bidOrdersFilled = 0;
    let askOrdersFilled = 0;
    const bidsToDelete: BN[] = [];
    const asksToDelete: BN[] = [];
    for (const event of events) {
      const matchingBid = this.bidOrders.get(event.clientOrderId.toString());
      if (matchingBid != undefined) {
        bidOrdersFilled += event.nativeQuantityReleased;
        matchingBid.size -= event.nativeQuantityReleased;
        if (matchingBid.size == 0) {
          bidsToDelete.push(event.clientOrderId);
        }
      } else {
        const matchingAsk = this.askOrders.get(event.clientOrderId.toString());
        if (matchingAsk != undefined) {
          askOrdersFilled += event.nativeQuantityReleased;
          matchingAsk.size -= event.nativeQuantityReleased;
          if (matchingAsk.size == 0) {
            asksToDelete.push(event.clientOrderId);
          }
        }
      }
    }

    for (const key of bidsToDelete) {
      this.bidOrders.delete(key.toString());
    }
    for (const key of asksToDelete) {
      this.askOrders.delete(key.toString());
    }
    return [bidOrdersFilled, askOrdersFilled];
  }
}

export function throwIfNull<T>(value: T | null): T {
  if (value === null) {
    throw new Error("account not found");
  }
  return value;
}

export function throwIfUndefined<T>(value: T | undefined): T {
  if (value === undefined) {
    throw new Error("account not found");
  }
  return value;
}
