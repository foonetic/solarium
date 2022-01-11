use portpicker;
use solana_client::rpc_client;
use std::{io, process, thread, time};
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
    runtime: tokio::runtime::Runtime,
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
            runtime: tokio::runtime::Runtime::new().unwrap(),
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

    /// Returns an async runtime.
    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.runtime
    }
}

impl Drop for Sandbox {
    /// Stops the validator.
    fn drop(&mut self) {
        self.validator.kill().unwrap_or(());
    }
}
