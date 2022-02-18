use crate::errors::{Error, Result};
use portpicker;
use solana_client::rpc_client;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signer::keypair::Keypair, transaction::Transaction,
};
use std::{io, path::Path, process, thread, time};
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
    pub fn new() -> Result<Self> {
        let tmp = tempfile::Builder::new().prefix("solarium").tempdir()?;
        let port = portpicker::pick_unused_port();
        let faucet = portpicker::pick_unused_port();
        if port.is_none() {
            return Err(Error::from(io::Error::from(
                io::ErrorKind::AddrNotAvailable,
            )));
        }

        if faucet.is_none() {
            return Err(Error::from(io::Error::from(
                io::ErrorKind::AddrNotAvailable,
            )));
        }

        let port = port.expect("could not get port");
        let faucet = faucet.expect("could not get faucet");
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
                "--faucet-port",
                &faucet.to_string(),
            ])
            .stdout(std::process::Stdio::null())
            .spawn()?;

        let commitment_level = solana_sdk::commitment_config::CommitmentConfig::confirmed();
        let client = rpc_client::RpcClient::new_with_commitment(
            String::from("http://localhost:") + &port.to_string(),
            commitment_level,
        );

        // Wait for the cluster to come online and respond to basic commands.
        while client.get_latest_blockhash().is_err() {
            thread::sleep(time::Duration::from_millis(10));
        }

        Ok(Self {
            tmp,
            validator,
            port,
            client,
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
    pub fn tmpdir(&self) -> &Path {
        self.tmp.as_ref()
    }

    /// Create & send signed transaction with payers from instructions
    pub fn send_signed_transaction_with_payers(
        &self,
        instructions: &[Instruction],
        payer: Option<&Pubkey>,
        signers: Vec<&Keypair>,
    ) -> Result<()> {
        let recent_hash = self.client.get_latest_blockhash()?;
        let transaction =
            Transaction::new_signed_with_payer(instructions, payer, &signers, recent_hash);
        self.client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// Create & send transaction with payers from instructions
    pub fn send_transaction_with_payer(
        &self,
        instructions: &[Instruction],
        payer: Option<&Pubkey>,
    ) -> Result<()> {
        let transaction = Transaction::new_with_payer(instructions, payer);
        self.client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }
}

impl Drop for Sandbox {
    /// Stops the validator.
    fn drop(&mut self) {
        self.validator.kill().unwrap_or(());
    }
}
