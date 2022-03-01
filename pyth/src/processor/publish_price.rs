use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
    clock::Clock, 
    sysvar::Sysvar,
};

use crate::instruction::PublishPriceInstruction;
use crate::state::Price;

use crate::pack::PythPack;


pub fn handle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    pub_instr: PublishPriceInstruction,
) -> ProgramResult {

    let ai_iter = &mut accounts.iter();
    let payer_acct = next_account_info(ai_iter)?;
    let acct_pkey = next_account_info(ai_iter)?;

    let price: i64 = pub_instr.price as i64;

    let mut price_struct: Price = Price::unpack_from_slice(&acct_pkey.data.borrow_mut())?;
 
    price_struct.agg.price = price;
    price_struct.agg.pub_slot = Clock::get().unwrap().slot;
    price_struct.pack_into_slice(&mut *acct_pkey.data.borrow_mut())?;

    Ok(())
}
