use crate::actor::Actor;
use crate::errors::Error;
use crate::sandbox::Sandbox;
use crate::token::{Mint, TokenAccount};
use bytemuck;
use serum_dex::{
    instruction::SelfTradeBehavior,
    matching::{OrderType, Side},
    state as serum_state,
};
use solana_sdk::pubkey::Pubkey;
use std::num::NonZeroU64;

/// Represents a Serum market. This is a V2 market if there is an authority
/// specified, otherwise a V1 market.
pub struct Market<'a> {
    _sandbox: &'a Sandbox,
    serum: &'a Pubkey,
    market: Actor<'a>,
    authority: Option<&'a Pubkey>,
    _request_queue: Actor<'a>,
    _event_queue: Actor<'a>,
    _bids: Actor<'a>,
    _asks: Actor<'a>,
    _base_vault: TokenAccount<'a>,
    _quote_vault: TokenAccount<'a>,
    base_mint: &'a Mint<'a>,
    quote_mint: &'a Mint<'a>,
    pub open_orders_accounts: Vec<&'a Pubkey>,
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

    pub fn consume_events(
        &self,
        participant: Participant<'a>,
        coin_fee_receivable_account: &Pubkey,
        pc_fee_receivable_account: &Pubkey,
        limit: u16,
    ) -> Result<(), Error> {
        let mut instructions = Vec::new();
        instructions.push(serum_dex::instruction::consume_events(
            self.serum,
            self.open_orders_accounts.clone(),
            self.market.pubkey(),
            self._event_queue.pubkey(),
            coin_fee_receivable_account,
            pc_fee_receivable_account,
            limit,
        )?);

        let recent_hash = self._sandbox.client().get_latest_blockhash()?;
        let market_transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &instructions,
            Some(participant.account().pubkey()),
            &vec![
                participant.account().keypair(),
                self.market.keypair(),
                self._request_queue.keypair(),
                self._event_queue.keypair(),
                self._bids.keypair(),
                self._asks.keypair(),
            ],
            recent_hash,
        );
        self._sandbox
            .client()
            .send_and_confirm_transaction(&market_transaction)?;

        Ok(())
    }

    /// Creates a new order and pushes it to the sandbox.
    /// The participant pays the fees for this transaction.
    pub fn new_order(
        &self,
        payer: &Actor<'a>,
        participant: &Participant<'a>,
        side: Side,
        limit_price: NonZeroU64,
        order_type: OrderType,
        max_base_qty: NonZeroU64,
        client_order_id: u64,
        self_trade_behavior: SelfTradeBehavior,
        limit: u16,
        max_native_quote_including_fees: NonZeroU64,
        srm_account_referral: Option<&Pubkey>,
    ) -> Result<(), Error> {
        let mut instructions = Vec::new();

        // Create new order instruction.
        // The participant pays and is the owner of the open_orders account provided.
        instructions.push(serum_dex::instruction::new_order(
            self.market.pubkey(),
            participant.open_orders().pubkey(),
            self._request_queue.pubkey(),
            self._event_queue.pubkey(),
            self._bids.pubkey(),
            self._asks.pubkey(),
            payer.pubkey(),
            participant.account().pubkey(),
            self._base_vault.account().pubkey(),
            self._quote_vault.account().pubkey(),
            &spl_token::ID,
            &solana_program::sysvar::rent::ID,
            srm_account_referral,
            self.serum,
            side,
            limit_price,
            max_base_qty,
            order_type,
            client_order_id,
            self_trade_behavior,
            limit,
            max_native_quote_including_fees,
        )?);

        // Push transaction
        let recent_hash = self._sandbox.client().get_latest_blockhash()?;
        let market_transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &instructions,
            Some(participant.account().pubkey()),
            &vec![participant.account().keypair()],
            recent_hash,
        );
        self._sandbox
            .client()
            .send_and_confirm_transaction(&market_transaction)?;

        Ok(())
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
            instructions.push(actor.create_account(pubkey, *len, serum)?);
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

        let open_orders_accounts = Vec::new();
        Ok(Market {
            _sandbox: sandbox,
            serum: serum,
            market: market,
            authority: authority,
            _request_queue: request_queue,
            _event_queue: event_queue,
            _bids: bids,
            _asks: asks,
            _base_vault: base_vault,
            _quote_vault: quote_vault,
            base_mint: base_mint,
            quote_mint: quote_mint,
            open_orders_accounts: open_orders_accounts,
        })
    }
}

