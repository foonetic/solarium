use crate::actor::Actor;
use crate::errors::{Error, Result};
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
use std::thread;
use std::thread::sleep;
use std::time::Duration;

/// Represents a Serum market. This is a V2 market if there is an authority
/// specified, otherwise a V1 market.
pub struct Market<'a> {
    sandbox: &'a Sandbox,
    serum: &'a Pubkey,
    market: Actor<'a>,
    authority: Option<&'a Pubkey>,
    request_queue: Actor<'a>,
    event_queue: Actor<'a>,
    bids: Actor<'a>,
    asks: Actor<'a>,
    base_vault: TokenAccount<'a>,
    quote_vault: TokenAccount<'a>,
    vault_signer_key: Pubkey,
    base_mint: &'a Mint<'a>,
    quote_mint: &'a Mint<'a>,
    pub open_orders_accounts: Vec<&'a Pubkey>,
}

impl<'a> Market<'a> {
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
    ) -> Result<Self> {
        // Make sure that certain accounts meet the minimum size requirements for allocation
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

        let market = Actor::new(sandbox)?;
        let request_queue = Actor::new(sandbox)?;
        let event_queue = Actor::new(sandbox)?;
        let bids = Actor::new(sandbox)?;
        let asks = Actor::new(sandbox)?;

        let (vault_address, vault_nonce) = Self::create_vault_address(serum, market.pubkey());
        let base_vault = TokenAccount::new(sandbox, actor, base_mint, Some(&vault_address))?;
        let quote_vault = TokenAccount::new(sandbox, actor, quote_mint, Some(&vault_address))?;
        let has_authority = authority.is_some();

        // Fetch the size of serum accounts so that we can send create_account
        // instructions with the appropriate sizes.
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

        // Bundle create_account instructions
        let mut instructions = Vec::new();
        for (pubkey, len) in sized_accounts.iter() {
            instructions.push(actor.create_account(pubkey, *len, serum)?);
        }

        // Trail with market initialization
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

        sandbox.send_signed_transaction_with_payers(
            &instructions,
            Some(actor.pubkey()),
            vec![
                actor.keypair(),
                market.keypair(),
                request_queue.keypair(),
                event_queue.keypair(),
                bids.keypair(),
                asks.keypair(),
            ],
        )?;

        let vault_signer_key =
            serum_dex::state::gen_vault_signer_key(vault_nonce, market.pubkey(), serum)?;

        Ok(Market {
            sandbox,
            serum,
            market,
            authority,
            request_queue,
            event_queue,
            bids,
            asks,
            base_vault,
            quote_vault,
            vault_signer_key,
            base_mint,
            quote_mint,
            open_orders_accounts: Vec::new(),
        })
    }

    /// Creates a new order and pushes it to the sandbox -
    /// will fail if the transaction does not go through.
    /// It is important to note that matching occurs at this state
    /// inside of Serum itself in V3, however, in earlier versions,
    /// this does not occur until requests are popped off of the request queue.
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
    ) -> Result<()> {
        let new_order_instruction = serum_dex::instruction::new_order(
            self.market.pubkey(),
            participant.open_orders().pubkey(),
            self.request_queue.pubkey(),
            self.event_queue.pubkey(),
            self.bids.pubkey(),
            self.asks.pubkey(),
            payer.pubkey(),
            participant.account().pubkey(),
            self.base_vault.account().pubkey(),
            self.quote_vault.account().pubkey(),
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
        )?;

        self.sandbox.send_signed_transaction_with_payers(
            &[new_order_instruction],
            Some(participant.account.pubkey()),
            vec![participant.account.keypair()],
        )
    }

    /// Spin up consume_events_loop on another thread and kill it after
    /// crank_for_ms milliseconds.
    pub fn consume_events_loop(
        &self,
        cranker: &Actor,
        num_workers: usize,
        events_per_worker: usize,
        log_directory: String,
        crank_for_ms: u64,
    ) -> Result<()> {
        let payer = cranker
            .keyfile()
            .to_str()
            .ok_or_else(|| {
                Error::InputOutputError(std::io::Error::from(std::io::ErrorKind::NotFound))
            })?
            .to_string();

        let consume_events_command = crank::Command::ConsumeEvents {
            dex_program_id: *self.serum,
            payer,
            market: *self.market.pubkey(),
            coin_wallet: *self.base_vault.account().pubkey(),
            pc_wallet: *self.quote_vault.account().pubkey(),
            num_workers,
            events_per_worker,
            num_accounts: None,
            log_directory,
            max_q_length: None,
            max_wait_for_events_delay: None,
        };

        let crank_opts = crank::Opts {
            cluster: serum_common::client::Cluster::Custom(cranker.sandbox().url()),
            command: consume_events_command,
        };

        // For some reason, when unwrapped, crank_opts panics saying that the market pubkey
        // is not found. Despite this, it still works. I need to look into why this is.
        thread::spawn(|| {
            crank::start(crank_opts);
        });

        sleep(Duration::from_millis(crank_for_ms));

        Ok(())
    }

    /// Cranker settles funds for a particular participant by invoking crank::start
    pub fn settle_funds(&self, cranker: &Actor, participant: &Participant) -> Result<()> {
        let payer = cranker
            .keyfile()
            .to_str()
            .ok_or_else(|| {
                Error::InputOutputError(std::io::Error::from(std::io::ErrorKind::NotFound))
            })?
            .to_string();

        let settle_funds_command = crank::Command::SettleFunds {
            payer,
            dex_program_id: *self.serum,
            market: *self.market.pubkey(),
            orders: *participant.open_orders().pubkey(),
            coin_wallet: *self.base_vault.account().pubkey(),
            pc_wallet: *self.quote_vault.account().pubkey(),
            signer: None,
        };

        let crank_opts = crank::Opts {
            cluster: serum_common::client::Cluster::Custom(cranker.sandbox().url()),
            command: settle_funds_command,
        };

        // For some reason, when unwrapped, crank_opts panics saying that the market pubkey
        // is not found. Despite this, it still works. I need to look into why this is.
        crank::start(crank_opts);

        Ok(())
    }

    /// Returns reference to the Serum program id
    pub fn serum(&self) -> &Pubkey {
        self.serum
    }

    /// Returns a reference to the underlying market account
    pub fn market(&self) -> &Actor {
        &self.market
    }

    /// Returns reference to market authority account
    pub fn authority(&self) -> &Option<&Pubkey> {
        &self.authority
    }

    /// Returns reference to request queue account
    pub fn request_queue(&self) -> &Actor {
        &self.request_queue
    }

    /// Returns reference to event queue account
    pub fn event_queue(&self) -> &Actor {
        &self.event_queue
    }

    /// Returns reference to bids account
    pub fn bids(&self) -> &Actor {
        &self.bids
    }

    /// Returns reference to asks account
    pub fn asks(&self) -> &Actor {
        &self.asks
    }

    /// Returns reference to this market's base vault account
    pub fn base_vault(&self) -> &TokenAccount {
        &self.base_vault
    }

    /// Returns reference to this market's quote vault account
    pub fn quote_vault(&self) -> &TokenAccount {
        &self.quote_vault
    }

    /// Returns reference to this market's base mint account
    pub fn base_mint(&self) -> &Mint {
        &self.base_mint
    }

    /// Returns reference to this market's quote mint account
    pub fn quote_mint(&self) -> &Mint {
        &self.quote_mint
    }

    /// Returns reference to this market's vault signer key
    pub fn vault_signer_key(&self) -> &Pubkey {
        &self.vault_signer_key
    }

    /// Fetch the size/space of the request queue account given a number of requests
    fn request_queue_size(num_requests: usize) -> usize {
        let mut size: usize = 0;
        size += serum_state::ACCOUNT_HEAD_PADDING.len();
        size += serum_state::ACCOUNT_TAIL_PADDING.len();
        size += std::mem::size_of::<serum_state::RequestQueueHeader>();
        size += num_requests * std::mem::size_of::<serum_state::Request>();
        size
    }

    /// Fetch the size/space of the event queue account given a number of events
    fn event_queue_size(num_events: usize) -> usize {
        let mut size: usize = 0;
        size += serum_state::ACCOUNT_HEAD_PADDING.len();
        size += serum_state::ACCOUNT_TAIL_PADDING.len();
        size += std::mem::size_of::<serum_state::EventQueueHeader>();
        size += num_events * std::mem::size_of::<serum_state::Event>();
        size
    }

    /// Fetch the size/space of the side account given a number of nodes
    fn side_size(num_nodes: usize) -> usize {
        let mut size: usize = 0;
        size += serum_state::ACCOUNT_HEAD_PADDING.len();
        size += serum_state::ACCOUNT_TAIL_PADDING.len();
        size += 8; // private struct OrderBookStateHeader
        size += 8 + 8 + 4 + 4 + 8; // private struct SlabHeader
        size += num_nodes * std::mem::size_of::<serum_dex::critbit::AnyNode>();
        size
    }

    /// Fetch the size/space of the market account depending on authority
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
}

