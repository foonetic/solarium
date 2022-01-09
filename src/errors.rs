#[derive(Debug, foonetic_macros::From)]
pub enum Error {
    SolanaClientError(solana_client::client_error::ClientError),
    SolanaProgramError(solana_sdk::program_error::ProgramError),
    InputOutputError(std::io::Error),
    SerumDexError(serum_dex::error::DexError),
}
