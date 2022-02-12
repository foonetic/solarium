# Solarium examples

MM Bot on Solarium

Before using:

(npm)
```
npm install @solana/web3.js @project-serum/serum
```
(yarn)
```
yarn add @solana/web3.js @project-serum/serum
```

Example usage:

```
// Start Solarium's MM test (Init market & participants)
cargo test -- --nocapture mm_bot

// Wait for "Made Market" log, then start the MM
cd examples/
ts-node mm.ts

// Simulating other orders:
ts-node participant.ts ('buy' | 'sell' ) qty@price
e.g. ts-node participant.ts buy 10@100

//Checking market state:
ts-node market_state.ts

// Settling participant funds
ts-node participant.ts settle

```