use num_enum::{IntoPrimitive, TryFromPrimitive};
use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone, PartialEq)]
pub enum PythError {
    #[error("Instruction not implemented.")]
    NotImplemented,
    #[error("Could not decode instruction.")]
    CouldNotDecodeInstruction,
    #[error("Invalid instruction id.")]
    InvalidInstructionId,
    #[error("Invalid account.")]
    InvalidAccount,
    #[error("Invalid request status.")]
    InvalidRequestStatus,
    #[error("Invalid seeds.")]
    InvalidSeeds,
}

impl From<PythError> for ProgramError {
    fn from(e: PythError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

pub type Result<T> = std::result::Result<T, PythError>;

pub fn err_at(line: u32, file_id: u8) -> u32 {
    line + ((file_id as u32) << 24)
}

#[macro_export]
macro_rules! assert_or_err {
    ($val:expr, $file_id:expr) => {{
        if $val {
            Ok(())
        } else {
            Err(ProgramError::Custom(err_at(line!(), $file_id)))
        }
    }};
}
