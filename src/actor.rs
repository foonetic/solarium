use crate::errors::Error;
use crate::sandbox::Sandbox;
use serde_json;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::{io::Write, process, thread, time};

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
        let keypair_slice: &[u8] = &keypair.to_bytes();
        let key_json = serde_json::json!(keypair_slice).to_string();
        write!(&keyfile, "{}", key_json).expect("could not write to keyfile");
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

    /// Deploys the given program to the Sandbox. The input path should be an
    /// .so file, typically built with `cargo build-bpf` in a path like
    /// `target/deploy/program.so`. Returns the Actor representing the deployed
    /// program. In particular, the returned Actor's public key is the program's
    /// public key.
    pub fn deploy(&self, program_location: &std::path::Path) -> Result<Actor, Error> {
        let actor = Actor::new(self.sandbox);

        let code = process::Command::new("solana")
            .args([
                "program",
                "deploy",
                "--keypair",
                &self.keyfile().to_str().expect("could not specify keyfile"),
                "--program-id",
                &actor.keyfile().to_str().expect("could not specify keyfile"),
                "--commitment",
                "finalized",
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
}
