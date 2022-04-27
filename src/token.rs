use crate::actor::Actor;
use crate::errors::Result;
use crate::sandbox::Sandbox;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use spl_token::{self, instruction as spl_instruction, state as spl_state};
use spl_associated_token_account::{instruction as spl_assocated_instruction};

#[derive(BorshSerialize, BorshDeserialize, Eq, PartialEq, PartialOrd)]
pub enum BaseOrQuote {
    Base,
    Quote,
}
/// Represents an spl_token program Mint.
pub struct Mint<'a> {
    sandbox: &'a Sandbox,
    mint: Actor<'a>,
    authority: &'a Actor<'a>,
    freeze_authority: &'a Actor<'a>,
}

impl<'a> Mint<'a> {
    /// Constructs a Mint in the given Sandbox environment.
    ///
    /// The input actor creates the mint and is the default authority and freeze
    /// authority.
    pub fn new(
        sandbox: &'a Sandbox,
        actor: &'a Actor,
        decimals: u8,
        authority: Option<&'a Actor>,
        freeze_authority: Option<&'a Actor>,
    ) -> Result<Mint<'a>> {
        let mint = Actor::new(sandbox)?;

        let authority = match authority {
            Some(auth) => auth,
            None => actor,
        };

        let freeze_authority = match freeze_authority {
            Some(auth) => auth,
            None => authority,
        };

        let create_account =
            actor.create_account(mint.pubkey(), spl_state::Mint::LEN, &spl_token::id())?;
        let initialize_mint = spl_instruction::initialize_mint(
            &spl_token::id(),
            mint.pubkey(),
            authority.pubkey(),
            Some(freeze_authority.pubkey()),
            decimals,
        )?;

        sandbox.send_signed_transaction_with_payers(
            &[create_account, initialize_mint],
            Some(actor.pubkey()),
            vec![actor.keypair(), mint.keypair()],
        )?;

        Ok(Mint {
            sandbox,
            mint,
            authority,
            freeze_authority,
        })
    }

    /// Returns underlying Actor representing the Mint.
    pub fn actor(&self) -> &Actor {
        &self.mint
    }

    /// Returns the Mint authority.
    pub fn authority(&self) -> &Actor {
        self.authority
    }

    /// Returns the Mint freeze authority.
    pub fn freeze_authority(&self) -> &Actor {
        self.freeze_authority
    }

    /// The given Actor mints an amount into the provided token account. Note
    /// that this instruction is always signed by the mint authority, even if
    /// the input actor doesn't have minting authority.
    pub fn mint_to(&self, actor: &Actor, destination: &TokenAccount, amount: u64) -> Result<()> {
        let instruction = spl_instruction::mint_to(
            &spl_token::id(),
            self.mint.pubkey(),
            destination.account().pubkey(),
            self.authority.pubkey(),
            &[],
            amount,
        )?;

        self.sandbox.send_signed_transaction_with_payers(
            &[instruction],
            Some(actor.pubkey()),
            vec![actor.keypair(), self.authority.keypair()],
        )
    }

    pub fn mint_to_pkey(&self, actor: &Actor, destination: &Pubkey, amount: u64) -> Result<()> {
        let instruction = spl_instruction::mint_to(
            &spl_token::id(),
            self.mint.pubkey(),
            destination,
            self.authority.pubkey(),
            &[],
            amount,
        )?;

        self.sandbox.send_signed_transaction_with_payers(
            &[instruction],
            Some(actor.pubkey()),
            vec![actor.keypair(), self.authority.keypair()],
        )
    }
}

/// Represents an spl_token token account.
pub struct TokenAccount<'a> {
    sandbox: &'a Sandbox,
    account: Actor<'a>,
}

impl<'a> TokenAccount<'a> {
    /// Creates and initializes an spl_token account.
    ///
    /// The account is created by the actor. If no owner is specified, then the
    /// actor will be set as the owner.
    pub fn new<'b>(
        sandbox: &'a Sandbox,
        actor: &'a Actor,
        mint: &'a Mint,
        owner: Option<&'b Pubkey>,
    ) -> Result<TokenAccount<'a>> {
        let account = Actor::new(sandbox)?;

        let owner = match owner {
            Some(person) => person,
            None => actor.pubkey(),
        };

        let create_account =
            actor.create_account(account.pubkey(), spl_state::Account::LEN, &spl_token::id())?;
        let initialize_account = spl_instruction::initialize_account(
            &spl_token::id(),
            account.pubkey(),
            mint.actor().pubkey(),
            owner,
        )?;

        sandbox.send_signed_transaction_with_payers(
            &[create_account, initialize_account],
            Some(actor.pubkey()),
            vec![actor.keypair(), account.keypair()],
        )?;

        Ok(TokenAccount { sandbox, account })
    }

    /// Returns the underlying account.
    pub fn account(&self) -> &Actor {
        &self.account
    }

    /// Returns the account information
    pub fn get_account_info(&self) -> Result<spl_token::state::Account> {
        let data = self
            .sandbox
            .client()
            .get_account_data(self.account.pubkey())?;
        Ok(spl_token::state::Account::unpack_from_slice(&data)?)
    }
}
