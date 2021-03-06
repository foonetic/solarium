use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::error::Result;
use crate::pack::{PythInstruction, PythPack};

#[derive(Eq, PartialEq, PartialOrd, Debug, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum PythInstructionId {
    CreatePriceAccount,
    CreateProductAccount,
    CreateMappingAccount,
    PublishPrice,
}

#[derive(Eq, PartialEq, PartialOrd, Debug, Clone)]
pub struct CreatePriceAccountInstruction {
}

impl PythInstruction for CreatePriceAccountInstruction {
    const ID: PythInstructionId = PythInstructionId::CreatePriceAccount;
}

impl PythPack for CreatePriceAccountInstruction {
    const LEN: usize = 0;

    fn unpack_from_slice(src: &[u8]) -> Result<Self> {
        Ok(Self { })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()> {
        Ok(())
    }
}

#[derive(Eq, PartialEq, PartialOrd, Debug, Clone)]
pub struct CreateProductAccountInstruction {
}

impl PythInstruction for CreateProductAccountInstruction {
    const ID: PythInstructionId = PythInstructionId::CreateProductAccount;
}

impl PythPack for CreateProductAccountInstruction {
    const LEN: usize = 0;

    fn unpack_from_slice(src: &[u8]) -> Result<Self> {
        Ok(Self { })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()> {
        Ok(())
    }
}

#[derive(Eq, PartialEq, PartialOrd, Debug, Clone)]
pub struct CreateMappingAccountInstruction {
}

impl PythInstruction for CreateMappingAccountInstruction {
    const ID: PythInstructionId = PythInstructionId::CreateMappingAccount;
}

impl PythPack for CreateMappingAccountInstruction {
    const LEN: usize = 0;

    fn unpack_from_slice(src: &[u8]) -> Result<Self> {
        Ok(Self { })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()> {
        Ok(())
    }
}

#[derive(Eq, PartialEq, PartialOrd, Debug, Clone)]
pub struct PublishPriceInstruction {
    pub price: i64,
    pub decimal: i32,
}

impl PythInstruction for PublishPriceInstruction {
    const ID: PythInstructionId = PythInstructionId::PublishPrice;
}

impl PythPack for PublishPriceInstruction {
    const LEN: usize = 12;

    fn unpack_from_slice(src: &[u8]) -> Result<Self> {
        let src = array_ref![src, 0, PublishPriceInstruction::LEN];
        let (price, decimal) = array_refs![src, 8, 4];

        let price = i64::from_le_bytes(*price);
        let decimal = i32::from_le_bytes(*decimal);

        Ok(Self { price, decimal })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()> {
        let dst = array_mut_ref![dst, 0, PublishPriceInstruction::LEN];
        let (tp_dst, dec_dst) = mut_array_refs![dst, 8, 4];
        *tp_dst = self.price.to_le_bytes();
        *dec_dst = self.decimal.to_le_bytes();
        Ok(())
    }
}


pub fn create_price_acc(
    program_id: &Pubkey,
    payer: &Pubkey,
    acct_pkey: &Pubkey,
) -> Result<Instruction> {
    let data = CreatePriceAccountInstruction { }.pack_instruction_into_vec()?;
    let accounts = vec![
        AccountMeta::new_readonly(*payer, true),
        AccountMeta::new(*acct_pkey, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}

pub fn publish_price(
    program_id: &Pubkey,
    payer: &Pubkey,
    acct_pkey: &Pubkey,
    price: i64,
    decimal: i32,
) -> Result<Instruction> {
    let data = PublishPriceInstruction { price, decimal }.pack_instruction_into_vec()?;
    let accounts = vec![
        AccountMeta::new_readonly(*payer, true),
        AccountMeta::new(*acct_pkey, false),
    ];
    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}
