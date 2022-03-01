use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};

use crate::instruction::CreatePriceAccountInstruction;

use pyth_client:: {
    MAGIC,
    VERSION_2,
    AccountType,
    PriceStatus,
    CorpAction,
};

pub fn handle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    initialize_instr: CreatePriceAccountInstruction,
) -> ProgramResult {

    let ai_iter = &mut accounts.iter();
    let payer_acct = next_account_info(ai_iter)?;
    let acct_pkey = next_account_info(ai_iter)?;

    let mut data = &mut *acct_pkey.data.borrow_mut();

    for x in 0..4 {
        data[x] = u32::to_le_bytes(MAGIC)[x];
    }

    for x in 4..8 {
        data[x] = u32::to_le_bytes(VERSION_2)[x-4];
    }

    for x in 8..12 {
        data[x] = u32::to_le_bytes(AccountType::Price as u32)[x-8];
    }
    for x in 12..3312 {
        data[x] = 0;
    }

    let pr: i64 = 0;
    let conf: u64 = 100;
    let status: PriceStatus = PriceStatus::Trading;
    let corp_act = CorpAction::NoCorpAct;
    let pub_slot: u64 = 0; 

    for x in 208..216 {
        data[x] = i64::to_le_bytes(pr)[x - 208];
    }

    for x in 216..224 {
        data[x] = u64::to_le_bytes(conf)[x - 216];
    }

    for x in 224..228 {
        data[x] = u32::to_le_bytes(status as u32)[x - 224];
    }

    for x in 228..232 {
        data[x] = u32::to_le_bytes(corp_act as u32)[x - 228];
    }

    for x in 232..240 {
        data[x] = u64::to_le_bytes(pub_slot)[x - 232];
    }
    
    Ok(())
}
