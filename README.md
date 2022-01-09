# solarium = solana + selenium

Test driver for Solana.

Example usage:

```rust
// Starts solana-test-validator with an empty ledger.
let sandbox = Sandbox::new()?;

// Creates an actor and airdrops 10 SOL.
let actor = Actor::new(&sandbox);
actor.airdrop(10 * LAMPORTS_PER_SOL)?;

// Actor deploys a proram.
let program = actor.deploy(std::path::Path::new("target/deploy/program.so"))?;

// Creates a mint and associated token account.
let mint = Mint::new(&sandbox, &actor, 0, None, None)?;
let token_account = TokenAccount::new(&sandbox, &actor, &mint, None)?;

// Mints to the token account.
let account_info = token_account.get_state()?;
assert_eq!(0, account_info.amount);
mint.mint_to(&actor, &token_account, 123)?;
let account_info = token_account.get_state()?;
assert_eq!(123, account_info.amount);
```