use crate::actor::Actor;
use crate::errors::Error;
use crate::sandbox::Sandbox;
use crate::token::{Mint, TokenAccount};
use bytemuck;
use serum_dex::state as serum_state;
use solana_sdk::pubkey::Pubkey;

/// Represents a Serum market. This is a V2 market if there is an authority
/// specified, otherwise a V1 market.
pub struct Market<'a> {
    _sandbox: &'a Sandbox,
    _market: Actor<'a>,
    _request_queue: Actor<'a>,
    _event_queue: Actor<'a>,
    _bids: Actor<'a>,
    _asks: Actor<'a>,
    _base_vault: TokenAccount<'a>,
    _quote_vault: TokenAccount<'a>,
}

impl<'a> Market<'a> {
    fn request_queue_size(num_requests: usize) -> usize {
        let mut size: usize = 0;
        size += serum_state::ACCOUNT_HEAD_PADDING.len();
        size += serum_state::ACCOUNT_TAIL_PADDING.len();
        size += std::mem::size_of::<serum_state::RequestQueueHeader>();
        size += num_requests * std::mem::size_of::<serum_state::Request>();
        size
    }

    fn event_queue_size(num_events: usize) -> usize {
        let mut size: usize = 0;
        size += serum_state::ACCOUNT_HEAD_PADDING.len();
        size += serum_state::ACCOUNT_TAIL_PADDING.len();
        size += std::mem::size_of::<serum_state::EventQueueHeader>();
        size += num_events * std::mem::size_of::<serum_state::Event>();
        size
    }

    fn side_size(num_nodes: usize) -> usize {
        let mut size: usize = 0;
        size += serum_state::ACCOUNT_HEAD_PADDING.len();
        size += serum_state::ACCOUNT_TAIL_PADDING.len();
        size += 8; // private struct OrderBookStateHeader
        size += 8 + 8 + 4 + 4 + 8; // private struct SlabHeader
        size += num_nodes * std::mem::size_of::<serum_dex::critbit::AnyNode>();
        size
    }

    fn market_size(has_authority: bool) -> usize {
        let mut size: usize = 0;
        size += serum_state::ACCOUNT_HEAD_PADDING.len();
        size += serum_state::ACCOUNT_TAIL_PADDING.len();
        if has_authority {
            size += std::mem::size_of::<serum_state::MarketStateV2>();
        } else {
            size += std::mem::size_of::<serum_state::MarketState>();
        }
        size
    }

    /// Generates the vault authority address. Note that you cannot use
    /// find_program_address because Serum uses a u64 nonce convention.
    fn create_vault_address(serum: &Pubkey, market: &Pubkey) -> (Pubkey, u64) {
        let mut nonce: u64 = 0;
        loop {
            let seeds = [market.as_ref(), bytemuck::bytes_of(&nonce)];
            match Pubkey::create_program_address(&seeds, serum) {
                Ok(key) => return (key, nonce),
                _ => nonce += 1,
            }
        }
    }

    /// Creates and initializes a serum market. Creation is funded by the given
    /// actor. If an authority is provided then a V2 market is created.
    /// Otherwise, a V1 market is created.
    pub fn new(
        sandbox: &'a Sandbox,
        actor: &'a Actor,
        serum: &'a Pubkey,
        base_mint: &'a Mint,
        quote_mint: &'a Mint,
        authority: Option<&'a Pubkey>,
        base_lot_size: u64,
        quote_lot_size: u64,
        dust_threshold: u64,
        request_queue_size: usize,
        event_queue_size: usize,
        book_size: usize,
    ) -> Result<Self, Error> {
        if request_queue_size == 0 {
            return Err(Error::from(serum_dex::error::DexError::from(
                serum_dex::error::DexErrorCode::RequestQueueEmpty,
            )));
        }
        if event_queue_size < 128 {
            return Err(Error::from(serum_dex::error::DexError::from(
                serum_dex::error::DexErrorCode::EventQueueTooSmall,
            )));
        }
        if book_size <= 200 {
            return Err(Error::from(serum_dex::error::DexError::from(
                serum_dex::error::DexErrorCode::SlabTooSmall,
            )));
        }

        let market = Actor::new(sandbox);
        let request_queue = Actor::new(sandbox);
        let event_queue = Actor::new(sandbox);
        let bids = Actor::new(sandbox);
        let asks = Actor::new(sandbox);

        let (vault_address, vault_nonce) = Self::create_vault_address(serum, market.pubkey());
        let base_vault = TokenAccount::new(&sandbox, actor, &base_mint, Some(&vault_address))?;
        let quote_vault = TokenAccount::new(&sandbox, actor, &quote_mint, Some(&vault_address))?;
        let has_authority = match authority {
            Some(_) => true,
            None => false,
        };

        let book_size = Self::side_size(book_size);
        let sized_accounts = vec![
            (market.pubkey(), Self::market_size(has_authority)),
            (
                request_queue.pubkey(),
                Self::request_queue_size(request_queue_size),
            ),
            (
                event_queue.pubkey(),
                Self::event_queue_size(event_queue_size),
            ),
            (bids.pubkey(), book_size),
            (asks.pubkey(), book_size),
        ];

        let mut instructions = Vec::new();
        for (pubkey, len) in sized_accounts.iter() {
            instructions.push(solana_sdk::system_instruction::create_account(
                actor.pubkey(),
                pubkey,
                sandbox
                    .client()
                    .get_minimum_balance_for_rent_exemption(*len)?,
                *len as u64,
                serum,
            ));
        }

        instructions.push(serum_dex::instruction::initialize_market(
            market.pubkey(),
            serum,
            base_mint.actor().pubkey(),
            quote_mint.actor().pubkey(),
            base_vault.account().pubkey(),
            quote_vault.account().pubkey(),
            authority,
            authority,
            authority,
            bids.pubkey(),
            asks.pubkey(),
            request_queue.pubkey(),
            event_queue.pubkey(),
            base_lot_size,
            quote_lot_size,
            vault_nonce,
            dust_threshold,
        )?);

        let recent_hash = sandbox.client().get_latest_blockhash()?;
        let market_transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &instructions,
            Some(actor.pubkey()),
            &vec![
                actor.keypair(),
                market.keypair(),
                request_queue.keypair(),
                event_queue.keypair(),
                bids.keypair(),
                asks.keypair(),
            ],
            recent_hash,
        );
        sandbox
            .client()
            .send_and_confirm_transaction(&market_transaction)?;

        Ok(Market {
            _sandbox: sandbox,
            _market: market,
            _request_queue: request_queue,
            _event_queue: event_queue,
            _bids: bids,
            _asks: asks,
            _base_vault: base_vault,
            _quote_vault: quote_vault,
        })
    }
}
