use crate::actor::Actor;
use crate::errors::{Error, Result};
use crate::sandbox::Sandbox;
use crate::token::{Mint, TokenAccount};
use bytemuck;
use pyth_sim::state::Price;
use solana_sdk::pubkey::Pubkey;
use std::mem::size_of;
use std::num::NonZeroU64;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use pyth_sim::instruction::CreatePriceAccountInstruction;

pub struct PriceAccount<'a> {
    sandbox: &'a Sandbox,
    account: Actor<'a>,
}

impl<'a> PriceAccount<'a> {
    pub fn new(sandbox: &'a Sandbox, pyth: &'a Pubkey, payer: &'a Actor) -> Result<Self> {
        let acc = Actor::new(sandbox)?;

        let sized_accounts = vec![(acc.pubkey(), 3312)];

        let mut instructions = Vec::new();

        for (pubkey, len) in sized_accounts.iter() {
            instructions.push(payer.create_account(pubkey, *len, pyth)?);
        }

        let create_instr =
            pyth_sim::instruction::create_price_acc(pyth, payer.pubkey(), acc.pubkey()).unwrap();

        instructions.push(create_instr);

        sandbox.send_signed_transaction_with_payers(
            &instructions,
            Some(payer.pubkey()),
            vec![payer.keypair(), acc.keypair()],
        )?;

        Ok(PriceAccount {
            sandbox,
            account: acc,
        })
    }

    pub fn publish_price(
        &self,
        pyth: &'a Pubkey,
        payer: &'a Actor,
        price: i64,
        decimal: i32,
    ) -> Result<()> {
        let mut instructions = Vec::new();

        let publish_instr = pyth_sim::instruction::publish_price(
            pyth,
            payer.pubkey(),
            &self.account.pubkey(),
            price,
            decimal,
        )
        .unwrap();

        instructions.push(publish_instr);

        &self.sandbox.send_signed_transaction_with_payers(
            &instructions,
            Some(payer.pubkey()),
            vec![payer.keypair()],
        )?;

        Ok(())
    }

    pub fn account(&self) -> &Actor {
        &self.account
    }
}
