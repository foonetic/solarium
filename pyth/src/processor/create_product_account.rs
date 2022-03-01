use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::instruction::CreateProductAccountInstruction;
use crate::state::Price;

use crate::pack::PythPack;


pub fn handle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    pub_instr: CreateProductAccountInstruction,
) -> ProgramResult {
    Ok(())
}
