import { Engine } from "./engine";
import { MarketMaker, SimpleMarketMaker } from "./mm_ws";

let mm: MarketMaker = new SimpleMarketMaker("../mm_keys.txt");
let engine = new Engine(mm);

engine.run();