/// Represents a Serum market participant.
pub struct Participant<'a> {
    _market: &'a Market<'a>,
    _base: TokenAccount<'a>,
    _quote: TokenAccount<'a>,
    _open_orders: Actor<'a>,
    _account: Actor<'a>,
}

impl<'a> Participant<'a> {
    /// Returns base account.
    pub fn base(&self) -> &Actor {
        &self._base.account()
    }

    /// Returns quote account.
    pub fn quote(&self) -> &Actor {
        &self._quote.account()
    }

    /// Returns open orders account.
    pub fn open_orders(&self) -> &Actor {
        &self._open_orders
    }

    /// Returns underlying account.
    pub fn account(&self) -> &Actor {
        &self._account
    }

    /// Constructs a Serum market participant and seeds the participant account
    /// with lamports to drive transactions, as well as some amount of base and
    /// quote tokens.
    pub fn new(
        sandbox: &'a Sandbox,
        payer: &'a Actor,
        market: &'a Market<'a>,
        starting_lamports: u64,
        starting_base: u64,
        starting_quote: u64,
    ) -> Result<Participant<'a>, Error> {
        // Create a participant actor with initial balance
        let participant = Actor::new(sandbox);
        participant.airdrop(starting_lamports)?;

        // Setup base and quote accounts
        let participant_base =
            TokenAccount::new(sandbox, payer, market.base_mint, Some(participant.pubkey()))?;
        let participant_quote = TokenAccount::new(
            sandbox,
            payer,
            market.quote_mint,
            Some(participant.pubkey()),
        )?;

        // Mint amounts to base & quote token accounts
        if starting_base > 0 {
            market
                .base_mint
                .mint_to(payer, &participant_base, starting_base)?;
        }
        if starting_quote > 0 {
            market
                .quote_mint
                .mint_to(payer, &participant_quote, starting_quote)?;
        }

        // Create open orders account
        let participant_open_orders = Actor::new(sandbox);
        let open_orders_size = std::mem::size_of::<serum_dex::state::OpenOrders>()
            + serum_state::ACCOUNT_HEAD_PADDING.len()
            + serum_state::ACCOUNT_TAIL_PADDING.len();

        // Set serum to the owner of the open orders account
        let create_open_orders = solana_sdk::system_instruction::create_account(
            payer.pubkey(),
            participant_open_orders.pubkey(),
            sandbox
                .client()
                .get_minimum_balance_for_rent_exemption(open_orders_size)?,
            open_orders_size as u64,
            market.serum,
        );

        // Set participant_open_order's userspace owner to participant
        let init_open_orders = serum_dex::instruction::init_open_orders(
            market.serum,
            participant_open_orders.pubkey(),
            participant.pubkey(),
            market.market.pubkey(),
            market.authority,
        )?;

        // Push both transactions
        let recent_hash = sandbox.client().get_latest_blockhash()?;
        let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[create_open_orders, init_open_orders],
            Some(payer.pubkey()),
            &vec![
                payer.keypair(),
                participant_open_orders.keypair(),
                participant.keypair(),
            ],
            recent_hash,
        );
        sandbox
            .client()
            .send_and_confirm_transaction(&transaction)?;

        Ok(Participant {
            _market: &market,
            _base: participant_base,
            _quote: participant_quote,
            _open_orders: participant_open_orders,
            _account: participant,
        })
    }
}