/// Represents a Serum market participant.
pub struct Participant<'a> {
    market: &'a Market<'a>,
    base: TokenAccount<'a>,
    quote: TokenAccount<'a>,
    open_orders: Actor<'a>,
    account: Actor<'a>,
}

impl<'a> Participant<'a> {
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
    ) -> Result<Participant<'a>> {
        // Create a participant actor with initial balance
        let participant_actor = Actor::new(sandbox)?;
        participant_actor.airdrop(starting_lamports)?;

        // Setup base and quote accounts
        let participant_base = TokenAccount::new(
            sandbox,
            payer,
            market.base_mint,
            Some(participant_actor.pubkey()),
        )?;
        let participant_quote = TokenAccount::new(
            sandbox,
            payer,
            market.quote_mint,
            Some(participant_actor.pubkey()),
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
        let participant_open_orders = Actor::new(sandbox)?;
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
            participant_actor.pubkey(),
            market.market.pubkey(),
            None,
        )?;

        sandbox.send_signed_transaction_with_payers(
            &[create_open_orders, init_open_orders],
            Some(payer.pubkey()),
            vec![
                payer.keypair(),
                participant_open_orders.keypair(),
                participant_actor.keypair(),
            ],
        )?;

        Ok(Participant {
            market,
            base: participant_base,
            quote: participant_quote,
            open_orders: participant_open_orders,
            account: participant_actor,
        })
    }

    /// Returns reference to base account.
    pub fn base(&self) -> &Actor {
        self.base.account()
    }

    /// Returns reference to quote account.
    pub fn quote(&self) -> &Actor {
        self.quote.account()
    }

    /// Returns reference to open orders account.
    pub fn open_orders(&self) -> &Actor {
        &self.open_orders
    }

    /// Returns reference to underlying account.
    pub fn account(&self) -> &Actor {
        &self.account
    }
}
