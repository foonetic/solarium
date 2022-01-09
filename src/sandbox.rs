use portpicker;
use serde_json;
use solana_client::rpc_client;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::{
    io::{self, Write},
    process, thread, time,
};
use tempfile;

/// Represents a Solana test environment.
///
/// A Sandbox wraps a solana-test-validator instance. A Sandbox facilitates the
/// creation of Actors, which represent keypairs known to this environment.
pub struct Sandbox {
    tmp: tempfile::TempDir,
    validator: process::Child,
    port: u16,
    client: rpc_client::RpcClient,
}

impl Sandbox {
    /// Creates a Sandbox and blocks until the RPC server is ready to use.
    pub fn new() -> Result<Self, io::Error> {
        let tmp = tempfile::Builder::new().prefix("solarium").tempdir()?;
        let port = portpicker::pick_unused_port();
        if port.is_none() {
            return Err(io::Error::from(io::ErrorKind::AddrNotAvailable));
        }

        let port = port.expect("could not get port");
        let validator = process::Command::new("solana-test-validator")
            .args([
                "--ledger",
                &tmp.path()
                    .join("solana-test-validator-ledger")
                    .into_os_string()
                    .into_string()
                    .expect("could not get tmp path"),
                "--rpc-port",
                &port.to_string(),
            ])
            .stdout(std::process::Stdio::null())
            .spawn()?;

        let client =
            rpc_client::RpcClient::new(String::from("http://localhost:") + &port.to_string());

        // Wait for the cluster to come online and respond to basic commands.
        while client.get_latest_blockhash().is_err() {
            thread::sleep(time::Duration::from_millis(10));
        }

        Ok(Self {
            tmp: tmp,
            validator: validator,
            port: port,
            client: client,
        })
    }

    /// Returns the validator's RPC service port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the validator's RPC service url.
    pub fn url(&self) -> String {
        String::from("http://localhost:") + &self.port.to_string()
    }

    /// Returns an RPC client that is connected to the validator.
    pub fn client(&self) -> &rpc_client::RpcClient {
        &self.client
    }

    /// Returns a temporary directory associated with this Sandbox.
    pub fn tmpdir(&self) -> &std::path::Path {
        self.tmp.as_ref()
    }

    /// Creates an Actor configured for this Sandbox.
    pub fn create_actor(&self) -> Actor {
        Actor::new(self)
    }
}

impl Drop for Sandbox {
    /// Stops the validator.
    fn drop(&mut self) {
        self.validator.kill().unwrap_or(());
    }
}

/// Represents a keypair in a parent Sandbox environment.
pub struct Actor<'a> {
    sandbox: &'a Sandbox,
    keypair: Keypair,
    keyfile: tempfile::NamedTempFile,
    pubkey: Pubkey,
}

impl<'a> Actor<'a> {
    fn new(sandbox: &'a Sandbox) -> Self {
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
    pub fn airdrop(&self, lamports: u64) {
        let signature = self
            .sandbox
            .client()
            .request_airdrop(self.pubkey(), lamports)
            .expect("could not request airdrop");
        while !self
            .sandbox
            .client()
            .confirm_transaction(&signature)
            .expect("could not confirm airdrop")
        {
            thread::sleep(time::Duration::from_millis(10));
        }
    }

    /// Deploys the given program to the Sandbox. The input path should be an
    /// .so file, typically built with `cargo build-bpf` in a path like
    /// `target/deploy/program.so`. Returns the Actor representing the deployed
    /// program. In particular, the returned Actor's public key is the program's
    /// public key.
    pub fn deploy(&self, program_location: &std::path::Path) -> Actor {
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
            .spawn()
            .expect("failed to deploy")
            .wait()
            .expect("failed to wait");
        assert!(code.success());
        actor
    }
}
