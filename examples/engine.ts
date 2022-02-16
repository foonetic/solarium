import { Orderbook } from "@project-serum/serum";
import { Connection, PublicKey, AccountInfo } from "@solana/web3.js";
import * as fs from 'fs';
import { MarketMaker } from "./mm_ws";

export class Engine {
    connection   : Connection;
    market_bids  : PublicKey;
    market_asks  : PublicKey;
    market_eq    : PublicKey;
    init_pos     : boolean;
    mm           : MarketMaker;

    constructor(mm : MarketMaker) {

        this.mm = mm;
        this.init_pos = false;
        [this.connection, this.market_bids, this.market_asks, this.market_eq] = getEngineKeys();
    }

    async run() {

        if(this.init_pos == false) {
            await this.mm.initialize_positions();
            this.init_pos = true;
        }

        this.connection.onAccountChange(this.market_bids, async (accountinfo : AccountInfo<Buffer>) => {
            let bids = Orderbook.decode(this.mm.market, accountinfo.data);
            let asks = Orderbook.decode(this.mm.market, throwIfNull( await this.connection.getAccountInfo(this.market_asks) ).data);
            await this.mm.onBook(bids, asks);
        });
        this.connection.onAccountChange(this.market_eq, async (accountinfo : AccountInfo<Buffer>) => {await this.mm.onFill()});
    }
}

function getEngineKeys(): [Connection, PublicKey, PublicKey, PublicKey] {
    let text = fs.readFileSync(__dirname + "/engine_keys.txt",'utf8');
    let tbl = text.split("\n");

    let connection = new Connection(tbl[0]);
    let market_bids = new PublicKey(tbl[1]);
    let market_asks = new PublicKey(tbl[2]);
    let market_eq = new PublicKey(tbl[3]);

    return [connection, market_bids, market_asks, market_eq];
}

function throwIfNull<T>(value: T | null, message = 'account not found'): T {
    if (value === null) {
      throw new Error(message);
    }
    return value;
  }