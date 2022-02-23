import {
  Account,
  AccountInfo,
  Connection,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { Market } from "@project-serum/serum";
import { Mutex } from "async-mutex";
import BN from "bn.js";
import { Orderbook } from "@project-serum/serum";
import { blob, struct, u32 } from "buffer-layout";
import { accountFlagsLayout, zeros } from "@project-serum/serum/lib/layout";
import { DexInstructions } from "@project-serum/serum/lib/instructions";
import { decodeEventsSince } from "@project-serum/serum/lib/queue";

const MIN_BID = 1;
const MIN_ASK = 10000;
const ORDER_SIZE = 100;

export const EVENT_QUEUE_HEADER = struct([
  blob(5),

  accountFlagsLayout("accountFlags"),
  u32("head"),
  zeros(4),
  u32("count"),
  zeros(4),
  u32("seqNum"),
  zeros(4),
]);

export interface EventQueue {
  head: number;
  count: number;
  seqNum: number;
}

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
  bidOrders: Map<string, OrderState>;
  bidPrice: number | null;
  askOrders: Map<string, OrderState>;
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

  async placeOrder(
    clientID: BN,
    side: "buy" | "sell",
    price: number,
    qty: number
  ) {
    let market = this.market;

    if (side == "buy") {
      return await market.placeOrder(this.connection, {
        owner: new Account(this.marketMaker.secretKey),
        payer: this.marketMakerQuoteVault,
        clientId: clientID,
        side: "buy",
        price: price,
        size: qty,
        orderType: "limit",
        feeDiscountPubkey: null,
      });
    } else {
      return await market.placeOrder(this.connection, {
        owner: new Account(this.marketMaker.secretKey),
        payer: this.marketMakerBaseVault,
        clientId: clientID,
        side: "sell",
        price: price,
        size: qty,
        orderType: "limit",
        feeDiscountPubkey: null,
      });
    }
  }

  async cancelAll(orders: Map<string, OrderState>) {
    if (orders.size == 0) return;
    const transaction = new Transaction();
    for (let order of orders.values()) {
      let instr = DexInstructions.cancelOrderByClientIdV2({
        market: this.marketAddress,
        bids: this.bids,
        asks: this.asks,
        eventQueue: this.eventQueue,
        openOrders: this.marketMakerOpenOrders,
        owner: this.marketMaker.publicKey,
        clientId: order.clientID,
        programId: this.market.programId,
      });
      console.log("Cancelling " + order.size + "@" + order.price);
      transaction.add(instr);
    }
    this.sendTxn(transaction, [this.marketMaker]);
  }

  async sendTxn(txn: Transaction, signers: Array<Keypair>) {
    try {
      const signature = await this.connection.sendTransaction(txn, signers, {
        skipPreflight: true,
      });

      await this.connection.confirmTransaction(signature);
      return signature;
    } catch {}
  }

  async initialize() {
    this.market = await Market.load(
      this.connection,
      this.marketAddress,
      {},
      this.programId
    );
    let bids = await this.market.loadBids(this.connection);
    let asks = await this.market.loadAsks(this.connection);
    let eq = await this.connection.getAccountInfo(this.eventQueue);
    let decodedEventQueue = EVENT_QUEUE_HEADER.decode(
      throwIfNull(eq).data
    ) as EventQueue;
    this.eventQueueSequenceNumber = decodedEventQueue.seqNum;

    this.bestAsk = null;
    this.bestBid = null;

    for (let bid of bids) {
      if (bid.openOrdersAddress.equals(this.marketMakerOpenOrders)) {
        let id = throwIfUndefined(bid.clientId);
        this.bidOrders.set(
          id.toString(),
          new OrderState(id, bid.side, bid.price, bid.size)
        );
      }
      if (this.bestBid == null) {
        this.bestBid = bid.price;
      } else {
        this.bestBid = Math.max(this.bestBid, bid.price);
      }
    }
    for (let ask of asks) {
      if (ask.openOrdersAddress.equals(this.marketMakerOpenOrders)) {
        let id = throwIfUndefined(ask.clientId);
        this.askOrders.set(
          id.toString(),
          new OrderState(id, ask.side, ask.price, ask.size)
        );
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

    await this.onBook();
  }

  async onBook() {
    console.log(
      "onBook() bestBid = ",
      this.bestBid,
      "bestAsk = ",
      this.bestAsk,
      "bidPrice = ",
      this.bidPrice,
      "askPrice = ",
      this.askPrice
    );
    if (this.bestBid != this.bidPrice && this.bestBid != null) {
      await this.cancelAll(this.bidOrders);
      this.bidOrders.clear();
      let clientID = this.generateClientId(this.bestBid, "buy");
      console.log("Placing " + ORDER_SIZE + "@" + this.bestBid + " bid");
      await this.placeOrder(clientID, "buy", this.bestBid, ORDER_SIZE);
      this.bidOrders.set(
        clientID.toString(),
        new OrderState(clientID, "buy", this.bestBid, ORDER_SIZE)
      );
      this.bidPrice = this.bestBid;
    }

    if (this.bestAsk != this.askPrice && this.bestAsk != null) {
      await this.cancelAll(this.askOrders);
      this.askOrders.clear();
      let clientID = this.generateClientId(this.bestAsk, "sell");
      console.log("Placing " + ORDER_SIZE + "@" + this.bestAsk + " ask");
      await this.placeOrder(clientID, "sell", this.bestAsk, ORDER_SIZE);
      this.askOrders.set(
        clientID.toString(),
        new OrderState(clientID, "sell", this.bestAsk, ORDER_SIZE)
      );
      this.askPrice = this.bestAsk;
    }
  }

  async onFill(bidFillQuantity: number, askFillQuantity: number) {
    if (bidFillQuantity > 0 && this.bidPrice != null) {
      let clientID = this.generateClientId(this.bidPrice, "buy");
      console.log("Placing " + bidFillQuantity + "@" + this.bidPrice + " bid");
      await this.placeOrder(clientID, "buy", this.bidPrice, bidFillQuantity);
      this.bidOrders.set(
        clientID.toString(),
        new OrderState(clientID, "buy", this.bidPrice, bidFillQuantity)
      );
    }

    if (askFillQuantity > 0 && this.askPrice != null) {
      let clientID = this.generateClientId(this.askPrice, "sell");
      console.log("Placing " + askFillQuantity + "@" + this.askPrice + " ask");
      await this.placeOrder(clientID, "sell", this.askPrice, askFillQuantity);
      this.askOrders.set(
        clientID.toString(),
        new OrderState(clientID, "sell", this.askPrice, askFillQuantity)
      );
    }
  }

  async settle() {
    let market = this.market;

    for (let openOrders of await market.findOpenOrdersAccountsForOwner(
      this.connection,
      this.marketMaker.publicKey
    )) {
      if (
        openOrders.baseTokenFree > new BN(0) ||
        openOrders.quoteTokenFree > new BN(0)
      ) {
        // spl-token accounts to which to send the proceeds from trades
        await market.settleFunds(
          this.connection,
          new Account(this.marketMaker.secretKey),
          openOrders,
          this.marketMakerBaseVault,
          this.marketMakerQuoteVault
        );
      }
    }
  }

  generateClientId(price: number, side: "buy" | "sell") {
    let upper = new BN(price);
    upper.iushln(32);
    let lower = new BN(this.seqNum);
    if (side == "buy") lower.inotn(32);
    this.seqNum++;
    return upper.uor(lower);
  }

  async run() {
    await this.initialize();
    const mutex = new Mutex();

    this.connection.onAccountChange(
      this.bids,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.runExclusive(async () => {
          let bids = Orderbook.decode(this.market, accountinfo.data);
          let topOfBook = bids.items(true).next();
          this.bestBid =
            topOfBook.value == null ? MIN_BID : topOfBook.value.price;
          await this.onBook();
        });
      }
    );

    this.connection.onAccountChange(
      this.asks,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.runExclusive(async () => {
          let asks = Orderbook.decode(this.market, accountinfo.data);
          let topOfBook = asks.items().next();
          this.bestAsk =
            topOfBook.value == null ? MIN_ASK : topOfBook.value.price;
          await this.onBook();
        });
      }
    );

    this.connection.onAccountChange(
      this.eventQueue,
      async (accountinfo: AccountInfo<Buffer>) => {
        await mutex.runExclusive(async () => {
          let eq: any[] = decodeEventsSince(
            accountinfo.data,
            this.eventQueueSequenceNumber
          ).filter((event) => event.eventFlags.fill);
          this.eventQueueSequenceNumber = (
            EVENT_QUEUE_HEADER.decode(accountinfo.data) as EventQueue
          ).seqNum;
          let [bidOrdersFilled, askOrdersFilled] =
            this.parseFillsFromEventQueue(eq);

          if (eq.length != 0)
            await this.onFill(bidOrdersFilled, askOrdersFilled);
        });
      }
    );
  }

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
