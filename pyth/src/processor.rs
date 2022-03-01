pub mod create_mapping_account;
pub mod create_price_account;
pub mod create_product_account;
pub mod publish_price;

use crate::error::{PythError, Result};
use num_enum::TryFromPrimitive;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, msg};

use crate::instruction::{CreatePriceAccountInstruction, PublishPriceInstruction, CreateMappingAccountInstruction, CreateProductAccountInstruction, PythInstructionId};
use crate::pack::PythPack;

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(PythError::CouldNotDecodeInstruction.into());
    }

    let instruction_id = PythInstructionId::try_from_primitive(instruction_data[0]).unwrap();
    let instruction_data = &instruction_data[1..];

    msg!("process!");

    match instruction_id {
        
        PythInstructionId::CreatePriceAccount => {
            let unpacked_instruction =
                CreatePriceAccountInstruction::unpack_from_slice(instruction_data)?;
            create_price_account::handle(program_id, accounts, unpacked_instruction)
        }

        PythInstructionId::PublishPrice => {
            let unpacked_instruction =
                PublishPriceInstruction::unpack_from_slice(instruction_data)?;
            publish_price::handle(program_id, accounts, unpacked_instruction)
        }

        PythInstructionId::CreateProductAccount => {
            let unpacked_instruction =
                CreateProductAccountInstruction::unpack_from_slice(instruction_data)?;
            create_product_account::handle(program_id, accounts, unpacked_instruction)
        }

        PythInstructionId::CreateMappingAccount => {
            let unpacked_instruction =
                CreateMappingAccountInstruction::unpack_from_slice(instruction_data)?;
            create_mapping_account::handle(program_id, accounts, unpacked_instruction)
        }
    }
}
