# Solarium examples

```
// Start Solarium's MM test (Init market & participants)
./run --nocapture

// Wait for "Made Market" log, then start the MM
cd examples/
ts-node main.ts // websocket implementation
ts-node mm.ts // polling implementation

// Simulating other orders:
ts-node participant.ts ('buy' | 'sell' ) qty@price
e.g. ts-node participant.ts buy 10@100

//Checking market state:
ts-node market_state.ts

// Settling participant funds
ts-node participant.ts settle

```
