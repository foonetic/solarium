use crate::error::{PythError, Result};
use crate::instruction::PythInstructionId;

pub trait PythPack: Sized {
    /// Length of the data
    const LEN: usize;

    /// Unpack into slice
    fn unpack_from_slice(src: &[u8]) -> Result<Self>;

    /// Unpack `count` of this type from a vector
    fn unpack_items_from_slice(count: usize, src: &[u8]) -> Result<Vec<Self>> {
        let mut start = 0;
        let mut end = Self::LEN;
        let mut result: Vec<Self> = Vec::new();
        for _ in 0..count {
            let bytes = &src[start..end];
            result.push(Self::unpack_from_slice(bytes)?);
            start += Self::LEN;
            end += Self::LEN;
        }
        Ok(result)
    }

    /// Pack into slice
    fn pack_into_slice(&self, dst: &mut [u8]) -> Result<()>;

    /// Allocate a vector of size LEN & serialize data into it
    fn pack_into_vec(&self) -> Result<Vec<u8>> {
        let mut instr_data: Vec<u8> = vec![0; Self::LEN];
        Self::pack_into_slice(self, &mut instr_data)?;
        Ok(instr_data)
    }

    /// Pack an array of self into a slice
    fn pack_items_into_slice(items: &[Self], dst: &mut [u8]) -> Result<()> {
        let mut start = 0;
        let mut end = Self::LEN;
        for item in items.iter() {
            Self::pack_into_slice(item, &mut dst[start..end])?;
            start += Self::LEN;
            end += Self::LEN;
        }
        Ok(())
    }
}

/// Trait for packing sized Raptor instructions
pub trait PythInstruction: PythPack {
    const ID: PythInstructionId;

    /// Unpack entire instruction
    fn unpack_instruction(src: &[u8]) -> Result<Self> {
        let id: u8 = Self::ID.try_into().unwrap();
        if src[0] != id {
            Err(PythError::NotImplemented)
        } else {
            Self::unpack_from_slice(&src[1..])
        }
    }

    /// Pack entire instruction into payload
    fn pack_instruction(&self, dst: &mut [u8]) -> Result<()> {
        let identifier: u8 = Self::ID.try_into().unwrap();
        dst[0] = identifier;
        Self::pack_into_slice(self, &mut dst[1..])?;
        Ok(())
    }

    /// Allocate a vector of size  & pack into it
    fn pack_instruction_into_vec(&self) -> Result<Vec<u8>> {
        let mut instr_data: Vec<u8> = vec![0; Self::LEN + 1];
        let identifier: u8 = Self::ID.try_into().unwrap();
        instr_data[0] = identifier;
        Self::pack_into_slice(self, &mut instr_data[1..])?;
        Ok(instr_data)
    }
}
