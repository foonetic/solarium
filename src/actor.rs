use crate::errors::Error;
use crate::sandbox::Sandbox;
use serde_json;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::{io::Write, path::Path, process, thread, time};

/// Represents a keypair in a parent Sandbox environment.
pub struct Actor<'a> {
    sandbox: &'a Sandbox,
    keypair: Keypair,
    keyfile: tempfile::NamedTempFile,
    pubkey: Pubkey,
}

impl<'a> Actor<'a> {
    /// Creates an Actor in the given Sandbox environment.
    pub fn new(sandbox: &'a Sandbox) -> Self {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey();
        let keyfile =
            tempfile::NamedTempFile::new_in(sandbox.tmpdir()).expect("could not create keyfile");
        solana_sdk::signature::write_keypair_file(&keypair, &keyfile.path());
        keyfile.as_file().flush().expect("could not flush keyfile");

        Self {
            sandbox: sandbox,
            keypair: keypair,
            pubkey: pubkey,
            keyfile: keyfile,
        }
    }

    /// Returns the Actor's keypair.
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }

    /// Returns the Actor's public key.
    pub fn pubkey(&self) -> &Pubkey {
        &self.pubkey
    }

    /// Returns the path to a JSON file on disk containing the Actor's private
    /// key.
    pub fn keyfile(&self) -> &std::path::Path {
        self.keyfile.path()
    }

    pub fn sandbox(&self) -> &Sandbox {
        self.sandbox
    }

    /// Airdrops the given number of lamports to this actor. Blocks until the
    /// airdrop is complete.
    pub fn airdrop(&self, lamports: u64) -> Result<(), Error> {
        let signature = self
            .sandbox
            .client()
            .request_airdrop(self.pubkey(), lamports)?;
        while !self.sandbox.client().confirm_transaction(&signature)? {
            thread::sleep(time::Duration::from_millis(10));
        }
        Ok(())
    }

    /// Attempts to deploy a program if it exists locally. If it does not,
    /// it will fall back on deploy_remote.
    pub fn try_deploy_local(
        &self,
        program_location: &std::path::Path,
        fallback_remote_location: &str,
        fallback_file_name: &str,
    ) -> Result<Actor, Error> {
        if program_location.exists() {
            self.deploy_local(program_location)
        } else {
            self.deploy_remote(fallback_remote_location, fallback_file_name)
        }
    }

    /// Deploys the given program to the Sandbox. The input path should be an
    /// .so file, typically built with `cargo build-bpf` in a path like
    /// `target/deploy/program.so`. Returns the Actor representing the deployed
    /// program. In particular, the returned Actor's public key is the program's
    /// public key.
    pub fn deploy_local(&self, program_location: &std::path::Path) -> Result<Actor, Error> {
        let actor = Actor::new(self.sandbox);

        let code = process::Command::new("solana")
            .args([
                "program",
                "deploy",
                "--keypair",
                self.keyfile().to_str().expect("could not specify keyfile"),
                "--program-id",
                actor.keyfile().to_str().expect("could not specify keyfile"),
                "--commitment",
                "confirmed",
                "--url",
                &self.sandbox.url(),
                program_location
                    .to_str()
                    .expect("could not specify program location"),
            ])
            .spawn()?
            .wait()?;
        if code.success() {
            Ok(actor)
        } else {
            Err(Error::InputOutputError(std::io::Error::from(
                std::io::ErrorKind::InvalidInput,
            )))
        }
    }

    // Grabs executable from git and replicates it in the /solarium directory
    // Then, deploys the program to solana
    // remote_location: url to raw binary (i.e. ../../raw/../something.so)
    // file_name: local file name via wget
    pub fn deploy_remote(&self, remote_location: &str, file_name: &str) -> Result<Actor, Error> {
        let actor = Actor::new(self.sandbox);

        let _get = process::Command::new("wget")
            .args(["-O", file_name, remote_location])
            .spawn()?
            .wait()?;

        let code = process::Command::new("solana")
            .args([
                "program",
                "deploy",
                "--keypair",
                &self.keyfile().to_str().expect("could not specify keyfile"),
                "--program-id",
                &actor.keyfile().to_str().expect("could not specify keyfile"),
                "--commitment",
                "confirmed",
                "--url",
                &self.sandbox.url(),
                &("./".to_owned() + file_name),
            ])
            .spawn()?
            .wait()?;
        if code.success() {
            Ok(actor)
        } else {
            Err(Error::InputOutputError(std::io::Error::from(
                std::io::ErrorKind::InvalidInput,
            )))
        }
    }

    /// Returns an instruction to create an account at the given address with
    /// the given size and owner. Funds the account so that it is rent-exempt.
    pub fn create_account(
        &self,
        target: &Pubkey,
        target_bytes: usize,
        target_owner: &Pubkey,
    ) -> Result<Instruction, Error> {
        Ok(solana_sdk::system_instruction::create_account(
            self.pubkey(),
            target,
            self.sandbox
                .client()
                .get_minimum_balance_for_rent_exemption(target_bytes)?,
            target_bytes as u64,
            target_owner,
        ))
    }
}
