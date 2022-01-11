use crate::actor::Actor;
use crate::errors::Error;
use crate::sandbox::Sandbox;
use solana_program::program_pack::Pack;
use solana_sdk::{pubkey::Pubkey, transaction::Transaction};
use spl_token::{self, instruction as spl_instruction, state as spl_state};

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
    ) -> Result<Mint<'a>, Error> {
        let mint = Actor::new(sandbox);

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

        let recent_hash = sandbox.client().get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[create_account, initialize_mint],
            Some(actor.pubkey()),
            &[actor.keypair(), mint.keypair()],
            recent_hash,
        );
        sandbox
            .client()
            .send_and_confirm_transaction(&transaction)?;

        Ok(Mint {
            sandbox: sandbox,
            mint: mint,
            authority: authority,
            freeze_authority: freeze_authority,
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
    pub fn mint_to(
        &self,
        actor: &Actor,
        destination: &TokenAccount,
        amount: u64,
    ) -> Result<(), Error> {
        let instruction = spl_instruction::mint_to(
            &spl_token::id(),
            self.mint.pubkey(),
            destination.account().pubkey(),
            self.authority.pubkey(),
            &[],
            amount,
        )?;

        let recent_hash = self.sandbox.client().get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(actor.pubkey()),
            &[actor.keypair(), self.authority.keypair()],
            recent_hash,
        );
        self.sandbox
            .client()
            .send_and_confirm_transaction(&transaction)?;

        Ok(())
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
    ) -> Result<TokenAccount<'a>, Error> {
        let account = Actor::new(sandbox);

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

        let recent_hash = sandbox.client().get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[create_account, initialize_account],
            Some(actor.pubkey()),
            &[actor.keypair(), account.keypair()],
            recent_hash,
        );
        sandbox
            .client()
            .send_and_confirm_transaction(&transaction)?;

        Ok(TokenAccount {
            sandbox: sandbox,
            account: account,
        })
    }

    /// Returns the underlying account.
    pub fn account(&self) -> &Actor {
        &self.account
    }

    /// Returns the current state of the account.
    pub fn get_state(&self) -> Result<spl_token::state::Account, Error> {
        let data = self
            .sandbox
            .client()
            .get_account_data(self.account.pubkey())?;
        Ok(spl_token::state::Account::unpack_from_slice(&data)?)
    }
}
